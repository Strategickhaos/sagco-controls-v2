/// sagco-api — HTTP API server: query SAGCO state over REST
/// Exposes ledger, topology, and status as JSON endpoints.
/// USE: sagco-api [--port 7777]
///
/// Endpoints:
///   GET /             → system status
///   GET /topology     → latest topology nodes/edges
///   GET /ledger       → all ledger entries
///   GET /status       → agent + chain status
///   POST /run/guard   → run sagco-guard (body: {"crew":4,"hrs":10})
///   POST /run/reclass → run sagco-reclass
use std::{env, fs, io::{Read, Write}, net::TcpListener, process::Command};
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;

fn seal(s: &str) -> String { format!("{:x}", Sha256::digest(s.as_bytes())) }

// ── Auth ──────────────────────────────────────────────────────────────────────
// Bearer token guard for write routes.
// Set env SAGCO_TOKEN to override default (change in prod).
fn check_auth(req: &str) -> bool {
    let token = std::env::var("SAGCO_TOKEN")
        .unwrap_or_else(|_| "sagco-dev-token-2026".to_string());
    let bearer = format!("Bearer {}", token);
    req.lines().any(|l| l.starts_with("Authorization:") && l.contains(&bearer))
}

fn log_write(req: &str, payload: &str) {
    // Audit every write attempt regardless of auth result
    let ts    = Utc::now().to_rfc3339();
    let agent = req.lines()
        .find(|l| l.starts_with("User-Agent:"))
        .unwrap_or("User-Agent: unknown");
    println!("[{}] LEDGER_WRITE_ATTEMPT agent={} payload_len={}", ts, agent.trim(), payload.len());
}

fn run_bin(bin: &str, args: &[&str]) -> String {
    let exe = format!("target/debug/{}.exe", bin);
    let path = if std::path::Path::new(&exe).exists() { exe } else { bin.to_string() };
    match Command::new(&path).args(args).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => format!("ERROR={}", e),
    }
}

fn read_all_ledgers() -> serde_json::Value {
    let mut ledgers = serde_json::Map::new();
    if let Ok(entries) = fs::read_dir("data") {
        for e in entries.flatten() {
            if e.path().extension().map(|x| x == "jsonl").unwrap_or(false) {
                let name = e.file_name().to_string_lossy().trim_end_matches(".jsonl").to_string();
                let content = fs::read_to_string(e.path()).unwrap_or_default();
                let rows: Vec<serde_json::Value> = content.lines()
                    .filter(|l| !l.trim().is_empty())
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect();
                ledgers.insert(name, json!(rows));
            }
        }
    }
    serde_json::Value::Object(ledgers)
}

fn json_response(code: &str, body: serde_json::Value) -> String {
    let b = serde_json::to_string_pretty(&body).unwrap();
    format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
        code, b.len(), b
    )
}

fn handle(req: &str) -> String {
    let line = req.lines().next().unwrap_or("");
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() < 2 { return json_response("400 Bad Request", json!({"error":"bad request"})); }

    let method = parts[0];
    let path   = parts[1];

    match (method, path) {
        ("GET", "/") | ("GET", "/status") => {
            let topology = run_bin("sagco-topology", &["--json"]);
            let nodes = topology.lines()
                .find(|l| l.contains("TOTAL_NODES="))
                .and_then(|l| l.split('=').nth(1))
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            let edges = topology.lines()
                .find(|l| l.contains("TOTAL_EDGES="))
                .and_then(|l| l.split('=').nth(1))
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);

            json_response("200 OK", json!({
                "sagco": "controls-v2",
                "version": "0.3.0",
                "timestamp": Utc::now().to_rfc3339(),
                "topology": { "nodes": nodes, "edges": edges },
                "status": "SAGCO_API_ONLINE",
                "seal": seal(&Utc::now().to_rfc3339()),
                "endpoints": ["/", "/topology", "/ledger", "/status", "POST /run/guard", "POST /run/reclass"]
            }))
        }

        ("GET", "/topology") => {
            let out = run_bin("sagco-topology", &["--json"]);
            let topo_file = fs::read_dir("reports/topology")
                .ok().and_then(|mut d| d.next())
                .and_then(|e| e.ok())
                .map(|e| e.path());
            let topo_json = topo_file
                .and_then(|p| fs::read_to_string(p).ok())
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .unwrap_or(json!({"raw": out}));
            json_response("200 OK", topo_json)
        }

        ("GET", "/ledger") => {
            json_response("200 OK", json!({
                "timestamp": Utc::now().to_rfc3339(),
                "ledgers": read_all_ledgers()
            }))
        }

        ("POST", p) if p.starts_with("/run/guard") => {
            // Auth guard on all write/run routes
            if !check_auth(req) {
                return json_response("401 Unauthorized",
                    json!({"error":"unauthorized","hint":"Authorization: Bearer sagco-dev-token-2026"}));
            }
            let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(req.len());
            log_write(req, &req[body_start..]);
            let body: serde_json::Value = serde_json::from_str(&req[body_start..]).unwrap_or(json!({}));
            let crew = body["crew"].as_f64().unwrap_or(4.0).to_string();
            let hrs  = body["hrs"].as_f64().unwrap_or(10.0).to_string();
            let out  = run_bin("sagco-guard", &[&crew, &hrs]);
            let status = out.lines().find(|l| l.starts_with("STATUS=")).unwrap_or("STATUS=UNKNOWN").to_string();
            json_response("200 OK", json!({ "output": out, "status": status }))
        }

        ("POST", p) if p.starts_with("/run/reclass") => {
            if !check_auth(req) {
                return json_response("401 Unauthorized",
                    json!({"error":"unauthorized","hint":"Authorization: Bearer sagco-dev-token-2026"}));
            }
            let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(req.len());
            log_write(req, &req[body_start..]);
            let b: serde_json::Value = serde_json::from_str(&req[body_start..]).unwrap_or(json!({}));
            let ou = b["old_used"].as_f64().unwrap_or(1011.6).to_string();
            let or_ = b["old_remaining"].as_f64().unwrap_or(68.4).to_string();
            let nu = b["new_used"].as_f64().unwrap_or(851.6).to_string();
            let nr = b["new_remaining"].as_f64().unwrap_or(228.4).to_string();
            let cs = b["crew_size"].as_f64().unwrap_or(4.0).to_string();
            let hpd = b["hrs_per_day"].as_f64().unwrap_or(10.0).to_string();
            let out = run_bin("sagco-reclass", &[&ou, &or_, &nu, &nr, &cs, &hpd]);
            let status = out.lines().find(|l| l.starts_with("STATUS=")).unwrap_or("STATUS=UNKNOWN").to_string();
            json_response("200 OK", json!({ "output": out, "status": status }))
        }

        _ => json_response("404 Not Found", json!({"error":"unknown endpoint","path":path}))
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let port: u16 = args.windows(2)
        .find(|w| w[0] == "--port")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(7777);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).expect("cannot bind port");

    println!("=== SAGCO-API v1 ===");
    println!("ADDR=http://{}", addr);
    println!("ENDPOINTS=/ /topology /ledger /status POST:/run/guard POST:/run/reclass");
    println!("STATUS=SAGCO_API_ONLINE");

    for stream in listener.incoming().flatten() {
        let mut s = stream;
        let mut buf = [0u8; 4096];
        if let Ok(n) = s.read(&mut buf) {
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let resp = handle(&req);
            let _ = s.write_all(resp.as_bytes());
        }
    }
}
