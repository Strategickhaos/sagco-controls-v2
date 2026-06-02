/// sagco-agent — Sovereign agent runner: executes Red / Blue / Purple team loops
/// Orchestrates all SAGCO opcodes into structured team runs.
///
/// USE:
///   sagco-agent run red    [--target <path>]
///   sagco-agent run blue   [--target <path>]
///   sagco-agent run purple [--ledger <path>]
///   sagco-agent run gcp    [--project <id>]
///   sagco-agent run all    [--target <path>]
///   sagco-agent status
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{env, fs, io::Write, process::Command, time::Instant};

// ── Task descriptor ───────────────────────────────────────────────────────────

struct Task {
    name:   &'static str,
    bin:    &'static str,
    args:   Vec<String>,
    team:   &'static str,
    opcode: &'static str,
}

// ── Run one bin, capture output ───────────────────────────────────────────────

#[derive(serde::Serialize)]
struct TaskResult {
    name:       String,
    bin:        String,
    team:       String,
    opcode:     String,
    exit_code:  i32,
    status:     String,
    antibody:   String,
    duration_ms: u128,
    stdout:     String,
}

fn run_task(task: &Task) -> TaskResult {
    let t0 = Instant::now();
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--quiet", "--bin"]).arg(task.bin).arg("--");
    for a in &task.args { cmd.arg(a); }

    let (exit_code, stdout) = match cmd.output() {
        Ok(out) => {
            let code   = out.status.code().unwrap_or(-1);
            let text   = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let combined = if stderr.trim().is_empty() { text } else { format!("{}\nSTDERR: {}", text, stderr.lines().next().unwrap_or("")) };
            (code, combined)
        }
        Err(e) => (-1, format!("EXEC_ERROR={}", e)),
    };

    let status   = stdout.lines()
        .find(|l| l.starts_with("STATUS="))
        .map(|l| l.trim_start_matches("STATUS=").to_string())
        .unwrap_or_else(|| if exit_code == 0 { "PASS".to_string() } else { "FAIL".to_string() });

    let antibody = stdout.lines()
        .find(|l| l.starts_with("ANTIBODY="))
        .map(|l| l.trim_start_matches("ANTIBODY=").to_string())
        .unwrap_or_else(|| "NONE".to_string());

    TaskResult {
        name:        task.name.to_string(),
        bin:         task.bin.to_string(),
        team:        task.team.to_string(),
        opcode:      task.opcode.to_string(),
        exit_code,
        status,
        antibody,
        duration_ms: t0.elapsed().as_millis(),
        stdout,
    }
}

// ── Team definitions ──────────────────────────────────────────────────────────

fn red_team(target: &str, rules_file: &str) -> Vec<Task> {
    vec![
        Task {
            name: "topology-fuzz",
            bin:  "sagco-topofuzz",
            args: vec![],
            team: "RED",
            opcode: "FUZZ",
        },
        Task {
            name: "binscan",
            bin:  "sagco-binscan",
            args: vec![target.to_string(), "--json".to_string()],
            team: "RED",
            opcode: "BINSCAN",
        },
        Task {
            name: "extract",
            bin:  "sagco-extract",
            args: vec![target.to_string(), "--json".to_string()],
            team: "RED",
            opcode: "EXTRACT",
        },
        Task {
            name: "hunt",
            bin:  "sagco-hunt",
            args: vec![rules_file.to_string(), target.to_string(), "--json".to_string()],
            team: "RED",
            opcode: "HUNT",
        },
    ]
}

fn blue_team(target: &str) -> Vec<Task> {
    vec![
        Task {
            name: "guard-check",
            bin:  "sagco-guard",
            args: vec!["4".to_string(), "10".to_string()],
            team: "BLUE",
            opcode: "GUARD",
        },
        Task {
            name: "observe",
            bin:  "sagco-observe",
            args: vec![target.to_string()],
            team: "BLUE",
            opcode: "OBSERVE",
        },
        Task {
            name: "fswalk",
            bin:  "sagco-fswalk",
            args: vec![".".to_string(), "--json".to_string()],
            team: "BLUE",
            opcode: "FSWALK",
        },
        Task {
            name: "chainverify",
            bin:  "sagco-chainverify",
            args: vec![],
            team: "BLUE",
            opcode: "CHAINVERIFY",
        },
    ]
}

fn purple_team(ledger: &str) -> Vec<Task> {
    vec![
        Task {
            name: "tokenize",
            bin:  "sagco-tokenize",
            args: vec![ledger.to_string(), "--json".to_string()],
            team: "PURPLE",
            opcode: "TOKENIZE",
        },
        Task {
            name: "forecast",
            bin:  "sagco-forecast",
            args: vec!["--ledger".to_string(), ledger.to_string(), "--json".to_string()],
            team: "PURPLE",
            opcode: "FORECAST",
        },
        Task {
            name: "timeline",
            bin:  "sagco-timeline",
            args: vec![".".to_string(), "--json".to_string()],
            team: "PURPLE",
            opcode: "TIMELINE",
        },
    ]
}

