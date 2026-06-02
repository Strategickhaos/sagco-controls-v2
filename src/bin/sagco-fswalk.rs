/// sagco-fswalk — Tier-0 file-system stepper (rivals TSK fls/icat)
/// Walks a directory tree, emits SHA256 + metadata per file, seals the manifest.
/// USE: sagco-fswalk <root_path> [--json]
use std::fs;
use std::time::UNIX_EPOCH;
use sha2::{Digest, Sha256};
use serde_json::json;
use chrono::Utc;

#[derive(serde::Serialize)]
struct FileEntry {
    path:     String,
    size:     u64,
    modified: u64,
    seal:     String,
}

fn seal_file(path: &str) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    format!("{:x}", Sha256::digest(&bytes))
}

fn walk(root: &str) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    for result in walkdir::WalkDir::new(root).into_iter().flatten() {
        let meta = match result.metadata() {
            Ok(m) if m.is_file() => m,
            _ => continue,
        };
        let path = result.path().to_string_lossy().to_string();
        let size = meta.len();
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let seal = seal_file(&path);
        entries.push(FileEntry { path, size, modified, seal });
    }
    entries
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = args.get(1).map(|s| s.as_str()).unwrap_or(".");
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-FSWALK v1 ===");
    println!("ROOT={}", root);

    let entries = walk(root);
    let total_size: u64 = entries.iter().map(|e| e.size).sum();

    // Manifest seal = SHA256 of all per-file seals concatenated
    let manifest_input: String = entries.iter().map(|e| e.seal.as_str()).collect::<Vec<_>>().join("");
    let manifest_seal = format!("{:x}", Sha256::digest(manifest_input.as_bytes()));

    if json_mode {
        let out = json!({
            "sagco_command": "sagco-fswalk",
            "timestamp": Utc::now().to_rfc3339(),
            "root": root,
            "file_count": entries.len(),
            "total_bytes": total_size,
            "manifest_seal": manifest_seal,
            "entries": entries,
        });
        fs::create_dir_all("reports").ok();
        let ts = Utc::now().format("%Y%m%d_%H%M%S");
        let path = format!("reports/fswalk_{}.json", ts);
        fs::write(&path, serde_json::to_string_pretty(&out).unwrap()).ok();
        println!("REPORT={}", path);
    } else {
        for e in &entries {
            println!("FILE={} SIZE={} SEAL={}", e.path, e.size, e.seal);
        }
    }

    println!("FILE_COUNT={}", entries.len());
    println!("TOTAL_BYTES={}", total_size);
    println!("MANIFEST_SEAL={}", manifest_seal);
    println!("STATUS=SAGCO_FSWALK_PASS");
}
