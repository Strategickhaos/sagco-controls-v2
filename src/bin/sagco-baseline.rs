/// sagco-baseline — Blue Team opcode: idempotency + repeatability probe
/// Re-runs detect on a previous reclass JSON report and confirms delta = 0.
/// USE: sagco-baseline <reports/reclass_TIMESTAMP.json>
///
/// Pass: report re-runs to the same result → SAGCO_IDEMPOTENCY_PASS
/// Fail: result drifted from what was sealed → SAGCO_IDEMPOTENCY_DRIFT
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{env, fs, io::Write};

fn seal(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn recompute(old_used: f64, old_remaining: f64,
             new_used: f64, new_remaining: f64,
             crew: f64, hrs: f64) -> Option<serde_json::Value> {
    if crew <= 0.0 || hrs <= 0.0 { return None; }
    let old_budget   = old_used + old_remaining;
    let new_budget   = new_used + new_remaining;
    let budget_delta = new_budget - old_budget;
    let daily_cap    = crew * hrs;
    let shift_mhrs   = old_used - new_used;
    let old_days     = old_remaining / daily_cap;
    let new_days     = new_remaining / daily_cap;
    let day_var      = new_days - old_days;

    let (status, antibody) = if budget_delta > 0.001 {
        ("SAGCO_SCOPE_CREEP_DETECTED",      "SCOPE_CREEP_ANTIBODY")
    } else if budget_delta < -0.001 {
        ("SAGCO_SCOPE_REDUCTION_DETECTED",  "SCOPE_REDUCTION_ANTIBODY")
    } else if shift_mhrs.abs() > 0.001 {
        ("SAGCO_RECLASSIFICATION_DETECTED", "NONE_SCOPE_CREEP")
    } else {
        ("SAGCO_NO_CHANGE_DETECTED",        "NONE")
    };

    Some(json!({
        "old_budget":     old_budget,
        "new_budget":     new_budget,
        "budget_delta":   budget_delta,
        "shift_mhrs":     shift_mhrs,
        "daily_capacity": daily_cap,
        "old_days_left":  old_days,
        "new_days_left":  new_days,
        "day_variance":   day_var,
        "antibody":       antibody,
        "status":         status,
    }))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("USE: sagco-baseline <reports/reclass_TIMESTAMP.json>");
        std::process::exit(1);
    }
    let report_path = &args[1];

    println!("=== SAGCO-BASELINE v1 ===");
    println!("REPORT={}", report_path);

    // Load previous report
    let raw = match fs::read_to_string(report_path) {
        Ok(r) => r,
        Err(e) => {
            println!("ANTIBODY=BASELINE_READ_ANTIBODY");
            println!("ERROR={}", e);
            println!("STATUS=SAGCO_BASELINE_FAIL");
            std::process::exit(2);
        }
    };

    let v: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(j) => j,
        Err(_) => {
            println!("ANTIBODY=BASELINE_PARSE_ANTIBODY");
            println!("STATUS=SAGCO_BASELINE_FAIL");
            std::process::exit(2);
        }
    };

    // Extract original inputs
    let inp = &v["input"];
    let res = &v["result"];

    let pf = |key: &str| inp[key].as_f64().unwrap_or(0.0);
    let old_used      = pf("old_used");
    let old_remaining = pf("old_remaining");
    let new_used      = pf("new_used");
    let new_remaining = pf("new_remaining");
    let crew          = pf("crew_size");
    let hrs           = pf("hrs_per_day");

    // Guard
    if crew <= 0.0 || hrs <= 0.0 {
        println!("ANTIBODY=ZERO_CAPACITY_ANTIBODY");
        println!("STATUS=SAGCO_BAD_CAPACITY_FAIL");
        std::process::exit(2);
    }

    let rerun = match recompute(old_used, old_remaining, new_used, new_remaining, crew, hrs) {
        Some(r) => r,
        None => {
            println!("ANTIBODY=BASELINE_COMPUTE_ANTIBODY");
            println!("STATUS=SAGCO_BASELINE_FAIL");
            std::process::exit(2);
        }
    };

    // Compare original sealed status vs recomputed status
    let orig_status   = res["status"].as_str().unwrap_or("").to_string();
    let rerun_status  = rerun["status"].as_str().unwrap_or("").to_string();
    let orig_antibody = res["antibody"].as_str().unwrap_or("").to_string();
    let rerun_antibody = rerun["antibody"].as_str().unwrap_or("").to_string();

    let orig_delta:  f64 = res["budget_delta"].as_f64().unwrap_or(f64::NAN);
    let rerun_delta: f64 = rerun["budget_delta"].as_f64().unwrap_or(f64::NAN);
    let delta_drift = (orig_delta - rerun_delta).abs();

    let idempotent = orig_status == rerun_status
        && orig_antibody == rerun_antibody
        && delta_drift < 0.0001;

    let status = if idempotent { "SAGCO_IDEMPOTENCY_PASS" } else { "SAGCO_IDEMPOTENCY_DRIFT" };

    let timestamp = Utc::now().to_rfc3339();
    let report = json!({
        "opcode":     "BASELINE",
        "timestamp":  timestamp,
        "source_report": report_path,
        "original": {
            "status":       orig_status,
            "antibody":     orig_antibody,
            "budget_delta": orig_delta,
        },
        "rerun": {
            "status":       rerun_status,
            "antibody":     rerun_antibody,
            "budget_delta": rerun_delta,
        },
        "delta_drift": delta_drift,
        "idempotent":  idempotent,
        "status":      status,
    });

    let report_text = serde_json::to_string_pretty(&report).unwrap();
    let report_seal = seal(&report_text);

    fs::create_dir_all("reports").ok();
    let ts = Utc::now().format("%Y%m%d_%H%M%S");
    let out = format!("reports/baseline_{}.json", ts);
    fs::write(&out, &report_text).ok();

    fs::create_dir_all("data").ok();
    let ledger_line = serde_json::to_string(&json!({
        "opcode":    "BASELINE",
        "timestamp": timestamp,
        "source":    report_path,
        "idempotent": idempotent,
        "report":    out,
        "seal":      report_seal,
        "status":    status,
    })).unwrap() + "\n";
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/baseline_ledger.jsonl")
    {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    println!("ORIGINAL_STATUS={}", orig_status);
    println!("RERUN_STATUS={}", rerun_status);
    println!("DELTA_DRIFT={:.6}", delta_drift);
    println!("IDEMPOTENT={}", idempotent);
    println!("SEAL={}", report_seal);
    println!("REPORT={}", out);
    println!("STATUS={}", status);

    if !idempotent { std::process::exit(3); }
}
