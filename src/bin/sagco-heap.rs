/// sagco-heap — Chrome DevTools heap snapshot analyzer + differ
/// Takes 1-3 .heapsnapshot files, computes growth deltas, extracts
/// high-value strings (URLs, tokens, function names), seals each.
///
/// USE:
///   sagco-heap <snap.heapsnapshot>                  → single observe
///   sagco-heap <snap1.heapsnapshot> <snap2.heapsnapshot>  → diff
///   sagco-heap --dir <folder>                        → diff all .heapsnapshots in order
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{env, fs, io::Write};
use regex::Regex;

fn seal(s: &str) -> String { format!("{:x}", Sha256::digest(s.as_bytes())) }

#[derive(serde::Serialize)]
struct HeapObs {
    path:           String,
    bytes:          u64,
    entropy:        f64,
    string_count:   usize,
    url_count:      usize,
    func_count:     usize,
    top_urls:       Vec<String>,
    top_funcs:      Vec<String>,
    seal:           String,
}

fn shannon(data: &[u8]) -> f64 {
    if data.is_empty() { return 0.0; }
    let mut freq = [0u64; 256];
    for &b in data { freq[b as usize] += 1; }
    let n = data.len() as f64;
    freq.iter().filter(|&&c| c > 0)
        .map(|&c| { let p = c as f64 / n; -p * p.log2() })
        .sum::<f64>() / 8.0
}

