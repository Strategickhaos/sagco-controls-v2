/// sagco-diff — Compare any two SAGCO ledger states or report files
/// Shows what changed between two points in time.
/// USE: sagco-diff <ledger_a.jsonl> <ledger_b.jsonl>
///      sagco-diff --ledger <name> --from <entry_n> --to <entry_m>
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::{json, Value};
use std::{env, fs, io::Write};

fn seal(s: &str) -> String { format!("{:x}", Sha256::digest(s.as_bytes())) }

fn load_jsonl(path: &str) -> Vec<Value> {
    fs::read_to_string(path).unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn numeric_fields(v: &Value, prefix: &str) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            let key = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
            if let Some(n) = val.as_f64() { out.push((key, n)); }
            else if val.is_object() { out.extend(numeric_fields(val, &key)); }
        }
    }
    out
}

fn diff_entry(a: &Value, b: &Value) -> Vec<String> {
    let fa: std::collections::HashMap<_,_> = numeric_fields(a, "").into_iter().collect();
    let fb: std::collections::HashMap<_,_> = numeric_fields(b, "").into_iter().collect();
    let mut diffs = Vec::new();
    for (k, va) in &fa {
        if let Some(&vb) = fb.get(k) {
            let delta = vb - va;
            if delta.abs() > 1e-6 {
                diffs.push(format!("  FIELD={} A={:.3} B={:.3} DELTA={:+.3}", k, va, vb, delta));
            }
        }
    }
    diffs
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        println!("USE: sagco-diff <file_a> <file_b>");
        println!("     sagco-diff data/master_ledger.jsonl data/forecast_ledger.jsonl");
        std::process::exit(1);
    }

    let path_a = &args[1];
    let path_b = &args[2];

    println!("=== SAGCO-DIFF v1 ===");
    println!("A={}", path_a);
    println!("B={}", path_b);

    let entries_a = load_jsonl(path_a);
    let entries_b = load_jsonl(path_b);

    println!("ENTRIES_A={}", entries_a.len());
    println!("ENTRIES_B={}", entries_b.len());
    println!("ENTRY_DELTA={:+}", entries_b.len() as i64 - entries_a.len() as i64);
    println!("");

    // Status counts
    let count_status = |entries: &[Value], key: &str| -> std::collections::HashMap<String,usize> {
        let mut m = std::collections::HashMap::new();
        for e in entries {
            if let Some(v) = e[key].as_str() { *m.entry(v.to_string()).or_insert(0) += 1; }
        }
        m
    };

    let statuses_a = count_status(&entries_a, "status");
    let statuses_b = count_status(&entries_b, "status");

    println!("--- STATUS DISTRIBUTION ---");
    let mut all_statuses: std::collections::HashSet<&String> = statuses_a.keys().collect();
    all_statuses.extend(statuses_b.keys());
    let mut sorted: Vec<_> = all_statuses.into_iter().collect();
    sorted.sort();
    for s in &sorted {
        let ca = statuses_a.get(*s).copied().unwrap_or(0);
        let cb = statuses_b.get(*s).copied().unwrap_or(0);
        println!("  {} A={} B={} DELTA={:+}", s, ca, cb, cb as i64 - ca as i64);
    }

    // Opcode counts
    println!("");
    println!("--- OPCODE DISTRIBUTION ---");
    let opcodes_a = count_status(&entries_a, "opcode");
    let opcodes_b = count_status(&entries_b, "opcode");
    let mut all_ops: std::collections::HashSet<&String> = opcodes_a.keys().collect();
    all_ops.extend(opcodes_b.keys());
    let mut ops: Vec<_> = all_ops.into_iter().collect();
    ops.sort();
    for op in &ops {
        let ca = opcodes_a.get(*op).copied().unwrap_or(0);
        let cb = opcodes_b.get(*op).copied().unwrap_or(0);
        println!("  {} A={} B={} DELTA={:+}", op, ca, cb, cb as i64 - ca as i64);
    }

    // Numeric field drift across last entry in each
    if let (Some(last_a), Some(last_b)) = (entries_a.last(), entries_b.last()) {
        let field_diffs = diff_entry(last_a, last_b);
        if !field_diffs.is_empty() {
            println!("");
            println!("--- NUMERIC FIELD DRIFT (last entry) ---");
            for d in &field_diffs { println!("{}", d); }
        }
    }

    let timestamp = Utc::now().to_rfc3339();
    let seal_v = seal(&format!("{}{}{}{}", path_a, path_b, entries_a.len(), entries_b.len()));

    // Write report
    let report = json!({
        "opcode":     "DIFF",
        "timestamp":  timestamp,
        "path_a":     path_a,
        "path_b":     path_b,
        "entries_a":  entries_a.len(),
        "entries_b":  entries_b.len(),
        "entry_delta": entries_b.len() as i64 - entries_a.len() as i64,
        "seal":       seal_v,
        "status":     "SAGCO_DIFF_PASS",
    });

    fs::create_dir_all("reports").ok();
    let ts  = Utc::now().format("%Y%m%d_%H%M%S");
    let out = format!("reports/diff_{}.json", ts);
    fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).ok();

    fs::create_dir_all("data").ok();
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open("data/diff_ledger.jsonl") {
        let _ = f.write_all((serde_json::to_string(&report).unwrap() + "\n").as_bytes());
    }

    println!("");
    println!("SEAL={}", seal_v);
    println!("REPORT={}", out);
    println!("STATUS=SAGCO_DIFF_PASS");
}
