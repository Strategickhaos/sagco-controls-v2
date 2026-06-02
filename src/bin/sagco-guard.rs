/// sagco-guard — Input firewall (Tier-0 antibody gate)
/// Validates crew_size and hrs_per_day before any math runs downstream.
/// USE: sagco-guard <crew_size> <hrs_per_day>
use serde_json::json;
use chrono::Utc;
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 3 {
        println!("USE: sagco-guard <crew_size> <hrs_per_day>");
        std::process::exit(1);
    }

    println!("=== SAGCO-GUARD v1 ===");

    let crew: f64 = match args[1].parse() {
        Ok(v) => v,
        Err(_) => {
            println!("ANTIBODY=PARSE_ANTIBODY");
            println!("INPUT=crew_size={}", args[1]);
            println!("STATUS=SAGCO_BAD_INPUT_FAIL");
            std::process::exit(2);
        }
    };

    let hrs: f64 = match args[2].parse() {
        Ok(v) => v,
        Err(_) => {
            println!("ANTIBODY=PARSE_ANTIBODY");
            println!("INPUT=hrs_per_day={}", args[2]);
            println!("STATUS=SAGCO_BAD_INPUT_FAIL");
            std::process::exit(2);
        }
    };

    println!("CREW_SIZE={}", crew);
    println!("HRS_PER_DAY={}", hrs);

    // ── Antibody checks ───────────────────────────────────────────────────
    if crew <= 0.0 {
        println!("ANTIBODY=ZERO_CAPACITY_ANTIBODY");
        println!("REASON=crew_size must be > 0");
        println!("STATUS=SAGCO_BAD_CAPACITY_FAIL");
        std::process::exit(2);
    }

    if hrs <= 0.0 {
        println!("ANTIBODY=ZERO_CAPACITY_ANTIBODY");
        println!("REASON=hrs_per_day must be > 0");
        println!("STATUS=SAGCO_BAD_CAPACITY_FAIL");
        std::process::exit(2);
    }

    if crew > 10_000.0 {
        println!("ANTIBODY=OVERFLOW_CAPACITY_ANTIBODY");
        println!("REASON=crew_size exceeds physical ceiling 10000");
        println!("STATUS=SAGCO_BAD_CAPACITY_FAIL");
        std::process::exit(2);
    }

    if hrs > 24.0 {
        println!("ANTIBODY=OVERFLOW_CAPACITY_ANTIBODY");
        println!("REASON=hrs_per_day exceeds 24");
        println!("STATUS=SAGCO_BAD_CAPACITY_FAIL");
        std::process::exit(2);
    }

    if crew.is_nan() || hrs.is_nan() || crew.is_infinite() || hrs.is_infinite() {
        println!("ANTIBODY=NAN_INF_ANTIBODY");
        println!("STATUS=SAGCO_BAD_CAPACITY_FAIL");
        std::process::exit(2);
    }

    // ── All checks passed — compute clean capacity ────────────────────────
    let daily_capacity = crew * hrs;

    let report = json!({
        "opcode": "GUARD",
        "timestamp": Utc::now().to_rfc3339(),
        "inputs": { "crew_size": crew, "hrs_per_day": hrs },
        "daily_capacity": daily_capacity,
        "antibody": "NONE",
        "status": "SAGCO_GUARD_PASS"
    });

    fs::create_dir_all("reports").ok();
    let ts = Utc::now().format("%Y%m%d_%H%M%S");
    let path = format!("reports/guard_{}.json", ts);
    fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).ok();

    println!("DAILY_CAPACITY={}", daily_capacity);
    println!("REPORT={}", path);
    println!("STATUS=SAGCO_GUARD_PASS");
}
