mod models;
mod modules;

use models::ReclassInput;
use modules::{crypto, detector, ledger, reporter};

fn arg(args: &[String], i: usize, name: &str) -> f64 {
    args.get(i)
        .unwrap_or_else(|| panic!("missing arg: {}", name))
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("invalid arg: {}", name))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // ── CLI guard ──────────────────────────────────────────────────────────
    if args.len() == 2 && args[1] == "--verify-chain" {
        let (total, broken, ok) = ledger::verify_chain();
        println!("=== SAGCO LEDGER CHAIN VERIFY ===");
        println!("TOTAL_ENTRIES={}", total);
        println!("BROKEN_LINKS={}", broken);
        println!(
            "STATUS={}",
            if ok { "SAGCO_CHAIN_VERIFIED" } else { "SAGCO_CHAIN_BROKEN" }
        );
        std::process::exit(if ok { 0 } else { 3 });
    }

    if args.len() != 7 {
        println!("SAGCO RECLASSIFICATION DETECTOR v2");
        println!("USE: sagco-reclass <old_used> <old_remaining> <new_used> <new_remaining> <crew_size> <hrs_per_day>");
        println!("     sagco-reclass --verify-chain");
        std::process::exit(1);
    }

    // ── 1. MEASUREMENT — parse inputs ──────────────────────────────────────
    let input = ReclassInput {
        old_used:      arg(&args, 1, "old_used"),
        old_remaining: arg(&args, 2, "old_remaining"),
        new_used:      arg(&args, 3, "new_used"),
        new_remaining: arg(&args, 4, "new_remaining"),
        crew_size:     arg(&args, 5, "crew_size"),
        hrs_per_day:   arg(&args, 6, "hrs_per_day"),
    };

    // ── 2. ANALYSIS — classify variance ───────────────────────────────────
    let result = match detector::detect(&input) {
        Some(r) => r,
        None => {
            println!("ANTIBODY=ZERO_CAPACITY_ANTIBODY");
            println!("STATUS=SAGCO_BAD_CAPACITY_FAIL");
            std::process::exit(2);
        }
    };

    // ── 3. REPORTING — write JSON + MD to reports/ ─────────────────────────
    std::fs::create_dir_all("reports").expect("cannot create reports/");
    let paths = reporter::generate(&input, &result);

    // ── 4. SEALING — SHA256 fingerprint of JSON report ────────────────────
    let seal = crypto::seal_file(&paths.json_path);

    // ── 5. PERSISTENCE — append to master_ledger.jsonl ────────────────────
    ledger::commit(
        &result.status,
        &result.antibody,
        result.shift_mhrs,
        result.day_variance,
        &paths.json_path,
        &paths.md_path,
        &seal,
    );

    // ── STDOUT — full audit output ─────────────────────────────────────────
    println!("=== SAGCO RECLASSIFICATION DETECTOR v2 ===");
    println!("OLD_USED={:.1}",       input.old_used);
    println!("OLD_REMAINING={:.1}",  input.old_remaining);
    println!("OLD_BUDGET={:.1}",     result.old_budget);
    println!("NEW_USED={:.1}",       input.new_used);
    println!("NEW_REMAINING={:.1}",  input.new_remaining);
    println!("NEW_BUDGET={:.1}",     result.new_budget);
    println!("BUDGET_DELTA={:+.1}",  result.budget_delta);
    println!("---");
    println!("SHIFT_MHRS={:.1}",      result.shift_mhrs);
    println!("DAILY_CAPACITY={:.1}",  result.daily_capacity);
    println!("OLD_DAYS_LEFT={:.2}",   result.old_days_left);
    println!("NEW_DAYS_LEFT={:.2}",   result.new_days_left);
    println!("DAY_VARIANCE={:+.2}",   result.day_variance);
    println!("---");
    println!("ANTIBODY={}",           result.antibody);
    println!("STATUS={}",             result.status);
    println!("---");
    println!("REPORT_JSON={}",        paths.json_path);
    println!("REPORT_MD={}",          paths.md_path);
    println!("SEAL={}",               seal);
    println!("LEDGER=data/master_ledger.jsonl");
    println!("STATUS_AUDIT=SAGCO_AUDIT_COMMIT_PASS");
}