fn gcp_team(project: &str) -> Vec<Task> {
    vec![
        Task {
            name: "gcp-oscomputconsciousness",
            bin:  "sagco-gcp-agent",
            args: vec![
                "--project".to_string(), "sagco-oscomputconsciousness".to_string(),
                "--api".to_string(), "all".to_string(),
                "--json".to_string(),
            ],
            team: "GCP",
            opcode: "GCP_OBSERVE",
        },
        Task {
            name: "gcp-jarvis",
            bin:  "sagco-gcp-agent",
            args: vec![
                "--project".to_string(), project.to_string(),
                "--api".to_string(), "clusters".to_string(),
                "--json".to_string(),
            ],
            team: "GCP",
            opcode: "GCP_OBSERVE",
        },
        Task {
            name: "k8s-observe",
            bin:  "sagco-k8s-observe",
            args: vec![],
            team: "GCP",
            opcode: "K8S_OBSERVE",
        },
    ]
}

// ── Print and seal team results ───────────────────────────────────────────────

fn run_team(team_name: &str, tasks: Vec<Task>) -> (Vec<TaskResult>, bool) {
    println!("\n{}", "=".repeat(50));
    println!("=== TEAM: {} ===", team_name);
    println!("{}", "=".repeat(50));

    let mut results   = Vec::new();
    let mut all_pass  = true;

    for task in tasks {
        println!("\n--- TASK: {} (opcode={}) ---", task.name, task.opcode);
        let r = run_task(&task);

        // Print key lines from stdout
        for line in r.stdout.lines().filter(|l| {
            l.starts_with("STATUS=") || l.starts_with("ANTIBODY=") ||
            l.starts_with("SEAL=")   || l.starts_with("REPORT=")   ||
            l.starts_with("ERROR=")  || l.starts_with("ALERT=")
        }) {
            println!("  {}", line);
        }
        println!("  EXIT={} DURATION={}ms", r.exit_code, r.duration_ms);

        if r.exit_code != 0 { all_pass = false; }
        results.push(r);
    }

    (results, all_pass)
}

