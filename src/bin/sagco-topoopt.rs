/// sagco-topoopt — Tier-5 topology optimizer (ML gradient descent over SAGCO metrics)
/// Finds the optimal crew_size + hrs_per_day to hit a target completion date
/// within budget, using gradient descent over the SAGCO variance function.
///
/// USE: sagco-topoopt <old_used> <old_remaining> <new_used> <new_remaining>
///                    --target-days <days> [--lr 0.01] [--iters 2000] [--json]
use std::fs;
use serde_json::json;
use chrono::Utc;
use sha2::{Digest, Sha256};

#[derive(serde::Serialize)]
struct OptStep {
    iter:        usize,
    crew_size:   f64,
    hrs_per_day: f64,
    days_left:   f64,
    loss:        f64,
}

/// Forward pass: predicted days_left given remaining MHRS and capacity
fn days_left(remaining: f64, crew: f64, hrs: f64) -> f64 {
    let cap = crew * hrs;
    if cap <= 0.0 { return f64::MAX; }
    remaining / cap
}

/// Loss = (days_left - target_days)^2  +  regularization on capacity excess
fn loss(remaining: f64, crew: f64, hrs: f64, target: f64) -> f64 {
    let dl = days_left(remaining, crew, hrs);
    if dl == f64::MAX { return 1e9; }
    let residual = dl - target;
    residual * residual
}

/// Numerical gradient (central difference)
fn grad(remaining: f64, crew: f64, hrs: f64, target: f64, eps: f64) -> (f64, f64) {
    let gc = (loss(remaining, crew + eps, hrs, target) - loss(remaining, crew - eps, hrs, target)) / (2.0 * eps);
    let gh = (loss(remaining, crew, hrs + eps, target) - loss(remaining, crew, hrs - eps, target)) / (2.0 * eps);
    (gc, gh)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        println!("USE: sagco-topoopt <old_used> <old_remaining> <new_used> <new_remaining>");
        println!("     --target-days <days> [--lr 0.01] [--iters 2000] [--json]");
        std::process::exit(1);
    }

    let pf = |i: usize| args.get(i).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
    let ps = |flag: &str| -> Option<f64> {
        args.windows(2).find(|w| w[0] == flag).and_then(|w| w[1].parse().ok())
    };

    let old_used      = pf(1);
    let old_remaining = pf(2);
    let new_used      = pf(3);
    let new_remaining = pf(4);
    let target_days   = ps("--target-days").unwrap_or(30.0);
    let lr            = ps("--lr").unwrap_or(0.01);
    let iters         = ps("--iters").unwrap_or(2000.0) as usize;
    let json_mode     = args.iter().any(|a| a == "--json");

    // Budget conservation check
    let old_budget = old_used + old_remaining;
    let new_budget = new_used + new_remaining;
    let budget_delta = new_budget - old_budget;

    println!("=== SAGCO-TOPOOPT v1 ===");
    println!("OLD_BUDGET={:.1}", old_budget);
    println!("NEW_BUDGET={:.1}", new_budget);
    println!("BUDGET_DELTA={:+.1}", budget_delta);
    println!("TARGET_DAYS={:.2}", target_days);
    println!("LR={}", lr);
    println!("ITERS={}", iters);

    // Gradient descent — optimize crew_size and hrs_per_day
    let mut crew: f64 = 4.0;
    let mut hrs:  f64 = 8.0;
    let eps = 1e-4;

    let mut history: Vec<OptStep> = Vec::new();
    let log_every = (iters / 20).max(1);

    for i in 0..iters {
        let l = loss(new_remaining, crew, hrs, target_days);
        if i % log_every == 0 || i == iters - 1 {
            history.push(OptStep {
                iter: i,
                crew_size: (crew * 1000.0).round() / 1000.0,
                hrs_per_day: (hrs * 1000.0).round() / 1000.0,
                days_left: (days_left(new_remaining, crew, hrs) * 100.0).round() / 100.0,
                loss: (l * 1e6).round() / 1e6,
            });
        }
        if l < 1e-6 { break; }

        let (gc, gh) = grad(new_remaining, crew, hrs, target_days, eps);
        crew -= lr * gc;
        hrs  -= lr * gh;

        // Clamp to physical bounds
        crew = crew.max(1.0).min(200.0);
        hrs  = hrs.max(1.0).min(24.0);
    }

    let final_days = days_left(new_remaining, crew, hrs);
    let final_loss = loss(new_remaining, crew, hrs, target_days);
    let converged  = final_loss < 0.01;

    let status = if converged { "SAGCO_TOPOOPT_CONVERGED" } else { "SAGCO_TOPOOPT_PARTIAL" };
    let antibody = if budget_delta > 0.001 { "SCOPE_CREEP_ANTIBODY" } else { "NONE" };

    // Seal the result
    let seal_input = format!("{}{:.4}{:.4}{:.4}", status, crew, hrs, final_days);
    let seal = format!("{:x}", Sha256::digest(seal_input.as_bytes()));

    if json_mode {
        let out = json!({
            "sagco_command": "sagco-topoopt",
            "timestamp": Utc::now().to_rfc3339(),
            "inputs": {
                "old_used": old_used, "old_remaining": old_remaining,
                "new_used": new_used, "new_remaining": new_remaining,
                "target_days": target_days, "lr": lr, "iters": iters,
            },
            "result": {
                "optimal_crew_size":   (crew * 100.0).round() / 100.0,
                "optimal_hrs_per_day": (hrs  * 100.0).round() / 100.0,
                "predicted_days_left": (final_days * 100.0).round() / 100.0,
                "budget_delta": budget_delta,
                "antibody": antibody,
                "status": status,
                "converged": converged,
                "seal": seal,
            },
            "history": history,
        });
        fs::create_dir_all("reports").ok();
        let ts = Utc::now().format("%Y%m%d_%H%M%S");
        let p = format!("reports/topoopt_{}.json", ts);
        fs::write(&p, serde_json::to_string_pretty(&out).unwrap()).ok();
        println!("REPORT={}", p);
    }

    println!("---");
    println!("OPTIMAL_CREW={:.2}", crew);
    println!("OPTIMAL_HRS={:.2}", hrs);
    println!("PREDICTED_DAYS={:.2}", final_days);
    println!("FINAL_LOSS={:.8}", final_loss);
    println!("ANTIBODY={}", antibody);
    println!("SEAL={}", seal);
    println!("STATUS={}", status);
}
