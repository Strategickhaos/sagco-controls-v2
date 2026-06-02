/// sagco-forecast — Purple Team opcode: predict variance from historical ledger data
/// Reads a JSONL ledger of past reclass runs, fits a simple linear trend on
/// budget_delta and day_variance, outputs predicted next variance.
///
/// USE: sagco-forecast [--ledger data/master_ledger.jsonl] [--json]
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{env, fs, io::Write};

/// Simple ordinary least squares: y = mx + b
/// Returns (slope m, intercept b, r_squared)
fn linear_regression(xs: &[f64], ys: &[f64]) -> (f64, f64, f64) {
    let n = xs.len() as f64;
    if n < 2.0 { return (0.0, ys.first().copied().unwrap_or(0.0), 0.0); }

    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;

    let ss_xx: f64 = xs.iter().map(|x| (x - mean_x).powi(2)).sum();
    let ss_xy: f64 = xs.iter().zip(ys.iter()).map(|(x, y)| (x - mean_x) * (y - mean_y)).sum();
    let ss_yy: f64 = ys.iter().map(|y| (y - mean_y).powi(2)).sum();

    if ss_xx.abs() < 1e-12 { return (0.0, mean_y, 0.0); }

    let m = ss_xy / ss_xx;
    let b = mean_y - m * mean_x;
    let r_sq = if ss_yy.abs() < 1e-12 { 1.0 } else { (ss_xy * ss_xy) / (ss_xx * ss_yy) };

    (m, b, r_sq.clamp(0.0, 1.0))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let ledger_path: String = args.windows(2)
        .find(|w| w[0] == "--ledger")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "data/master_ledger.jsonl".to_string());
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-FORECAST v1 ===");
    println!("LEDGER={}", ledger_path);

    let raw = match fs::read_to_string(&ledger_path) {
        Ok(r) => r,
        Err(e) => {
            println!("ANTIBODY=FORECAST_READ_ANTIBODY");
            println!("ERROR={}", e);
            println!("STATUS=SAGCO_FORECAST_NO_DATA");
            std::process::exit(2);
        }
    };

    let entries: Vec<serde_json::Value> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    if entries.len() < 2 {
        println!("ANTIBODY=FORECAST_INSUFFICIENT_DATA_ANTIBODY");
        println!("ENTRIES={}", entries.len());
        println!("MIN_REQUIRED=2");
        println!("STATUS=SAGCO_FORECAST_INSUFFICIENT_DATA");
        std::process::exit(2);
    }

    // Extract numeric series
    let indices: Vec<f64> = (0..entries.len()).map(|i| i as f64).collect();

    let budget_deltas: Vec<f64> = entries.iter()
        .map(|e| e["budget_delta"].as_f64().unwrap_or(0.0))
        .collect();

    let day_variances: Vec<f64> = entries.iter()
        .map(|e| e["day_variance"].as_f64().unwrap_or(0.0))
        .collect();

    let shift_mhrs: Vec<f64> = entries.iter()
        .map(|e| e["shift_mhrs"].as_f64().unwrap_or(0.0))
        .collect();

    // Fit linear trends
    let (bd_m, bd_b, bd_r2) = linear_regression(&indices, &budget_deltas);
    let (dv_m, dv_b, dv_r2) = linear_regression(&indices, &day_variances);
    let (sm_m, sm_b, sm_r2) = linear_regression(&indices, &shift_mhrs);

    // Predict next (n+1)th entry
    let next_idx = entries.len() as f64;
    let pred_budget_delta  = bd_m * next_idx + bd_b;
    let pred_day_variance  = dv_m * next_idx + dv_b;
    let pred_shift_mhrs    = sm_m * next_idx + sm_b;

    // Confidence = average r_squared across models
    let confidence = (bd_r2 + dv_r2 + sm_r2) / 3.0;

    // Forecast classification
    let forecast_class = if pred_budget_delta > 0.001 {
        "FORECAST_SCOPE_CREEP_TREND"
    } else if pred_budget_delta < -0.001 {
        "FORECAST_SCOPE_REDUCTION_TREND"
    } else if pred_shift_mhrs.abs() > 0.001 {
        "FORECAST_RECLASSIFICATION_TREND"
    } else {
        "FORECAST_STABLE"
    };

    let timestamp  = Utc::now().to_rfc3339();
    let seal_input = format!("{}{:.4}{:.4}{:.4}", ledger_path, pred_budget_delta, pred_day_variance, confidence);
    let seal       = format!("{:x}", Sha256::digest(seal_input.as_bytes()));

    if json_mode {
        let report = json!({
            "opcode":     "FORECAST",
            "timestamp":  timestamp,
            "ledger":     ledger_path,
            "data_points": entries.len(),
            "models": {
                "budget_delta":  { "slope": (bd_m*10000.0).round()/10000.0, "intercept": (bd_b*10000.0).round()/10000.0, "r_squared": (bd_r2*1000.0).round()/1000.0 },
                "day_variance":  { "slope": (dv_m*10000.0).round()/10000.0, "intercept": (dv_b*10000.0).round()/10000.0, "r_squared": (dv_r2*1000.0).round()/1000.0 },
                "shift_mhrs":    { "slope": (sm_m*10000.0).round()/10000.0, "intercept": (sm_b*10000.0).round()/10000.0, "r_squared": (sm_r2*1000.0).round()/1000.0 },
            },
            "predictions": {
                "next_budget_delta": (pred_budget_delta*100.0).round()/100.0,
                "next_day_variance": (pred_day_variance*100.0).round()/100.0,
                "next_shift_mhrs":   (pred_shift_mhrs*100.0).round()/100.0,
            },
            "confidence":     (confidence*1000.0).round()/1000.0,
            "forecast_class": forecast_class,
            "seal":           seal,
            "status":         "SAGCO_FORECAST_PASS",
        });
        let report_text = serde_json::to_string_pretty(&report).unwrap();
        fs::create_dir_all("reports").ok();
        let ts  = Utc::now().format("%Y%m%d_%H%M%S");
        let out = format!("reports/forecast_{}.json", ts);
        fs::write(&out, &report_text).ok();
        println!("{}", report_text);
        println!("REPORT={}", out);
    }

    // Ledger
    fs::create_dir_all("data").ok();
    let ledger_line = serde_json::to_string(&json!({
        "opcode":         "FORECAST",
        "timestamp":      timestamp,
        "ledger":         ledger_path,
        "data_points":    entries.len(),
        "pred_delta":     (pred_budget_delta*100.0).round()/100.0,
        "pred_day_var":   (pred_day_variance*100.0).round()/100.0,
        "confidence":     (confidence*1000.0).round()/1000.0,
        "forecast_class": forecast_class,
        "seal":           seal,
        "status":         "SAGCO_FORECAST_PASS",
    })).unwrap() + "\n";
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/forecast_ledger.jsonl")
    {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    println!("DATA_POINTS={}", entries.len());
    println!("PRED_BUDGET_DELTA={:+.2}", pred_budget_delta);
    println!("PRED_DAY_VARIANCE={:+.2}", pred_day_variance);
    println!("PRED_SHIFT_MHRS={:+.2}", pred_shift_mhrs);
    println!("CONFIDENCE={:.3}", confidence);
    println!("SEAL={}", seal);
    println!("STATUS=SAGCO_FORECAST_PASS");
    println!("FORECAST_CLASS={}", forecast_class);
}
