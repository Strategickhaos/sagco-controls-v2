use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReclassInput {
    pub old_used: f64,
    pub old_remaining: f64,
    pub new_used: f64,
    pub new_remaining: f64,
    pub crew_size: f64,
    pub hrs_per_day: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReclassResult {
    pub old_budget: f64,
    pub new_budget: f64,
    pub budget_delta: f64,
    pub shift_mhrs: f64,
    pub daily_capacity: f64,
    pub old_days_left: f64,
    pub new_days_left: f64,
    pub day_variance: f64,
    pub antibody: String,
    pub status: String,
}
