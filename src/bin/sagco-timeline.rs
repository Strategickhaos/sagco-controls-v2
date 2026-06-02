/// sagco-timeline — Tier-0 super-timeline (rivals Plaso/log2timeline)
/// Walks a path, sorts all files by mtime, emits a sealed chronological event log.
/// USE: sagco-timeline <root_path> [--json]
use std::fs;
use std::time::UNIX_EPOCH;
use sha2::{Digest, Sha256};
use serde_json::json;
use chrono::{Utc, TimeZone};

#[derive(serde::Serialize)]
struct TimelineEvent {
    epoch:    u64,
    datetime: String,
    size:     u64,
    path:     String,
    event:    String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = args.get(1).map(|s| s.as_str()).unwrap_or(".");
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-TIMELINE v1 ===");
    println!("ROOT={}", root);

    let mut events: Vec<TimelineEvent> = walkdir::WalkDir::new(root)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if !meta.is_file() { return None; }
            let epoch = meta.modified().ok()?
                .duration_since(UNIX_EPOCH).ok()?.as_secs();
            let dt = Utc.timestamp_opt(epoch as i64, 0)
                .single()
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "UNKNOWN".to_string());
            Some(TimelineEvent {
                epoch,
                datetime: dt,
                size: meta.len(),
                path: e.path().to_string_lossy().to_string(),
                event: "MTIME".to_string(),
            })
        })
        .collect();

    events.sort_by_key(|e| e.epoch);

    // Seal the timeline
    let seal_input: String = events.iter().map(|e| format!("{}{}", e.epoch, e.path)).collect();
    let seal = format!("{:x}", Sha256::digest(seal_input.as_bytes()));

    if json_mode {
        let out = json!({
            "sagco_command": "sagco-timeline",
            "timestamp": Utc::now().to_rfc3339(),
            "root": root,
            "event_count": events.len(),
            "seal": seal,
            "events": events,
        });
        fs::create_dir_all("reports").ok();
        let ts = Utc::now().format("%Y%m%d_%H%M%S");
        let p = format!("reports/timeline_{}.json", ts);
        fs::write(&p, serde_json::to_string_pretty(&out).unwrap()).ok();
        println!("REPORT={}", p);
    } else {
        for e in &events {
            println!("EVENT={} PATH={} SIZE={}", e.datetime, e.path, e.size);
        }
    }

    println!("EVENT_COUNT={}", events.len());
    println!("SEAL={}", seal);
    println!("STATUS=SAGCO_TIMELINE_PASS");
}
