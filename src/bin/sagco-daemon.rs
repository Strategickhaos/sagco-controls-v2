/// sagco-daemon — Persistent SAGCO heartbeat agent
/// Runs the specified team loop on a fixed interval until stopped.
/// USE: sagco-daemon <team> [--interval <secs>] [--once]
///   team = blue | red | purple | self | all
use std::{env, process::Command, time::{Duration, Instant}};
use chrono::Utc;

fn run_bin(bin: &str, args: &[&str]) -> (i32, String) {
    let exe = format!("target/debug/{}.exe", bin);
    let exe_path = if std::path::Path::new(&exe).exists() {
        exe
    } else {
        // Try PATH
        bin.to_string()
    };

    match Command::new(&exe_path).args(args).output() {
        Ok(out) => {
            let code = out.status.code().unwrap_or(-1);
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            (code, text)
        }
        Err(e) => (-1, format!("ERR={}", e)),
    }
}

fn print_status(tick: u64, bin: &str, code: i32, stdout: &str) {
    let ts = Utc::now().to_rfc3339();
    let status = stdout.lines()
        .find(|l| l.starts_with("STATUS="))
        .unwrap_or("STATUS=UNKNOWN");
    let antibody = stdout.lines()
        .find(|l| l.starts_with("ANTIBODY=") && !l.contains("=NONE"))
        .unwrap_or("");

    println!("[{}] TICK={} BIN={} EXIT={} {}", ts, tick, bin, code, status);
    if !antibody.is_empty() {
        println!("  >>> {} <<<", antibody);
    }
}

fn run_team(team: &str, tick: u64) {
    match team {
        "blue" => {
            let (c, o) = run_bin("sagco-guard",       &["4", "10"]);  print_status(tick, "sagco-guard",       c, &o);
            let (c, o) = run_bin("sagco-chainverify", &[]);           print_status(tick, "sagco-chainverify", c, &o);
        }
        "red" => {
            let (c, o) = run_bin("sagco-topofuzz",    &[]);           print_status(tick, "sagco-topofuzz",    c, &o);
        }
        "purple" => {
            let (c, o) = run_bin("sagco-forecast",    &["--ledger", "data/master_ledger.jsonl"]); print_status(tick, "sagco-forecast", c, &o);
            let (c, o) = run_bin("sagco-topology",    &["--json"]);   print_status(tick, "sagco-topology",    c, &o);
        }
        "self" => {
            // The machine reads its own nervous system
            let (c, o) = run_bin("sagco-observe",  &["data/master_ledger.jsonl"]);   print_status(tick, "sagco-observe(ledger)",  c, &o);
            let (c, o) = run_bin("sagco-tokenize", &["data/master_ledger.jsonl"]);   print_status(tick, "sagco-tokenize(ledger)", c, &o);
        }
        "all" => {
            run_team("blue",   tick);
            run_team("red",    tick);
            run_team("purple", tick);
            run_team("self",   tick);
        }
        _ => {
            println!("ANTIBODY=UNKNOWN_TEAM_ANTIBODY TEAM={}", team);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let team = args.get(1).map(|s| s.as_str()).unwrap_or("blue");
    let interval_secs: u64 = args.windows(2)
        .find(|w| w[0] == "--interval")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(30);
    let once = args.iter().any(|a| a == "--once");

    println!("=== SAGCO-DAEMON v1 ===");
    println!("TEAM={}", team);
    println!("INTERVAL={}s", interval_secs);
    println!("ONCE={}", once);
    println!("STARTED={}", Utc::now().to_rfc3339());
    println!("");

    let mut tick = 0u64;

    loop {
        tick += 1;
        let t0 = Instant::now();
        println!("--- HEARTBEAT TICK={} {} ---", tick, Utc::now().to_rfc3339());

        run_team(team, tick);

        let elapsed = t0.elapsed().as_secs();
        println!("TICK={} ELAPSED={}s", tick, elapsed);
        println!("");

        if once { break; }

        let sleep_secs = interval_secs.saturating_sub(elapsed);
        if sleep_secs > 0 {
            std::thread::sleep(Duration::from_secs(sleep_secs));
        }
    }

    println!("STATUS=SAGCO_DAEMON_STOPPED");
}
