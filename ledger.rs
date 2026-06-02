use std::fs::{self, OpenOptions};
use std::io::Write;
use chrono::Utc;
use serde_json::json;

const LEDGER_PATH: &str = "data/master_ledger.jsonl";

/// Reads the hash of the last entry in the ledger (for chaining).
/// Returns "GENESIS" if the ledger is empty or missing.
fn last_hash() -> String {
    let content = fs::read_to_string(LEDGER_PATH).unwrap_or_default();
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .last()
        .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .and_then(|v| v["seal"].as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "GENESIS".to_string())
}

/// Appends one sealed entry to the ledger.
pub fn commit(
    status: &str,
    antibody: &str,
    shift_mhrs: f64,
    day_variance: f64,
    json_path: &str,
    md_path: &str,
    report_seal: &str,
) {
    fs::create_dir_all("data").expect("ledger: cannot create data/");

    let prev_hash = last_hash();
    let timestamp = Utc::now().to_rfc3339();

    let entry = json!({
        "timestamp":    timestamp,
        "status":       status,
        "antibody":     antibody,
        "shift_mhrs":   shift_mhrs,
        "day_variance": day_variance,
        "artifacts": {
            "json": json_path,
            "md":   md_path,
        },
        "seal":      report_seal,
        "prev_seal": prev_hash,
    });

    let line = serde_json::to_string(&entry).unwrap() + "\n";

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LEDGER_PATH)
        .expect("ledger: cannot open master_ledger.jsonl");

    file.write_all(line.as_bytes())
        .expect("ledger: write failed");
}

/// Verifies the entire chain — each entry's prev_seal must match
/// the seal of the entry before it.
pub fn verify_chain() -> (usize, usize, bool) {
    let content = fs::read_to_string(LEDGER_PATH).unwrap_or_default();
    let entries: Vec<serde_json::Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    let total = entries.len();
    if total == 0 {
        return (0, 0, true);
    }

    let mut broken = 0;
    let mut prev = "GENESIS".to_string();

    for entry in &entries {
        let ps = entry["prev_seal"].as_str().unwrap_or("");
        if ps != prev {
            broken += 1;
        }
        prev = entry["seal"].as_str().unwrap_or("").to_string();
    }

    (total, broken, broken == 0)
}
