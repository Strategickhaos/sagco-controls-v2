use std::fs;
use crate::models::{ReclassInput, ReclassResult};
use chrono::Utc;
use serde_json::json;

pub struct ReportPaths {
    pub json_path: String,
    pub md_path: String,
    pub report_json: String,
}

pub fn generate(input: &ReclassInput, result: &ReclassResult) -> ReportPaths {
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let ts_human  = Utc::now().to_rfc3339();

    let json_path = format!("reports/reclass_{}.json", timestamp);
    let md_path   = format!("reports/reclass_{}.md",   timestamp);

    // --- JSON report ---
    let report_json = serde_json::to_string_pretty(&json!({
        "sagco_version":  "controls-v2",
        "report_type":    "RECLASSIFICATION_EUR",
        "timestamp":      ts_human,
        "input": {
            "old_used":      input.old_used,
            "old_remaining": input.old_remaining,
            "new_used":      input.new_used,
            "new_remaining": input.new_remaining,
            "crew_size":     input.crew_size,
            "hrs_per_day":   input.hrs_per_day,
        },
        "result": {
            "old_budget":      result.old_budget,
            "new_budget":      result.new_budget,
            "budget_delta":    result.budget_delta,
            "shift_mhrs":      result.shift_mhrs,
            "daily_capacity":  result.daily_capacity,
            "old_days_left":   result.old_days_left,
            "new_days_left":   result.new_days_left,
            "day_variance":    result.day_variance,
            "antibody":        result.antibody,
            "status":          result.status,
        }
    })).unwrap();

    fs::write(&json_path, &report_json).expect("reporter: cannot write JSON");

    // --- Markdown report ---
    let md = format!(
"# SAGCO Reclassification EUR Report
**Generated:** {}

## Input Snapshot

| Field | Value |
|---|---:|
| Old Used | {:.1} MHRS |
| Old Remaining | {:.1} MHRS |
| Old Budget | {:.1} MHRS |
| New Used | {:.1} MHRS |
| New Remaining | {:.1} MHRS |
| New Budget | {:.1} MHRS |
| Crew Size | {:.0} men |
| Hrs / Day | {:.0} hrs |

## Variance Analysis

| Metric | Value |
|---|---:|
| Budget Delta | {:+.1} MHRS |
| Shift MHRS | {:.1} MHRS |
| Daily Capacity | {:.1} MHRS |
| Old Days Left | {:.2} days |
| New Days Left | {:.2} days |
| Day Variance | {:+.2} days |

## Classification

```
ANTIBODY = {}
STATUS   = {}
```
",
        ts_human,
        input.old_used, input.old_remaining, result.old_budget,
        input.new_used, input.new_remaining, result.new_budget,
        input.crew_size, input.hrs_per_day,
        result.budget_delta,
        result.shift_mhrs,
        result.daily_capacity,
        result.old_days_left,
        result.new_days_left,
        result.day_variance,
        result.antibody,
        result.status,
    );

    fs::write(&md_path, &md).expect("reporter: cannot write MD");

    ReportPaths { json_path, md_path, report_json }
}
