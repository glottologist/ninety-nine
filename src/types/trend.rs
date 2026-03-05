use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Stable,
    Degrading,
}

impl std::fmt::Display for TrendDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Improving => write!(f, "improving"),
            Self::Stable => write!(f, "stable"),
            Self::Degrading => write!(f, "degrading"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendSummary {
    pub test_name: String,
    pub direction: TrendDirection,
    pub recent_score: f64,
    pub previous_score: f64,
    pub score_delta: f64,
    pub window_runs: u64,
}
