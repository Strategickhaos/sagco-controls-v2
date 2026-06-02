/// sagco-verify — Purple Team opcode: compare two reports and measure variance
/// Loads two SAGCO JSON reports (any opcode), diffs key numeric fields,
/// emits a variance score and drift classification.
/// USE: sagco-verify <report_a.json> <report_b.json> [--json]
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::{json, Value};
use std::{env, fs, io::Write};

fn numeric_fields(v: &Value, prefix: &str) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            let full_key = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
            if let Some(n) = val.as_f64() {
                out.push((full_key, n));
            } else if val.is_object() {
                out.extend(numeric_fields(val, &full_key));
            }
        }
    }
    out
}

#[derive(serde::Serialize)]
struct FieldDiff {
    field:    String,
    a:        f64,
    b:        f64,
    delta:    f64,
    pct_diff: f64,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("USE: sagco-verify <report_a.json> <report_b.json> [--json]");
        std::process::exit(1);
    }

    let path_a    = &args[1];
    let path_b    = &args[2];
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-VERIFY v1 ===");
    println!("REPORT_A={}", path_a);
    println!("REPORT_B={}", path_b);

    let load = |p: &str| -> Option<Value> {
        fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok())
    };

    let va = match load(path_a) {
        Some(v) => v,
        None => {
            println!("ANTIBODY=VERIFY_READ_ANTIBODY_A");
            println!("STATUS=SAGCO_VERIFY_FAIL");
            std::process::exit(2);
        }
    };
    let vb = match load(path_b) {
        Some(v) => v,
        None => {
            println!("ANTIBODY=VERIFY_READ_ANTIBODY_B");
            println!("STATUS=SAGCO_VERIFY_FAIL");
            std::process::exit(2);
        }
    };

    let opcode_a = va["opcode"].as_str().unwrap_or("UNKNOWN").to_string();
    let opcode_b = vb["opcode"].as_str().unwrap_or("UNKNOWN").to_string();
    let status_a = va["status"].as_str().unwrap_or("").to_string();
    let status_b = vb["status"].as_str().unwrap_or("").to_string();
    let status_match = status_a == status_b;

    // Numeric field diff
    let fields_a: std::collections::HashMap<String, f64> = numeric_fields(&va, "").into_iter().collect();
    let fields_b: std::collections::HashMap<String, f64> = numeric_fields(&vb, "").into_iter().collect();

    let mut diffs: Vec<FieldDiff> = Vec::new();
    for (k, a) in &fields_a {
        if let Some(&b) = fields_b.get(k) {
            let delta    = b - a;
            let pct_diff = if a.abs() > 1e-9 { (delta / a.abs()) * 100.0 } else { 0.0 };
            if delta.abs() > 1e-9 {
                diffs.push(FieldDiff {
                    field: k.clone(),
                    a: (a * 10000.0).round() / 10000.0,
                    b: (b * 10000.0).round() / 10000.0,
                    delta: (delta * 10000.0).round() / 10000.0,
                    pct_diff: (pct_diff * 100.0).round() / 100.0,
                });
            }
        }
    }

    // Max absolute delta across all fields = variance score
    let variance_score: f64 = diffs.iter()
        .map(|d| d.delta.abs())
        .fold(0.0f64, f64::max);

    let drift_class = if diffs.is_empty() {
        "SAGCO_VERIFY_IDENTICAL"
    } else if variance_score < 0.01 {
        "SAGCO_VERIFY_NEGLIGIBLE_DRIFT"
    } else if variance_score < 1.0 {
        "SAGCO_VERIFY_MINOR_DRIFT"
    } else if variance_score < 50.0 {
        "SAGCO_VERIFY_SIGNIFICANT_DRIFT"
    } else {
        "SAGCO_VERIFY_MAJOR_DRIFT"
    };

    let timestamp  = Utc::now().to_rfc3339();
    let seal_input = format!("{}{}{:.4}", path_a, path_b, variance_score);
    let seal       = format!("{:x}", Sha256::digest(seal_input.as_bytes()));

    if json_mode || !diffs.is_empty() {
        for d in &diffs {
            println!("DIFF field={} a={} b={} delta={:+.4} pct={:+.2}%",
                d.field, d.a, d.b, d.delta, d.pct_diff);
        }
    }

    if json_mode {
        let report = json!({
            "opcode":     "VERIFY",
            "timestamp":  timestamp,
            "report_a":   path_a,
            "report_b":   path_b,
            "opcode_a":   opcode_a,
            "opcode_b":   opcode_b,
            "status_match": status_match,
            "field_diffs":  diffs,
            "variance_score": (variance_score * 10000.0).round() / 10000.0,
            "drift_class":  drift_class,
            "seal":         seal,
        });
        let report_text = serde_json::to_string_pretty(&report).unwrap();
        fs::create_dir_all("reports").ok();
        let ts  = Utc::now().format("%Y%m%d_%H%M%S");
        let out = format!("reports/verify_{}.json", ts);
        fs::write(&out, &report_text).ok();
        println!("REPORT={}", out);
    }

    // Ledger
    fs::create_dir_all("data").ok();
    let ledger_line = serde_json::to_string(&json!({
        "opcode":         "VERIFY",
        "timestamp":      timestamp,
        "report_a":       path_a,
        "report_b":       path_b,
        "variance_score": variance_score,
        "drift_class":    drift_class,
        "seal":           seal,
    })).unwrap() + "\n";
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/verify_ledger.jsonl")
    {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    println!("OPCODE_A={}", opcode_a);
    println!("OPCODE_B={}", opcode_b);
    println!("STATUS_MATCH={}", status_match);
    println!("FIELDS_DIFFED={}", diffs.len());
    println!("VARIANCE_SCORE={:.4}", variance_score);
    println!("SEAL={}", seal);
    println!("STATUS={}", drift_class);
}
