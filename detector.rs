use crate::models::{ReclassInput, ReclassResult};

pub fn detect(input: &ReclassInput) -> ReclassResult {
    let old_budget    = input.old_used + input.old_remaining;
    let new_budget    = input.new_used + input.new_remaining;
    let daily_cap     = input.crew_size * input.hrs_per_day;
    let shift_mhrs    = input.old_used - input.new_used;
    let old_days_left = input.old_remaining / daily_cap;
    let new_days_left = input.new_remaining / daily_cap;
    let day_variance  = new_days_left - old_days_left;
    let budget_delta  = new_budget - old_budget;

    // Three-state classification — budget conservation is the gate
    let (status, antibody) = if budget_delta > 0.001 {
        (
            "SAGCO_SCOPE_CREEP_DETECTED",
            "SCOPE_CREEP_ANTIBODY",
        )
    } else if budget_delta < -0.001 {
        (
            "SAGCO_SCOPE_REDUCTION_DETECTED",
            "SCOPE_REDUCTION_ANTIBODY",
        )
    } else if shift_mhrs.abs() > 0.001 {
        (
            "SAGCO_RECLASSIFICATION_DETECTED",
            "NONE_SCOPE_CREEP",
        )
    } else {
        (
            "SAGCO_NO_CHANGE_DETECTED",
            "NONE",
        )
    };

    ReclassResult {
        old_budget,
        new_budget,
        budget_delta,
        shift_mhrs,
        daily_capacity: daily_cap,
        old_days_left,
        new_days_left,
        day_variance,
        antibody: antibody.to_string(),
        status: status.to_string(),
    }
}
