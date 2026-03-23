use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::test_name::TestName;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakinessScore {
    pub test_name: TestName,
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

impl FlakinessScore {
    /// Returns the score used for display and categorisation.
    ///
    /// When confidence is below the threshold, the Bayesian posterior is
    /// dominated by the prior and misleading. Fall back to the observed
    /// fail rate instead.
    #[must_use]
    pub fn effective_score(&self, confidence_threshold: f64) -> f64 {
        if self.confidence >= confidence_threshold {
            self.probability_flaky
        } else {
            self.fail_rate
        }
    }
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
    #[must_use]
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s < 0.01 => Self::Stable,
            s if s < 0.05 => Self::Occasional,
            s if s < 0.15 => Self::Moderate,
            s if s < 0.30 => Self::Frequent,
            _ => Self::Critical,
        }
    }

    #[must_use]
    pub const fn label(&self) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_score;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn effective_score_selects_correct_source(
            probability in 0.0f64..=1.0,
            fail_rate in 0.0f64..=1.0,
            confidence in 0.0f64..=1.0,
            threshold in 0.0f64..=1.0,
        ) {
            let mut score = test_score("test", probability);
            score.confidence = confidence;
            score.fail_rate = fail_rate;

            let effective = score.effective_score(threshold);
            if confidence >= threshold {
                prop_assert!((effective - probability).abs() < f64::EPSILON);
            } else {
                prop_assert!((effective - fail_rate).abs() < f64::EPSILON);
            }
        }

        #[test]
        fn from_score_always_returns_valid_category(score in 0.0f64..=1.0) {
            let category = FlakinessCategory::from_score(score);
            let label = category.label();
            prop_assert!(!label.is_empty());

            if score < 0.01 {
                prop_assert_eq!(category, FlakinessCategory::Stable);
            } else if score < 0.05 {
                prop_assert_eq!(category, FlakinessCategory::Occasional);
            } else if score < 0.15 {
                prop_assert_eq!(category, FlakinessCategory::Moderate);
            } else if score < 0.30 {
                prop_assert_eq!(category, FlakinessCategory::Frequent);
            } else {
                prop_assert_eq!(category, FlakinessCategory::Critical);
            }
        }
    }
}
