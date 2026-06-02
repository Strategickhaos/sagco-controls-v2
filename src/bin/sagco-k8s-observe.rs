/// sagco-k8s-observe — K8S opcode: observe live cluster state, seal + ledger
/// USE: sagco-k8s-observe [--context <ctx>]
/// Runs: kubectl cluster-info, get nodes, get pods, get svc — seals all outputs.
use chrono::Utc;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, io::Write, process::Command};

fn run_kubectl(args: &[&str]) -> (bool, String) {
    match Command::new("kubectl").args(args).output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let combined = if stderr.is_empty() { stdout } else { format!("{}\nSTDERR: {}", stdout, stderr) };
            (out.status.success(), combined)
        }
        Err(e) => (false, format!("KUBECTL_NOT_FOUND: {}", e)),
    }
}

fn seal(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn classify_error(text: &str) -> (&'static str, &'static str) {
    if text.contains("KUBECTL_NOT_FOUND") {
        return ("SAGCO_K8S_KUBECTL_MISSING",    "KUBECTL_MISSING_ANTIBODY");
    }
    if text.contains("gke-gcloud-auth-plugin") {
        return ("SAGCO_K8S_AUTH_PLUGIN_MISSING", "GKE_AUTH_PLUGIN_MISSING_ANTIBODY");
    }
    if text.contains("Unable to connect") || text.contains("connection refused") || text.contains("no such host") {
        return ("SAGCO_K8S_CLUSTER_UNREACHABLE", "K8S_CLUSTER_UNREACHABLE_ANTIBODY");
    }
    if text.contains("Unauthorized") || text.contains("forbidden") {
        return ("SAGCO_K8S_AUTH_FAIL",           "K8S_AUTH_ANTIBODY");
    }
    ("SAGCO_K8S_OBSERVE_VARIANCE", "K8S_CONTEXT_VARIANCE_ANTIBODY")
}

fn count_kind(json_text: &str, kind: &str) -> usize {
    // Count occurrences of "kind":"Node" and "kind": "Node" (spacing varies)
    let tight = format!("\"kind\":\"{}\"", kind);
    let loose = format!("\"kind\": \"{}\"", kind);
    json_text.matches(tight.as_str()).count() + json_text.matches(loose.as_str()).count()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Optional --context override
    let ctx_override: Option<&str> = args.windows(2)
        .find(|w| w[0] == "--context")
        .map(|w| w[1].as_str());

    fs::create_dir_all("reports/k8s").ok();
    fs::create_dir_all("data").ok();

    let timestamp = Utc::now().to_rfc3339();

    println!("=== SAGCO-K8S-OBSERVE v1 ===");

    // ── 1. Active context ─────────────────────────────────────────────────
    let (ctx_ok, ctx_text) = if let Some(c) = ctx_override {
        run_kubectl(&["config", "use-context", c])
    } else {
        run_kubectl(&["config", "current-context"])
    };
    let context_name = ctx_text.trim().to_string();
    println!("CONTEXT={}", context_name);

    // ── 2. Cluster info ───────────────────────────────────────────────────
    let (_info_ok, cluster_info) = run_kubectl(&["cluster-info"]);

    // ── 3. Nodes ──────────────────────────────────────────────────────────
    let (nodes_ok, nodes_json) = run_kubectl(&["get", "nodes", "-o", "json"]);

    // ── 4. Pods (all namespaces) ──────────────────────────────────────────
    let (pods_ok, pods_json) = run_kubectl(&["get", "pods", "-A", "-o", "json"]);

    // ── 5. Services ───────────────────────────────────────────────────────
    let (svc_ok, svc_json) = run_kubectl(&["get", "svc", "-A", "-o", "json"]);

    // ── Write artifact files ──────────────────────────────────────────────
    fs::write("reports/k8s/context.txt",   &context_name).ok();
    fs::write("reports/k8s/cluster_info.txt", &cluster_info).ok();
    fs::write("reports/k8s/nodes.json",    &nodes_json).ok();
    fs::write("reports/k8s/pods.json",     &pods_json).ok();
    fs::write("reports/k8s/services.json", &svc_json).ok();

    // ── Counts (heuristic from raw JSON text) ─────────────────────────────
    let node_count    = count_kind(&nodes_json, "Node");
    let pod_count     = count_kind(&pods_json,  "Pod");
    let svc_count     = count_kind(&svc_json,   "Service");

    // ── Classify status from combined output ──────────────────────────────
    let all_text = format!("{}\n{}\n{}\n{}\n{}", context_name, cluster_info, nodes_json, pods_json, svc_json);
    let master_seal = seal(&all_text);

    let (status, antibody) = if ctx_ok && nodes_ok && pods_ok && svc_ok {
        ("SAGCO_K8S_OBSERVE_PASS", "NONE")
    } else {
        classify_error(&all_text)
    };

    println!("NODE_COUNT={}", node_count);
    println!("POD_COUNT={}", pod_count);
    println!("SERVICE_COUNT={}", svc_count);
    println!("ANTIBODY={}", antibody);

    // ── Sealed report ─────────────────────────────────────────────────────
    let report = json!({
        "opcode":    "K8S_OBSERVE",
        "timestamp": timestamp,
        "context":   context_name,
        "checks": {
            "context_ok":  ctx_ok,
            "nodes_ok":    nodes_ok,
            "pods_ok":     pods_ok,
            "services_ok": svc_ok,
        },
        "counts": {
            "nodes":    node_count,
            "pods":     pod_count,
            "services": svc_count,
        },
        "artifacts": {
            "context":      "reports/k8s/context.txt",
            "cluster_info": "reports/k8s/cluster_info.txt",
            "nodes":        "reports/k8s/nodes.json",
            "pods":         "reports/k8s/pods.json",
            "services":     "reports/k8s/services.json",
        },
        "seal":     master_seal,
        "antibody": antibody,
        "status":   status,
    });

    let report_text = serde_json::to_string_pretty(&report).unwrap();
    fs::write("reports/k8s/k8s_observe_report.json", &report_text).ok();

    // ── Ledger append ─────────────────────────────────────────────────────
    let ledger_entry = serde_json::to_string(&json!({
        "opcode":    "K8S_OBSERVE",
        "timestamp": Utc::now().to_rfc3339(),
        "context":   context_name,
        "nodes":     node_count,
        "pods":      pod_count,
        "services":  svc_count,
        "report":    "reports/k8s/k8s_observe_report.json",
        "seal":      master_seal,
        "antibody":  antibody,
        "status":    status,
    })).unwrap() + "\n";

    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/k8s_ledger.jsonl")
    {
        let _ = f.write_all(ledger_entry.as_bytes());
    }

    println!("SEAL={}", master_seal);
    println!("REPORT=reports/k8s/k8s_observe_report.json");
    println!("LEDGER=data/k8s_ledger.jsonl");
    println!("STATUS={}", status);

    if status != "SAGCO_K8S_OBSERVE_PASS" {
        std::process::exit(2);
    }
}
