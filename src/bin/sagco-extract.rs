/// sagco-extract — Tier-0 artifact extractor (rivals Bulk Extractor)
/// Scans any file for emails, URLs, IPv4s, hex strings — no mounting needed.
/// USE: sagco-extract <target_file_or_dir> [--json]
use std::fs;
use regex::Regex;
use serde_json::json;
use sha2::{Digest, Sha256};
use chrono::Utc;

#[derive(serde::Serialize, Default)]
struct Artifacts {
    emails:   Vec<String>,
    urls:     Vec<String>,
    ipv4s:    Vec<String>,
    hex_blobs: Vec<String>,
}

fn extract_from_bytes(data: &[u8]) -> Artifacts {
    // Safe UTF-8 decode — forensic files may be binary
    let text = String::from_utf8_lossy(data);

    let re_email = Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap();
    let re_url   = Regex::new(r#"https?://[^\s"'<>]+"#).unwrap();
    let re_ip    = Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap();
    let re_hex   = Regex::new(r"[0-9a-fA-F]{32,}").unwrap();

    let dedup = |v: Vec<String>| -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        v.into_iter().filter(|s| seen.insert(s.clone())).collect()
    };

    Artifacts {
        emails:    dedup(re_email.find_iter(&text).map(|m| m.as_str().to_string()).collect()),
        urls:      dedup(re_url.find_iter(&text).map(|m| m.as_str().to_string()).collect()),
        ipv4s:     dedup(re_ip.find_iter(&text).map(|m| m.as_str().to_string()).collect()),
        hex_blobs: dedup(re_hex.find_iter(&text).map(|m| m.as_str().to_string()).collect()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let target = args.get(1).map(|s| s.as_str()).unwrap_or(".");
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-EXTRACT v1 ===");
    println!("TARGET={}", target);

    let mut all = Artifacts::default();
    let mut file_count = 0usize;

    let path = std::path::Path::new(target);
    let files: Vec<std::path::PathBuf> = if path.is_dir() {
        walkdir::WalkDir::new(target)
            .into_iter()
            .flatten()
            .filter(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
            .map(|e| e.path().to_path_buf())
            .collect()
    } else {
        vec![path.to_path_buf()]
    };

    for f in &files {
        let data = fs::read(f).unwrap_or_default();
        let arts = extract_from_bytes(&data);
        all.emails.extend(arts.emails);
        all.urls.extend(arts.urls);
        all.ipv4s.extend(arts.ipv4s);
        all.hex_blobs.extend(arts.hex_blobs);
        file_count += 1;
    }

    // Seal the artifact set
    let seal_input = format!("{:?}{:?}{:?}", all.emails, all.urls, all.ipv4s);
    let seal = format!("{:x}", Sha256::digest(seal_input.as_bytes()));

    if json_mode {
        let out = json!({
            "sagco_command": "sagco-extract",
            "timestamp": Utc::now().to_rfc3339(),
            "target": target,
            "file_count": file_count,
            "artifacts": all,
            "seal": seal,
        });
        fs::create_dir_all("reports").ok();
        let ts = Utc::now().format("%Y%m%d_%H%M%S");
        let p = format!("reports/extract_{}.json", ts);
        fs::write(&p, serde_json::to_string_pretty(&out).unwrap()).ok();
        println!("REPORT={}", p);
    } else {
        println!("EMAILS={}", all.emails.len());
        println!("URLS={}", all.urls.len());
        println!("IPV4S={}", all.ipv4s.len());
        println!("HEX_BLOBS={}", all.hex_blobs.len());
    }

    println!("FILES_SCANNED={}", file_count);
    println!("SEAL={}", seal);
    println!("STATUS=SAGCO_EXTRACT_PASS");
}
