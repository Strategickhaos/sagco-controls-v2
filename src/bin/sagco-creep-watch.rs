/// sagco-creep-watch — Red Team opcode: batch scope creep scanner
/// Reads a JSONL file of EUR snapshots, flags every entry where budget_delta
/// exceeds threshold. Emits SAGCO_CREEP_ALERT per offending entry.
///
/// JSONL row format (one per line):
/// {"id":"C001","old_used":1011.6,"old_remaining":68.4,"new_used":851.6,"new_remaining":388.4,"crew_size":4,"hrs_per_day":10}
///
/// USE: sagco-creep-watch <snapshots.jsonl> [--threshold 0.001] [--json]
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{env, fs, io::Write};

#[derive(serde::Serialize)]
struct CreepHit {
    id:           String,
    budget_delta: f64,
    old_budget:   f64,
    new_budget:   f64,
    shift_mhrs:   f64,
    antibody:     String,
    alert:        String,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("USE: sagco-creep-watch <snapshots.jsonl> [--threshold 0.001] [--json]");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let threshold: f64 = args.windows(2)
        .find(|w| w[0] == "--threshold")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(0.001);
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-CREEP-WATCH v1 ===");
    println!("INPUT={}", input_path);
    println!("THRESHOLD={:+.3}", threshold);

    let raw = match fs::read_to_string(input_path) {
        Ok(r) => r,
        Err(e) => {
            println!("ANTIBODY=CREEP_WATCH_READ_ANTIBODY");
            println!("ERROR={}", e);
            println!("STATUS=SAGCO_CREEP_WATCH_FAIL");
            std::process::exit(2);
        }
    };

    let mut total      = 0usize;
    let mut creep_hits: Vec<CreepHit> = Vec::new();
    let mut reduction_count = 0usize;
    let mut reclass_count   = 0usize;
    let mut clean_count     = 0usize;
    let mut bad_input_count = 0usize;

    for (line_no, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }

        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(j) => j,
            Err(_) => {
                println!("WARN=line {} parse error", line_no + 1);
                bad_input_count += 1;
                continue;
            }
        };

        total += 1;

        let pf = |key: &str| v[key].as_f64().unwrap_or(0.0);
        let id            = v["id"].as_str().unwrap_or(&format!("row_{}", line_no + 1)).to_string();
        let old_used      = pf("old_used");
        let old_remaining = pf("old_remaining");
        let new_used      = pf("new_used");
        let new_remaining = pf("new_remaining");
        let crew          = pf("crew_size");
        let hrs           = pf("hrs_per_day");

        // Guard bad capacity
        if crew <= 0.0 || hrs <= 0.0 {
            bad_input_count += 1;
            println!("ANTIBODY=ZERO_CAPACITY_ANTIBODY ID={}", id);
            continue;
        }

        let old_budget   = old_used + old_remaining;
        let new_budget   = new_used + new_remaining;
        let budget_delta = new_budget - old_budget;
        let shift_mhrs   = old_used - new_used;

        if budget_delta > threshold {
            creep_hits.push(CreepHit {
                id: id.clone(),
                budget_delta,
                old_budget,
                new_budget,
                shift_mhrs,
                antibody: "SCOPE_CREEP_ANTIBODY".to_string(),
                alert:    "SAGCO_CREEP_ALERT".to_string(),
            });
            println!("ALERT=SAGCO_CREEP_ALERT ID={} BUDGET_DELTA={:+.1}", id, budget_delta);
        } else if budget_delta < -threshold {
            reduction_count += 1;
        } else if shift_mhrs.abs() > threshold {
            reclass_count += 1;
        } else {
            clean_count += 1;
        }
    }

    let creep_count = creep_hits.len();
    let timestamp   = Utc::now().to_rfc3339();

    // Seal the hit set
    let seal_input: String = creep_hits.iter()
        .map(|h| format!("{}{:.4}", h.id, h.budget_delta))
        .collect::<Vec<_>>().join("|");
    let seal = format!("{:x}", Sha256::digest(seal_input.as_bytes()));

    let overall_status = if creep_hits.is_empty() {
        "SAGCO_CREEP_WATCH_CLEAN"
    } else {
        "SAGCO_CREEP_WATCH_ALERTS"
    };

    if json_mode {
        let report = json!({
            "opcode":     "CREEP_WATCH",
            "timestamp":  timestamp,
            "input":      input_path,
            "threshold":  threshold,
            "totals": {
                "rows_processed":    total,
                "bad_input":         bad_input_count,
                "scope_creep_hits":  creep_count,
                "scope_reductions":  reduction_count,
                "reclassifications": reclass_count,
                "clean":             clean_count,
            },
            "hits":   creep_hits,
            "seal":   seal,
            "status": overall_status,
        });
        let report_text = serde_json::to_string_pretty(&report).unwrap();
        fs::create_dir_all("reports").ok();
        let ts  = Utc::now().format("%Y%m%d_%H%M%S");
        let out = format!("reports/creep_watch_{}.json", ts);
        fs::write(&out, &report_text).ok();
        println!("REPORT={}", out);
    }

    // Ledger append
    fs::create_dir_all("data").ok();
    let ledger_line = serde_json::to_string(&json!({
        "opcode":           "CREEP_WATCH",
        "timestamp":        timestamp,
        "input":            input_path,
        "rows_processed":   total,
        "scope_creep_hits": creep_count,
        "seal":             seal,
        "status":           overall_status,
    })).unwrap() + "\n";
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/creep_watch_ledger.jsonl")
    {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    println!("---");
    println!("ROWS_PROCESSED={}", total);
    println!("SCOPE_CREEP_HITS={}", creep_count);
    println!("SCOPE_REDUCTIONS={}", reduction_count);
    println!("RECLASSIFICATIONS={}", reclass_count);
    println!("CLEAN={}", clean_count);
    println!("BAD_INPUT={}", bad_input_count);
    println!("SEAL={}", seal);
    println!("STATUS={}", overall_status);

    if !creep_hits.is_empty() { std::process::exit(3); }
}
