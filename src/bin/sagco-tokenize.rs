/// sagco-tokenize — Opcode 2: consume OBSERVE report, emit structured tokens
/// Reads an observe_*.json report OR a raw file path, classifies token kinds.
/// USE: sagco-tokenize <path_or_observe_report.json> [--json]
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{env, fs, io::Write};
use regex::Regex;

#[derive(serde::Serialize, Default)]
struct TokenSet {
    words:    Vec<String>,
    numbers:  Vec<String>,
    urls:     Vec<String>,
    emails:   Vec<String>,
    hex_seals: Vec<String>,
    opcodes:  Vec<String>,    // SAGCO_* tokens
    keys:     Vec<String>,    // KEY=VALUE pairs
}

fn tokenize_text(text: &str) -> TokenSet {
    let re_url    = Regex::new(r#"https?://[^\s"'<>]+"#).unwrap();
    let re_email  = Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap();
    let re_hex    = Regex::new(r"\b[0-9a-fA-F]{40,}\b").unwrap();
    let re_opcode = Regex::new(r"\bSAGCO_[A-Z_]+\b").unwrap();
    let re_kv     = Regex::new(r"\b([A-Z_]{2,})=([^\s]+)").unwrap();
    let re_num    = Regex::new(r"\b\d+(?:\.\d+)?\b").unwrap();

    let dedup = |v: Vec<String>| -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        v.into_iter().filter(|s| seen.insert(s.clone())).collect()
    };

    let urls    = dedup(re_url.find_iter(text).map(|m| m.as_str().to_string()).collect());
    let emails  = dedup(re_email.find_iter(text).map(|m| m.as_str().to_string()).collect());
    let hex     = dedup(re_hex.find_iter(text).map(|m| m.as_str().to_string()).collect());
    let opcodes = dedup(re_opcode.find_iter(text).map(|m| m.as_str().to_string()).collect());
    let keys    = dedup(re_kv.find_iter(text).map(|m| m.as_str().to_string()).collect());
    let numbers = dedup(re_num.find_iter(text).map(|m| m.as_str().to_string()).collect());

    // Words = whitespace tokens not captured by above
    let words: Vec<String> = dedup(
        text.split_whitespace()
            .filter(|w| w.len() >= 3 && w.chars().all(|c| c.is_alphabetic()))
            .map(|w| w.to_lowercase())
            .collect()
    );

    TokenSet { words, numbers, urls, emails, hex_seals: hex, opcodes, keys }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("USE: sagco-tokenize <path_or_observe_report.json> [--json]");
        std::process::exit(1);
    }

    let input     = &args[1];
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-TOKENIZE v1 ===");

    // Accept either a raw file or a previous OBSERVE JSON report
    let (target_path, source_bytes) = {
        let raw = fs::read(input).unwrap_or_default();
        // If it parses as an OBSERVE report, pull the path from it
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw) {
            if v["opcode"].as_str() == Some("OBSERVE") {
                let obs_path = v["path"].as_str().unwrap_or(input).to_string();
                let bytes = fs::read(&obs_path).unwrap_or_else(|_| raw.clone());
                (obs_path, bytes)
            } else {
                (input.clone(), raw)
            }
        } else {
            (input.clone(), raw)
        }
    };

    let text   = String::from_utf8_lossy(&source_bytes);
    let tokens = tokenize_text(&text);

    let token_count = tokens.words.len()
        + tokens.numbers.len()
        + tokens.urls.len()
        + tokens.emails.len()
        + tokens.hex_seals.len()
        + tokens.opcodes.len()
        + tokens.keys.len();

    let timestamp = Utc::now().to_rfc3339();

    let report = json!({
        "opcode":       "TOKENIZE",
        "timestamp":    timestamp,
        "input":        target_path,
        "token_count":  token_count,
        "token_kinds": {
            "words":     tokens.words.len(),
            "numbers":   tokens.numbers.len(),
            "urls":      tokens.urls.len(),
            "emails":    tokens.emails.len(),
            "hex_seals": tokens.hex_seals.len(),
            "opcodes":   tokens.opcodes.len(),
            "keys":      tokens.keys.len(),
        },
        "tokens": tokens,
        "status": "SAGCO_TOKENIZE_PASS"
    });

    let report_text = serde_json::to_string_pretty(&report).unwrap();
    let seal = format!("{:x}", Sha256::digest(report_text.as_bytes()));

    fs::create_dir_all("reports/tokenize").ok();
    let safe = target_path.replace(['/', '\\', ' ', ':'], "_");
    let ts   = Utc::now().format("%Y%m%d_%H%M%S");
    let out  = format!("reports/tokenize/tokenize_{}_{}.json", ts, safe);
    fs::write(&out, &report_text).ok();

    fs::create_dir_all("data").ok();
    let ledger_line = serde_json::to_string(&json!({
        "opcode":      "TOKENIZE",
        "timestamp":   timestamp,
        "input":       target_path,
        "token_count": token_count,
        "report":      out,
        "seal":        seal,
    })).unwrap() + "\n";
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/tokenize_ledger.jsonl")
    {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    if json_mode {
        println!("{}", report_text);
    } else {
        println!("INPUT={}", target_path);
        println!("TOKEN_COUNT={}", token_count);
        println!("WORDS={}", tokens.words.len());
        println!("NUMBERS={}", tokens.numbers.len());
        println!("URLS={}", tokens.urls.len());
        println!("OPCODES={}", tokens.opcodes.len());
        println!("HEX_SEALS={}", tokens.hex_seals.len());
        println!("KEYS={}", tokens.keys.len());
    }

    println!("SEAL={}", seal);
    println!("REPORT={}", out);
    println!("STATUS=SAGCO_TOKENIZE_PASS");
}
