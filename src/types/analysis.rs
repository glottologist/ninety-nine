use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    pub pattern_type: PatternType,
    pub occurrences: u32,
    pub correlation: f64,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternType {
    TimeOfDay,
    Environmental,
    Random,
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimeOfDay => write!(f, "time-of-day"),
            Self::Environmental => write!(f, "environmental"),
            Self::Random => write!(f, "random"),
        }
    }
}
