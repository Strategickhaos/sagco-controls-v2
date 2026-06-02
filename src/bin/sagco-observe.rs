/// sagco-observe — Root opcode / first syscall of the SAGCO kernel
/// Every other opcode depends on this contract.
/// USE: sagco-observe <path> [--json]
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{env, fs, io::Write, path::Path};

fn shannon_entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() { return 0.0; }
    let mut freq = [0u64; 256];
    for &b in bytes { freq[b as usize] += 1; }
    let n = bytes.len() as f64;
    // Normalized to 0..1 by dividing by log2(256) = 8
    freq.iter()
        .filter(|&&c| c > 0)
        .map(|&c| { let p = c as f64 / n; -p * p.log2() })
        .sum::<f64>() / 8.0
}

fn mime_hint(bytes: &[u8], path: &str) -> &'static str {
    let magic: &[(&[u8], &str)] = &[
        (b"%PDF",     "application/pdf"),
        (b"PK\x03\x04", "application/zip"),
        (b"\x7fELF",  "application/elf"),
        (b"MZ",       "application/pe"),
        (b"\x1f\x8b", "application/gzip"),
        (b"RIFF",     "audio/video/riff"),
        (b"\x89PNG",  "image/png"),
        (b"\xff\xd8\xff", "image/jpeg"),
    ];
    for (sig, mime) in magic {
        if bytes.starts_with(sig) { return mime; }
    }
    if path.ends_with(".rs")   { return "text/rust"; }
    if path.ends_with(".toml") { return "text/toml"; }
    if path.ends_with(".json") { return "application/json"; }
    if path.ends_with(".md")   { return "text/markdown"; }
    if path.ends_with(".sh")   { return "text/shell"; }
    if path.ends_with(".yaml") || path.ends_with(".yml") { return "text/yaml"; }
    "application/octet-stream"
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("USE: sagco-observe <path> [--json]");
        std::process::exit(1);
    }

    let path   = &args[1];
    let json_mode = args.iter().any(|a| a == "--json");
    let p = Path::new(path);

    if !p.exists() {
        println!("ANTIBODY=OBSERVE_PATH_MISSING_ANTIBODY");
        println!("PATH={}", path);
        println!("STATUS=SAGCO_OBSERVE_FAIL");
        std::process::exit(2);
    }

    let bytes = fs::read(path).unwrap_or_default();
    let byte_count = bytes.len();

    let entropy = shannon_entropy(&bytes);
    let text    = String::from_utf8_lossy(&bytes);
    let tokens  = text.split_whitespace().count();

    let printable: usize = bytes.iter()
        .filter(|b| b.is_ascii_graphic() || **b == b' ' || **b == b'\n' || **b == b'\t')
        .count();
    let printable_ratio = if byte_count > 0 { printable as f64 / byte_count as f64 } else { 0.0 };

    // Evidence score: reward high printable ratio + low entropy (structured text wins)
    let evidence_score = ((printable_ratio * 0.6) + ((1.0 - entropy) * 0.4)).clamp(0.0, 1.0);
    let evidence_score = (evidence_score * 1000.0).round() / 1000.0;

    let mime = mime_hint(&bytes, path);
    let timestamp = Utc::now().to_rfc3339();

    let report = json!({
        "opcode":          "OBSERVE",
        "timestamp":       timestamp,
        "path":            path,
        "bytes":           byte_count,
        "tokens":          tokens,
        "entropy":         (entropy * 1000.0).round() / 1000.0,
        "printable_ratio": (printable_ratio * 1000.0).round() / 1000.0,
        "evidence_score":  evidence_score,
        "mime":            mime,
        "status":          "SAGCO_OBSERVE_PASS"
    });

    let report_text = serde_json::to_string_pretty(&report).unwrap();
    let seal = format!("{:x}", Sha256::digest(report_text.as_bytes()));

    // Write artifact report
    fs::create_dir_all("reports/observe").ok();
    let safe = path.replace(['/', '\\', ' ', ':'], "_");
    let ts   = Utc::now().format("%Y%m%d_%H%M%S");
    let out  = format!("reports/observe/observe_{}_{}.json", ts, safe);
    fs::write(&out, &report_text).ok();

    // Append to observe ledger
    fs::create_dir_all("data").ok();
    let ledger_entry = serde_json::to_string(&json!({
        "opcode":    "OBSERVE",
        "timestamp": timestamp,
        "path":      path,
        "bytes":     byte_count,
        "entropy":   entropy,
        "evidence_score": evidence_score,
        "report":    out,
        "seal":      seal,
    })).unwrap() + "\n";

    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/observe_ledger.jsonl")
    {
        let _ = f.write_all(ledger_entry.as_bytes());
    }

    if json_mode {
        println!("{}", report_text);
    } else {
        println!("=== SAGCO-OBSERVE v1 ===");
        println!("OPCODE=OBSERVE");
        println!("PATH={}", path);
        println!("BYTES={}", byte_count);
        println!("TOKENS={}", tokens);
        println!("ENTROPY={:.3}", entropy);
        println!("PRINTABLE_RATIO={:.3}", printable_ratio);
        println!("EVIDENCE_SCORE={:.3}", evidence_score);
        println!("MIME={}", mime);
    }

    println!("SEAL={}", seal);
    println!("REPORT={}", out);
    println!("LEDGER=data/observe_ledger.jsonl");
    println!("STATUS=SAGCO_OBSERVE_PASS");
}