fn seal(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn ensure_default_rules() -> String {
    let path = "data/sagco_default.rules";
    fs::create_dir_all("data").ok();
    if !std::path::Path::new(path).exists() {
        let rules = "# SAGCO default hunt rules\n\
SAGCO_STATUS: SAGCO_[A-Z_]+\n\
SCOPE_CREEP: SCOPE_CREEP\n\
HIGH_ENTROPY: [0-9a-fA-F]{64}\n\
URL_FOUND: https?://[^\\s]+\n\
ANTIBODY: ANTIBODY\n";
        fs::write(path, rules).ok();
    }
    path.to_string()
}

// ── Status command ────────────────────────────────────────────────────────────

fn cmd_status() {
    println!("=== SAGCO-AGENT STATUS ===");

    let teams = [
        ("🔴 RED",    vec!["sagco-topofuzz", "sagco-binscan", "sagco-extract", "sagco-hunt", "sagco-creep-watch"]),
        ("🔵 BLUE",   vec!["sagco-guard", "sagco-observe", "sagco-fswalk", "sagco-chainverify", "sagco-baseline", "sagco-k8s-observe"]),
        ("🟣 PURPLE", vec!["sagco-tokenize", "sagco-forecast", "sagco-verify", "sagco-timeline", "sagco-topoopt"]),
        ("🌐 GCP",    vec!["sagco-gcp-agent"]),
        ("🔧 CORE",   vec!["sagco-reclass", "sagco-stepper", "sagco-crawler"]),
    ];

    for (team, bins) in &teams {
        println!("\n{}", team);
        for bin in bins { println!("  ✓ {}", bin); }
    }

    // Ledger counts
    println!("\n=== LEDGER STATE ===");
    if let Ok(entries) = fs::read_dir("data") {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().map(|x| x == "jsonl").unwrap_or(false) {
                let count = fs::read_to_string(&path).unwrap_or_default()
                    .lines().filter(|l| !l.trim().is_empty()).count();
                println!("  {} entries={}", path.file_name().unwrap().to_string_lossy(), count);
            }
        }
    } else {
        println!("  NO LEDGERS YET — run a team first");
    }

    println!("\nSTATUS=SAGCO_AGENT_STATUS_PASS");
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("SAGCO-AGENT v1");
        println!("USE:");
        println!("  sagco-agent status");
        println!("  sagco-agent run red    [--target <path>]");
        println!("  sagco-agent run blue   [--target <path>]");
        println!("  sagco-agent run purple [--ledger data/master_ledger.jsonl]");
        println!("  sagco-agent run gcp    [--project jarvis-swarm-personal]");
        println!("  sagco-agent run all    [--target <path>]");
        std::process::exit(1);
    }

    if args[1] == "status" {
        cmd_status();
        return;
    }

    if args.len() < 3 || args[1] != "run" {
        println!("USE: sagco-agent run <red|blue|purple|gcp|all>");
        std::process::exit(1);
    }

    let team_name = args[2].as_str();
    let target    = args.windows(2).find(|w| w[0] == "--target")
        .map(|w| w[1].clone()).unwrap_or_else(|| "Cargo.toml".to_string());
    let ledger    = args.windows(2).find(|w| w[0] == "--ledger")
        .map(|w| w[1].clone()).unwrap_or_else(|| "data/master_ledger.jsonl".to_string());
    let project   = args.windows(2).find(|w| w[0] == "--project")
        .map(|w| w[1].clone()).unwrap_or_else(|| "jarvis-swarm-personal".to_string());

    let rules_file = ensure_default_rules();

    println!("=== SAGCO-AGENT v1 ===");
    println!("TEAM={}", team_name.to_uppercase());
    println!("TARGET={}", target);
    println!("TIME={}", Utc::now().to_rfc3339());

    let timestamp = Utc::now().to_rfc3339();

    let (all_results, all_pass) = match team_name {
        "red" => {
            let (r, p) = run_team("RED", red_team(&target, &rules_file));
            (r, p)
        }
        "blue" => {
            let (r, p) = run_team("BLUE", blue_team(&target));
            (r, p)
        }
        "purple" => {
            let (r, p) = run_team("PURPLE", purple_team(&ledger));
            (r, p)
        }
        "gcp" => {
            let (r, p) = run_team("GCP", gcp_team(&project));
            (r, p)
        }
        "all" => {
            let mut all_r  = Vec::new();
            let mut all_p  = true;

            let (r, p) = run_team("RED",    red_team(&target, &rules_file)); if !p { all_p = false; } all_r.extend(r);
            let (r, p) = run_team("BLUE",   blue_team(&target));              if !p { all_p = false; } all_r.extend(r);
            let (r, p) = run_team("PURPLE", purple_team(&ledger));            if !p { all_p = false; } all_r.extend(r);

            (all_r, all_p)
        }
        _ => {
            println!("ANTIBODY=UNKNOWN_TEAM_ANTIBODY");
            println!("VALID_TEAMS=red|blue|purple|gcp|all");
            println!("STATUS=SAGCO_AGENT_BAD_TEAM");
            std::process::exit(2);
        }
    };

    // ── Seal and report ───────────────────────────────────────────────────────
    let chain_input: String = all_results.iter()
        .map(|r| format!("{}{}", r.opcode, r.status))
        .collect::<Vec<_>>().join("|");
    let master_seal = seal(&chain_input);

    let overall_status = if all_pass {
        format!("SAGCO_AGENT_{}_PASS", team_name.to_uppercase())
    } else {
        format!("SAGCO_AGENT_{}_PARTIAL", team_name.to_uppercase())
    };

    // Write agent report
    let report = json!({
        "opcode":      "AGENT",
        "timestamp":   timestamp,
        "team":        team_name,
        "target":      target,
        "tasks_run":   all_results.len(),
        "all_pass":    all_pass,
        "results":     all_results,
        "master_seal": master_seal,
        "status":      overall_status,
    });

    let report_text = serde_json::to_string_pretty(&report).unwrap();
    fs::create_dir_all("reports/agent").ok();
    let ts  = Utc::now().format("%Y%m%d_%H%M%S");
    let out = format!("reports/agent/agent_{}_{}.json", team_name, ts);
    fs::write(&out, &report_text).ok();

    // Ledger
    fs::create_dir_all("data").ok();
    let ledger_line = serde_json::to_string(&json!({
        "opcode":    "AGENT",
        "timestamp": Utc::now().to_rfc3339(),
        "team":      team_name,
        "tasks":     all_results.len(),
        "all_pass":  all_pass,
        "report":    out,
        "seal":      master_seal,
        "status":    overall_status,
    })).unwrap() + "\n";

    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/agent_ledger.jsonl")
    {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    println!("\n{}", "=".repeat(50));
    println!("TASKS_RUN={}", report.get("tasks_run").and_then(|v| v.as_u64()).unwrap_or(0));
    println!("ALL_PASS={}", all_pass);
    println!("MASTER_SEAL={}", master_seal);
    println!("REPORT={}", out);
    println!("LEDGER=data/agent_ledger.jsonl");
    println!("STATUS={}", overall_status);
}
