/// sagco-chainverify — Blue Team opcode: verify ALL ledger chains
/// Scans data/ for every *.jsonl ledger, verifies prev_seal links are intact.
/// USE: sagco-chainverify [--ledger <specific.jsonl>]
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{env, fs, io::Write};

#[derive(serde::Serialize)]
struct ChainReport {
    ledger:       String,
    total_entries: usize,
    broken_links:  usize,
    intact:        bool,
}

fn verify_chain(path: &str) -> ChainReport {
    let content = fs::read_to_string(path).unwrap_or_default();
    let entries: Vec<serde_json::Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    let total = entries.len();
    if total == 0 {
        return ChainReport { ledger: path.to_string(), total_entries: 0, broken_links: 0, intact: true };
    }

    let mut broken = 0usize;
    let mut prev   = "GENESIS".to_string();

    for entry in &entries {
        let ps = entry["prev_seal"].as_str().unwrap_or("");
        // Only check chains that actually use prev_seal (master_ledger does; others seal only)
        if !ps.is_empty() && ps != &prev {
            broken += 1;
        }
        if let Some(s) = entry["seal"].as_str() {
            prev = s.to_string();
        }
    }

    ChainReport {
        ledger: path.to_string(),
        total_entries: total,
        broken_links:  broken,
        intact:        broken == 0,
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let specific: Option<String> = args.windows(2)
        .find(|w| w[0] == "--ledger")
        .map(|w| w[1].clone());

    println!("=== SAGCO-CHAINVERIFY v1 ===");

    let ledgers: Vec<String> = if let Some(p) = specific {
        vec![p]
    } else {
        // Auto-discover all *.jsonl in data/
        match fs::read_dir("data") {
            Ok(entries) => entries
                .flatten()
                .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
                .map(|e| e.path().to_string_lossy().to_string())
                .collect(),
            Err(_) => {
                println!("ANTIBODY=NO_DATA_DIR_ANTIBODY");
                println!("STATUS=SAGCO_CHAINVERIFY_NO_DATA");
                std::process::exit(2);
            }
        }
    };

    if ledgers.is_empty() {
        println!("ANTIBODY=NO_LEDGERS_ANTIBODY");
        println!("STATUS=SAGCO_CHAINVERIFY_EMPTY");
        std::process::exit(2);
    }

    let mut reports: Vec<ChainReport> = Vec::new();
    let mut all_intact = true;

    for ledger_path in &ledgers {
        let r = verify_chain(ledger_path);
        println!(
            "LEDGER={} ENTRIES={} BROKEN={} INTACT={}",
            r.ledger, r.total_entries, r.broken_links, r.intact
        );
        if !r.intact { all_intact = false; }
        reports.push(r);
    }

    let timestamp  = Utc::now().to_rfc3339();
    let seal_input = reports.iter()
        .map(|r| format!("{}{}{}", r.ledger, r.total_entries, r.broken_links))
        .collect::<Vec<_>>().join("|");
    let seal = format!("{:x}", Sha256::digest(seal_input.as_bytes()));

    let overall = if all_intact { "SAGCO_CHAIN_VERIFIED" } else { "SAGCO_CHAIN_BROKEN" };

    let report = json!({
        "opcode":      "CHAINVERIFY",
        "timestamp":   timestamp,
        "ledgers_checked": reports.len(),
        "all_intact":  all_intact,
        "chains":      reports,
        "seal":        seal,
        "status":      overall,
    });

    let report_text = serde_json::to_string_pretty(&report).unwrap();
    fs::create_dir_all("reports").ok();
    let ts  = Utc::now().format("%Y%m%d_%H%M%S");
    let out = format!("reports/chainverify_{}.json", ts);
    fs::write(&out, &report_text).ok();

    // Self-append to master ledger
    fs::create_dir_all("data").ok();
    let ledger_line = serde_json::to_string(&json!({
        "opcode":    "CHAINVERIFY",
        "timestamp": timestamp,
        "ledgers":   ledgers.len(),
        "all_intact": all_intact,
        "report":    out,
        "seal":      seal,
        "status":    overall,
    })).unwrap() + "\n";
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/chainverify_ledger.jsonl")
    {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    println!("---");
    println!("LEDGERS_CHECKED={}", ledgers.len());
    println!("ALL_INTACT={}", all_intact);
    println!("SEAL={}", seal);
    println!("REPORT={}", out);
    println!("STATUS={}", overall);

    if !all_intact { std::process::exit(3); }
}
