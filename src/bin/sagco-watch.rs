/// sagco-watch — File system watcher: auto-OBSERVE + TOKENIZE on every change
/// Polls a path every N seconds. When a file changes, runs the full
/// OBSERVE → TOKENIZE → LEDGER opcode chain automatically.
/// USE: sagco-watch <path> [--interval 5] [--once]
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{collections::HashMap, env, fs, io::Write, process::Command, time::{Duration, Instant, SystemTime}};

fn seal(s: &str) -> String { format!("{:x}", Sha256::digest(s.as_bytes())) }

fn run(bin: &str, args: &[&str]) -> String {
    let exe = format!("target/debug/{}.exe", bin);
    let path = if std::path::Path::new(&exe).exists() { exe } else { bin.to_string() };
    match Command::new(&path).args(args).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => format!("ERR={}", e),
    }
}

fn mtime(path: &str) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

fn file_seal(path: &str) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    format!("{:x}", Sha256::digest(&bytes))
}

fn scan_files(root: &str) -> HashMap<String, (SystemTime, String)> {
    let mut map = HashMap::new();
    for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
        if entry.metadata().map(|m| m.is_file()).unwrap_or(false) {
            let path = entry.path().to_string_lossy().to_string();
            if let Some(mt) = mtime(&path) {
                let s = seal(&path);
                map.insert(path, (mt, s));
            }
        }
    }
    map
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let watch_path = args.get(1).cloned().unwrap_or_else(|| ".".to_string());
    let interval: u64 = args.windows(2)
        .find(|w| w[0] == "--interval")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(5);
    let once = args.iter().any(|a| a == "--once");

    println!("=== SAGCO-WATCH v1 ===");
    println!("WATCHING={}", watch_path);
    println!("INTERVAL={}s", interval);
    println!("STARTED={}", Utc::now().to_rfc3339());

    let mut known = scan_files(&watch_path);
    let mut tick  = 0u64;
    let mut events_total = 0u64;

    println!("INITIAL_FILES={}", known.len());
    println!("STATUS=SAGCO_WATCH_ONLINE");
    println!("");

    loop {
        std::thread::sleep(Duration::from_secs(interval));
        tick += 1;

        let current = scan_files(&watch_path);
        let ts = Utc::now().to_rfc3339();

        // Detect changes
        let mut changed: Vec<String> = Vec::new();
        let mut added:   Vec<String> = Vec::new();
        let mut removed: Vec<String> = Vec::new();

        for (path, (mt, _)) in &current {
            match known.get(path) {
                None => added.push(path.clone()),
                Some((old_mt, _)) => {
                    if mt != old_mt { changed.push(path.clone()); }
                }
            }
        }
        for path in known.keys() {
            if !current.contains_key(path) { removed.push(path.clone()); }
        }

        let total_events = changed.len() + added.len() + removed.len();
        if total_events == 0 {
            println!("[{}] TICK={} STABLE files={}", ts, tick, current.len());
        } else {
            events_total += total_events as u64;
            println!("[{}] TICK={} CHANGED={} ADDED={} REMOVED={}",
                ts, tick, changed.len(), added.len(), removed.len());

            // Run OBSERVE + TOKENIZE on changed/added files
            for path in changed.iter().chain(added.iter()).take(5) {
                println!("  EVENT=CHANGE PATH={}", path);
                let obs = run("sagco-observe", &[path]);
                let obs_status = obs.lines().find(|l| l.starts_with("STATUS=")).unwrap_or("STATUS=UNKNOWN");
                let obs_seal   = obs.lines().find(|l| l.starts_with("SEAL=")).unwrap_or("SEAL=NONE");
                println!("    OBSERVE: {} {}", obs_status, obs_seal);

                // Only tokenize text files (skip binaries)
                let is_text = path.ends_with(".rs") || path.ends_with(".toml") ||
                              path.ends_with(".json") || path.ends_with(".md") ||
                              path.ends_with(".yaml") || path.ends_with(".sh") ||
                              path.ends_with(".jsonl");
                if is_text {
                    let tok = run("sagco-tokenize", &[path]);
                    let tok_status = tok.lines().find(|l| l.starts_with("STATUS=")).unwrap_or("STATUS=UNKNOWN");
                    println!("    TOKENIZE: {}", tok_status);
                }

                // Append to watch ledger
                fs::create_dir_all("data").ok();
                let event_seal = file_seal(path);
                let entry = serde_json::to_string(&json!({
                    "opcode":    "WATCH_EVENT",
                    "timestamp": ts,
                    "path":      path,
                    "tick":      tick,
                    "seal":      event_seal,
                    "status":    "SAGCO_WATCH_EVENT",
                })).unwrap() + "\n";
                if let Ok(mut f) = fs::OpenOptions::new()
                    .create(true).append(true).open("data/watch_ledger.jsonl")
                {
                    let _ = f.write_all(entry.as_bytes());
                }
            }

            if removed.len() > 0 {
                for path in &removed {
                    println!("  EVENT=REMOVED PATH={}", path);
                }
            }
        }

        known = current;
        if once { break; }
    }

    println!("TICKS={} EVENTS_TOTAL={}", tick, events_total);
    println!("STATUS=SAGCO_WATCH_STOPPED");
}