fn observe_heap(path: &str) -> Option<HeapObs> {
    let raw = fs::read(path).ok()?;
    let bytes = raw.len() as u64;
    let entropy = (shannon(&raw) * 1000.0).round() / 1000.0;
    let text = String::from_utf8_lossy(&raw);

    // Extract strings section — V8 heap JSON has "strings" array
    let re_url   = Regex::new(r#"https?://[^\s"'\\]{4,120}"#).unwrap();
    let re_func  = Regex::new(r#""([a-zA-Z_$][a-zA-Z0-9_$]{8,60})""#).unwrap();

    // Deduplicate URLs
    let mut urls: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        re_url.find_iter(&text)
            .map(|m| m.as_str().to_string())
            .filter(|u| seen.insert(u.clone()))
            .collect()
    };
    urls.sort_by_key(|u| u.len());
    urls.dedup();

    // Function/identifier names (long identifiers = interesting)
    let mut funcs: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        re_func.captures_iter(&text)
            .filter_map(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .filter(|f| seen.insert(f.clone()))
            .take(500)
            .collect()
    };
    funcs.sort();
    funcs.dedup();

    let snapshot_seal = seal(&format!("{}{}", path, bytes));

    Some(HeapObs {
        path:         path.to_string(),
        bytes,
        entropy,
        string_count: text.matches('"').count() / 2,
        url_count:    urls.len(),
        func_count:   funcs.len(),
        top_urls:     urls.into_iter().take(20).collect(),
        top_funcs:    funcs.into_iter().take(20).collect(),
        seal:         snapshot_seal,
    })
}

#[derive(serde::Serialize)]
struct HeapDiff {
    snap_a:        String,
    snap_b:        String,
    bytes_a:       u64,
    bytes_b:       u64,
    delta_bytes:   i64,
    delta_pct:     f64,
    delta_urls:    i64,
    new_urls:      Vec<String>,
    entropy_drift: f64,
    classification: String,
    seal:          String,
}

fn diff_heaps(a: &HeapObs, b: &HeapObs) -> HeapDiff {
    let delta_bytes = b.bytes as i64 - a.bytes as i64;
    let delta_pct   = (delta_bytes as f64 / a.bytes as f64) * 100.0;
    let delta_urls  = b.url_count as i64 - a.url_count as i64;
    let entropy_drift = (b.entropy - a.entropy * 1000.0).round() / 1000.0;

    // New URLs in B not in A
    let a_urls: std::collections::HashSet<_> = a.top_urls.iter().collect();
    let new_urls: Vec<String> = b.top_urls.iter()
        .filter(|u| !a_urls.contains(u))
        .cloned().collect();

    let classification = if delta_bytes > 10_000_000 {
        "MAJOR_ALLOCATION"
    } else if delta_bytes > 1_000_000 {
        "SIGNIFICANT_GROWTH"
    } else if delta_bytes > 100_000 {
        "MINOR_GROWTH"
    } else if delta_bytes < -1_000_000 {
        "GC_COLLECTION"
    } else {
        "STABLE"
    }.to_string();

    let seal_in = format!("{}{}{}", a.seal, b.seal, delta_bytes);
    HeapDiff {
        snap_a: a.path.clone(),
        snap_b: b.path.clone(),
        bytes_a: a.bytes,
        bytes_b: b.bytes,
        delta_bytes,
        delta_pct: (delta_pct * 100.0).round() / 100.0,
        delta_urls,
        new_urls,
        entropy_drift,
        classification,
        seal: seal(&seal_in),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Collect snapshot paths
    let mut paths: Vec<String> = if args.iter().any(|a| a == "--dir") {
        let dir = args.windows(2).find(|w| w[0] == "--dir").map(|w| w[1].as_str()).unwrap_or(".");
        let mut v: Vec<String> = fs::read_dir(dir).unwrap_or_else(|_| panic!("bad dir"))
            .flatten()
            .filter(|e| e.path().extension().map(|x| x == "heapsnapshot").unwrap_or(false))
            .map(|e| e.path().to_string_lossy().to_string())
            .collect();
        v.sort();
        v
    } else {
        args[1..].iter().filter(|a| !a.starts_with("--") && a.ends_with(".heapsnapshot")).cloned().collect()
    };

    if paths.is_empty() {
        println!("USE: sagco-heap <a.heapsnapshot> [b.heapsnapshot] [c.heapsnapshot]");
        println!("     sagco-heap --dir <folder>");
        std::process::exit(1);
    }

    println!("=== SAGCO-HEAP v1 ===");
    println!("SNAPSHOTS={}", paths.len());
    println!("");

    // Observe each snapshot
    let mut observations: Vec<HeapObs> = Vec::new();
    for path in &paths {
        print!("OBSERVING {} ... ", std::path::Path::new(path).file_name().unwrap().to_string_lossy());
        match observe_heap(path) {
            Some(obs) => {
                println!("{}MB urls={} seal={:.12}...", obs.bytes / 1_000_000, obs.url_count, obs.seal);
                observations.push(obs);
            }
            None => println!("FAILED"),
        }
    }

    // Diff consecutive pairs
    let mut diffs: Vec<HeapDiff> = Vec::new();
    for i in 0..observations.len().saturating_sub(1) {
        let d = diff_heaps(&observations[i], &observations[i+1]);
        println!("DIFF [{} → {}]: {:+}MB  {}  new_urls={}",
            i, i+1,
            d.delta_bytes / 1_000_000,
            d.classification,
            d.new_urls.len(),
        );
        for u in d.new_urls.iter().take(5) {
            println!("  NEW_URL={}", u);
        }
        diffs.push(d);
    }

    // Master seal + report
    let chain = observations.iter().map(|o| o.seal.as_str()).collect::<Vec<_>>().join("|");
    let master_seal = seal(&chain);
    let timestamp = Utc::now().to_rfc3339();

    let report = json!({
        "opcode":      "HEAP_OBSERVE",
        "timestamp":   timestamp,
        "snapshots":   observations.len(),
        "diffs":       diffs.len(),
        "observations": observations,
        "diffs":       diffs,
        "master_seal": master_seal,
        "status":      "SAGCO_HEAP_PASS",
    });

    fs::create_dir_all("reports/heap").ok();
    let ts  = Utc::now().format("%Y%m%d_%H%M%S");
    let out = format!("reports/heap/heap_{}.json", ts);
    fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).ok();

    fs::create_dir_all("data").ok();
    let ledger_line = serde_json::to_string(&json!({
        "opcode":    "HEAP_OBSERVE",
        "timestamp": timestamp,
        "snapshots": paths.len(),
        "master_seal": master_seal,
        "report":    out,
        "status":    "SAGCO_HEAP_PASS",
    })).unwrap() + "\n";
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open("data/heap_ledger.jsonl") {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    println!("");
    println!("MASTER_SEAL={}", master_seal);
    println!("REPORT={}", out);
    println!("LEDGER=data/heap_ledger.jsonl");
    println!("STATUS=SAGCO_HEAP_PASS");
}
