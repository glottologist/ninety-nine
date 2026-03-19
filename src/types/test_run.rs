use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::test_name::TestName;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestRun {
    pub id: Uuid,
    pub test_name: TestName,
    pub test_path: PathBuf,
    pub outcome: TestOutcome,
    pub duration: Duration,
    pub timestamp: DateTime<Utc>,
    pub commit_hash: String,
    pub branch: String,
    pub environment: TestEnvironment,
    pub retry_count: u32,
    pub error_message: Option<String>,
    pub stack_trace: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TestOutcome {
    Passed,
    Failed,
    Ignored,
    Timeout,
    Panic,
}

impl std::fmt::Display for TestOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Passed => write!(f, "passed"),
            Self::Failed => write!(f, "failed"),
            Self::Ignored => write!(f, "ignored"),
            Self::Timeout => write!(f, "timeout"),
            Self::Panic => write!(f, "panic"),
        }
    }
}

impl std::str::FromStr for TestOutcome {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            "ignored" => Ok(Self::Ignored),
            "timeout" => Ok(Self::Timeout),
            "panic" => Ok(Self::Panic),
            _ => Err(format!("unknown test outcome: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_outcome() -> impl Strategy<Value = TestOutcome> {
        prop_oneof![
            Just(TestOutcome::Passed),
            Just(TestOutcome::Failed),
            Just(TestOutcome::Ignored),
            Just(TestOutcome::Timeout),
            Just(TestOutcome::Panic),
        ]
    }

    proptest! {
        #[test]
        fn outcome_display_fromstr_roundtrip(outcome in arb_outcome()) {
            let displayed = outcome.to_string();
            let parsed: TestOutcome = displayed.parse().unwrap();
            prop_assert_eq!(outcome, parsed);
        }

        #[test]
        fn invalid_outcome_returns_err(s in "[a-z]{6,20}") {
            let valid = ["passed", "failed", "ignored", "timeout", "panic"];
            if !valid.contains(&s.as_str()) {
                prop_assert!(s.parse::<TestOutcome>().is_err());
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestEnvironment {
    pub os: String,
    pub rust_version: String,
    pub cpu_count: u32,
    pub memory_gb: f64,
    pub is_ci: bool,
    pub ci_provider: Option<String>,
}
