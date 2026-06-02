/// sagco-stepper — State machine runner: executes an opcode plan tick by tick
/// Each tick feeds output state into the next opcode.
/// USE: sagco-stepper <target_path> [--plan default|forensic|controls] [--json]
///
/// Plans:
///   default  = GUARD → OBSERVE → TOKENIZE
///   forensic = GUARD → OBSERVE → TOKENIZE → BINSCAN
///   controls = GUARD → OBSERVE → TOKENIZE → RECLASS
use std::{env, process::Command, fs, io::Write};
use sha2::{Digest, Sha256};
use serde_json::json;
use chrono::Utc;

#[derive(serde::Serialize)]
struct Tick {
    tick:    usize,
    opcode:  String,
    status:  String,
    stdout:  String,
    exit:    i32,
}

fn plan_opcodes(plan: &str) -> Vec<&'static str> {
    match plan {
        "forensic" => vec!["observe", "tokenize", "binscan"],
        "controls" => vec!["observe", "tokenize", "reclass"],
        _          => vec!["observe", "tokenize"],  // default
    }
}

fn run_opcode(bin: &str, args: &[&str]) -> (i32, String) {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--quiet", "--bin"])
       .arg(format!("sagco-{}", bin))
       .arg("--");
    for a in args { cmd.arg(a); }

    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let code   = out.status.code().unwrap_or(-1);
            (code, stdout)
        }
        Err(e) => (-1, format!("EXEC_ERROR={}", e)),
    }
}

fn extract_status(stdout: &str) -> String {
    stdout.lines()
        .find(|l| l.starts_with("STATUS="))
        .map(|l| l.trim_start_matches("STATUS=").to_string())
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("USE: sagco-stepper <target_path> [--plan default|forensic|controls] [--json]");
        std::process::exit(1);
    }

    let target    = &args[1];
    let plan_name = args.windows(2)
        .find(|w| w[0] == "--plan")
        .map(|w| w[1].as_str())
        .unwrap_or("default");
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-STEPPER v1 ===");
    println!("TARGET={}", target);
    println!("PLAN={}", plan_name);

    let opcodes = plan_opcodes(plan_name);
    let mut ticks: Vec<Tick> = Vec::new();
    let timestamp = Utc::now().to_rfc3339();

    for (i, opcode) in opcodes.iter().enumerate() {
        let tick_no = i + 1;
        println!("--- TICK={} OPCODE={} ---", tick_no, opcode.to_uppercase());

        let (exit, stdout) = run_opcode(opcode, &[target]);
        let status = extract_status(&stdout);

        println!("{}", stdout.trim());

        let tick = Tick {
            tick:   tick_no,
            opcode: opcode.to_uppercase(),
            status: status.clone(),
            stdout: stdout.clone(),
            exit,
        };
        ticks.push(tick);

        // Abort plan on any non-zero exit
        if exit != 0 {
            println!("STEPPER_ABORT=TICK_{}_FAILED", tick_no);
            break;
        }
    }

    let all_pass = ticks.iter().all(|t| t.exit == 0);
    let final_status = if all_pass { "SAGCO_STEPPER_PASS" } else { "SAGCO_STEPPER_PARTIAL" };

    // Seal the full step chain
    let chain_input: String = ticks.iter().map(|t| t.status.as_str()).collect::<Vec<_>>().join("|");
    let seal = format!("{:x}", Sha256::digest(chain_input.as_bytes()));

    if json_mode {
        let report = json!({
            "opcode": "STEPPER",
            "timestamp": timestamp,
            "target": target,
            "plan": plan_name,
            "ticks_run": ticks.len(),
            "all_pass": all_pass,
            "seal": seal,
            "ticks": ticks,
        });
        fs::create_dir_all("reports").ok();
        let ts = Utc::now().format("%Y%m%d_%H%M%S");
        let p = format!("reports/stepper_{}_{}.json", plan_name, ts);
        fs::write(&p, serde_json::to_string_pretty(&report).unwrap()).ok();
        println!("REPORT={}", p);
    }

    // Ledger append
    fs::create_dir_all("data").ok();
    let ledger_line = serde_json::to_string(&json!({
        "opcode":    "STEPPER",
        "timestamp": timestamp,
        "target":    target,
        "plan":      plan_name,
        "seal":      seal,
        "status":    final_status,
    })).unwrap() + "\n";
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/stepper_ledger.jsonl")
    {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    println!("---");
    println!("TICKS_RUN={}", ticks.len());
    println!("CHAIN_SEAL={}", seal);
    println!("STATUS={}", final_status);
}
