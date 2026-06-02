/// sagco-binscan — Tier-1 binary entropy scanner (rivals Ghidra headless / radare2)
/// Reads any binary, computes Shannon entropy per 256-byte block, flags packed/encrypted regions.
/// High entropy (>7.2) = likely packed, encrypted, or compressed.
/// USE: sagco-binscan <file> [--threshold 7.2] [--json]
use std::fs;
use sha2::{Digest, Sha256};
use serde_json::json;
use chrono::Utc;

#[derive(serde::Serialize)]
struct Block {
    offset:  usize,
    entropy: f64,
    flag:    String,
}

fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() { return 0.0; }
    let mut freq = [0u64; 256];
    for &b in data { freq[b as usize] += 1; }
    let len = data.len() as f64;
    freq.iter()
        .filter(|&&c| c > 0)
        .map(|&c| { let p = c as f64 / len; -p * p.log2() })
        .sum()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("USE: sagco-binscan <file> [--threshold 7.2] [--json]");
        std::process::exit(1);
    }
    let file = &args[1];
    let threshold: f64 = args.windows(2)
        .find(|w| w[0] == "--threshold")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(7.2);
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-BINSCAN v1 ===");
    println!("FILE={}", file);
    println!("ENTROPY_THRESHOLD={:.2}", threshold);

    let data = match fs::read(file) {
        Ok(d) => d,
        Err(e) => {
            println!("ANTIBODY=FILE_READ_ANTIBODY");
            println!("STATUS=SAGCO_BINSCAN_IO_FAIL");
            println!("ERROR={}", e);
            std::process::exit(2);
        }
    };

    let block_size = 256usize;
    let mut blocks: Vec<Block> = data.chunks(block_size).enumerate().map(|(i, chunk)| {
        let e = shannon_entropy(chunk);
        Block {
            offset:  i * block_size,
            entropy: (e * 1000.0).round() / 1000.0,
            flag: if e >= threshold { "HIGH_ENTROPY".to_string() } else { "NORMAL".to_string() },
        }
    }).collect();

    let high_count = blocks.iter().filter(|b| b.flag == "HIGH_ENTROPY").count();
    let avg_entropy: f64 = if blocks.is_empty() { 0.0 } else {
        blocks.iter().map(|b| b.entropy).sum::<f64>() / blocks.len() as f64
    };
    let file_seal = format!("{:x}", Sha256::digest(&data));

    let status = if high_count == 0 {
        "SAGCO_BINSCAN_CLEAN"
    } else if high_count as f64 / blocks.len() as f64 > 0.5 {
        "SAGCO_BINSCAN_PACKED_DETECTED"
    } else {
        "SAGCO_BINSCAN_PARTIAL_ENTROPY"
    };

    if json_mode {
        // Truncate block list to first 500 to keep reports manageable
        blocks.truncate(500);
        let out = json!({
            "sagco_command": "sagco-binscan",
            "timestamp": Utc::now().to_rfc3339(),
            "file": file,
            "file_bytes": data.len(),
            "block_size": block_size,
            "threshold": threshold,
            "avg_entropy": (avg_entropy * 1000.0).round() / 1000.0,
            "high_entropy_blocks": high_count,
            "seal": file_seal,
            "status": status,
            "blocks": blocks,
        });
        fs::create_dir_all("reports").ok();
        let ts = Utc::now().format("%Y%m%d_%H%M%S");
        let p = format!("reports/binscan_{}.json", ts);
        fs::write(&p, serde_json::to_string_pretty(&out).unwrap()).ok();
        println!("REPORT={}", p);
    }

    println!("FILE_BYTES={}", data.len());
    println!("AVG_ENTROPY={:.3}", avg_entropy);
    println!("HIGH_ENTROPY_BLOCKS={}", high_count);
    println!("SEAL={}", file_seal);
    println!("STATUS={}", status);
}
