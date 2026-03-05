use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakinessScore {
    pub test_name: String,
    pub probability_flaky: f64,
    pub confidence: f64,
    pub pass_rate: f64,
    pub fail_rate: f64,
    pub total_runs: u64,
    pub consecutive_failures: u32,
    pub last_updated: DateTime<Utc>,
    pub bayesian_params: BayesianParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BayesianParams {
    pub alpha: f64,
    pub beta: f64,
    pub posterior_mean: f64,
    pub posterior_variance: f64,
    pub credible_interval_lower: f64,
    pub credible_interval_upper: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FlakinessCategory {
    Stable,
    Occasional,
    Moderate,
    Frequent,
    Critical,
}

impl FlakinessCategory {
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s < 0.01 => Self::Stable,
            s if s < 0.05 => Self::Occasional,
            s if s < 0.15 => Self::Moderate,
            s if s < 0.30 => Self::Frequent,
            _ => Self::Critical,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Stable => "Stable",
            Self::Occasional => "Occasional",
            Self::Moderate => "Moderate",
            Self::Frequent => "Frequent",
            Self::Critical => "Critical",
        }
    }
}

impl std::fmt::Display for FlakinessCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}
