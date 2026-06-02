/// sagco-gcp-agent — GCP REST API agent (curl -L equivalent in Rust)
/// Authenticates via `gcloud auth print-access-token`, hits GCP REST APIs,
/// seals all responses, ledgers as GCP_REALITY_ARTIFACTS.
///
/// USE: sagco-gcp-agent --project <project_id> [--api <opcode>] [--json]
///
/// Opcodes / --api values:
///   project-info   = resourcemanager.googleapis.com  (default)
///   clusters       = container.googleapis.com
///   services       = serviceusage.googleapis.com
///   billing        = cloudbilling.googleapis.com
///   all            = run all of the above
///
/// KNOWN PROJECTS:
///   sagco-oscomputconsciousness
///   jarvis-swarm-personal
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{env, fs, io::Write, process::Command};

// ── Token ────────────────────────────────────────────────────────────────────

fn gcloud_token() -> Result<String, String> {
    let out = Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .map_err(|e| format!("gcloud not found: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// ── curl -L equivalent: minreq GET with Bearer token ─────────────────────────

fn gcp_get(url: &str, token: &str) -> (u16, String) {
    match minreq::get(url)
        .with_header("Authorization", format!("Bearer {}", token))
        .with_header("Accept", "application/json")
        .with_header("User-Agent", "sagco-gcp-agent/1.0")
        .with_timeout(15)
        .send()
    {
        Ok(resp) => {
            let code   = resp.status_code as u16;
            let body   = resp.as_str().unwrap_or("").to_string();
            (code, body)
        }
        Err(e) => (0, format!("REQUEST_ERROR: {:?}", e)),
    }
}

// ── Individual API calls ──────────────────────────────────────────────────────

fn call_project_info(project: &str, token: &str) -> (String, u16, String) {
    let url = format!(
        "https://cloudresourcemanager.googleapis.com/v3/projects/{}",
        project
    );
    let (code, body) = gcp_get(&url, token);
    ("project-info".to_string(), code, body)
}

fn call_clusters(project: &str, token: &str) -> (String, u16, String) {
    let url = format!(
        "https://container.googleapis.com/v1/projects/{}/locations/-/clusters",
        project
    );
    let (code, body) = gcp_get(&url, token);
    ("clusters".to_string(), code, body)
}

fn call_services(project: &str, token: &str) -> (String, u16, String) {
    // List enabled services (first page)
    let url = format!(
        "https://serviceusage.googleapis.com/v1/projects/{}/services?filter=state:ENABLED&pageSize=50",
        project
    );
    let (code, body) = gcp_get(&url, token);
    ("services".to_string(), code, body)
}

fn call_billing(project: &str, token: &str) -> (String, u16, String) {
    let url = format!(
        "https://cloudbilling.googleapis.com/v1/projects/{}/billingInfo",
        project
    );
    let (code, body) = gcp_get(&url, token);
    ("billing".to_string(), code, body)
}

// ── Status classifier ─────────────────────────────────────────────────────────

fn classify(code: u16, body: &str) -> (&'static str, &'static str) {
    match code {
        200 => ("SAGCO_GCP_CALL_PASS", "NONE"),
        401 => ("SAGCO_GCP_AUTH_FAIL",  "GCP_AUTH_ANTIBODY"),
        403 => ("SAGCO_GCP_FORBIDDEN",  "GCP_PERMISSION_ANTIBODY"),
        404 => ("SAGCO_GCP_NOT_FOUND",  "GCP_PROJECT_MISSING_ANTIBODY"),
        0   => {
            if body.contains("gcloud not found") || body.contains("REQUEST_ERROR") {
                ("SAGCO_GCP_NETWORK_FAIL", "GCP_NETWORK_ANTIBODY")
            } else {
                ("SAGCO_GCP_UNKNOWN_FAIL", "GCP_UNKNOWN_ANTIBODY")
            }
        }
        _   => ("SAGCO_GCP_UNEXPECTED_CODE", "GCP_CODE_ANTIBODY"),
    }
}

fn seal(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();

    let project: String = args.windows(2)
        .find(|w| w[0] == "--project")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "sagco-oscomputconsciousness".to_string());

    let api: String = args.windows(2)
        .find(|w| w[0] == "--api")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "all".to_string());

    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-GCP-AGENT v1 ===");
    println!("PROJECT={}", project);
    println!("API={}", api);

    // ── Get Bearer token ──────────────────────────────────────────────────────
    let token = match gcloud_token() {
        Ok(t) => {
            println!("TOKEN=OK len={}", t.len());
            t
        }
        Err(e) => {
            println!("ANTIBODY=GCLOUD_TOKEN_ANTIBODY");
            println!("ERROR={}", e);
            println!("FIX=run: gcloud auth login");
            println!("STATUS=SAGCO_GCP_AUTH_FAIL");
            std::process::exit(2);
        }
    };

    // ── Choose which calls to run ─────────────────────────────────────────────
    let calls_to_run: Vec<&str> = match api.as_str() {
        "project-info" => vec!["project-info"],
        "clusters"     => vec!["clusters"],
        "services"     => vec!["services"],
        "billing"      => vec!["billing"],
        _              => vec!["project-info", "clusters", "services", "billing"],
    };

    // ── Execute calls ─────────────────────────────────────────────────────────
    fs::create_dir_all("reports/gcp").ok();
    fs::create_dir_all("data").ok();

    let timestamp = Utc::now().to_rfc3339();
    let mut results: Vec<serde_json::Value> = Vec::new();
    let mut all_pass = true;

    for call_name in &calls_to_run {
        let (name, code, body) = match *call_name {
            "project-info" => call_project_info(&project, &token),
            "clusters"     => call_clusters(&project, &token),
            "services"     => call_services(&project, &token),
            "billing"      => call_billing(&project, &token),
            _              => continue,
        };

        let (status, antibody) = classify(code, &body);
        let response_seal      = seal(&body);

        // Write artifact
        let art_path = format!("reports/gcp/{}_{}.json", name, Utc::now().format("%Y%m%d_%H%M%S"));
        fs::write(&art_path, &body).ok();

        println!("CALL={} HTTP={} SEAL={:.16}... STATUS={}",
            name, code, response_seal, status);

        if code != 200 { all_pass = false; }

        results.push(json!({
            "call":      name,
            "http_code": code,
            "artifact":  art_path,
            "seal":      response_seal,
            "antibody":  antibody,
            "status":    status,
        }));
    }

    // ── Master seal over all responses ───────────────────────────────────────
    let chain_input: String = results.iter()
        .map(|r| r["seal"].as_str().unwrap_or(""))
        .collect::<Vec<_>>().join("|");
    let master_seal = seal(&chain_input);

    let overall_status = if all_pass {
        "SAGCO_GCP_AGENT_PASS"
    } else {
        "SAGCO_GCP_AGENT_PARTIAL"
    };

    // ── Full sealed report ────────────────────────────────────────────────────
    let report = json!({
        "opcode":    "GCP_OBSERVE",
        "timestamp": timestamp,
        "project":   project,
        "api":       api,
        "calls_run": results.len(),
        "all_pass":  all_pass,
        "results":   results,
        "seal":      master_seal,
        "status":    overall_status,
    });

    let report_text = serde_json::to_string_pretty(&report).unwrap();
    let ts   = Utc::now().format("%Y%m%d_%H%M%S");
    let rout = format!("reports/gcp/gcp_agent_{}_{}.json", project, ts);
    fs::write(&rout, &report_text).ok();

    // ── Ledger ────────────────────────────────────────────────────────────────
    let ledger_line = serde_json::to_string(&json!({
        "opcode":    "GCP_OBSERVE",
        "timestamp": Utc::now().to_rfc3339(),
        "project":   project,
        "calls":     calls_to_run.len(),
        "all_pass":  all_pass,
        "report":    rout,
        "seal":      master_seal,
        "status":    overall_status,
    })).unwrap() + "\n";

    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/gcp_ledger.jsonl")
    {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    if json_mode { println!("{}", report_text); }

    println!("---");
    println!("MASTER_SEAL={}", master_seal);
    println!("REPORT={}", rout);
    println!("LEDGER=data/gcp_ledger.jsonl");
    println!("STATUS={}", overall_status);

    if !all_pass { std::process::exit(3); }
}
