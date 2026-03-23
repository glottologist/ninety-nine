use std::path::PathBuf;
use std::str::FromStr;

use uuid::Uuid;

use super::{ms_to_duration, parse_timestamp};
use crate::types::{
    BayesianParams, FlakinessScore, TestEnvironment, TestName, TestOutcome, TestRun,
};

pub struct RawTestRunRow {
    pub id: String,
    pub test_name: String,
    pub test_path: String,
    pub outcome: String,
    pub duration_ms: i64,
    pub timestamp: String,
    pub commit_hash: String,
    pub branch: String,
    pub retry_count: u32,
    pub error_message: Option<String>,
    pub stack_trace: Option<String>,
    pub env_os: String,
    pub env_rust_version: String,
    pub env_cpu_count: u32,
    pub env_memory_gb: f64,
    pub env_is_ci: bool,
    pub env_ci_provider: Option<String>,
}

impl RawTestRunRow {
    pub fn into_test_run(self) -> TestRun {
        let id = Uuid::parse_str(&self.id).unwrap_or_else(|e| {
            tracing::warn!(id = %self.id, error = %e, "corrupt UUID in storage, generating new");
            Uuid::new_v4()
        });
        let outcome = TestOutcome::from_str(&self.outcome).unwrap_or_else(|e| {
            tracing::warn!(outcome = %self.outcome, error = %e, "unrecognized test outcome in storage, defaulting to Failed");
            TestOutcome::Failed
        });

        TestRun {
            id,
            test_name: TestName::from(self.test_name),
            test_path: PathBuf::from(self.test_path),
            outcome,
            duration: ms_to_duration(self.duration_ms),
            timestamp: parse_timestamp(&self.timestamp),
            commit_hash: self.commit_hash,
            branch: self.branch,
            environment: TestEnvironment {
                os: self.env_os,
                rust_version: self.env_rust_version,
                cpu_count: self.env_cpu_count,
                memory_gb: self.env_memory_gb,
                is_ci: self.env_is_ci,
                ci_provider: self.env_ci_provider,
            },
            retry_count: self.retry_count,
            error_message: self.error_message,
            stack_trace: self.stack_trace,
        }
    }
}

pub struct RawScoreRow {
    pub test_name: String,
    pub probability_flaky: f64,
    pub confidence: f64,
    pub pass_rate: f64,
    pub fail_rate: f64,
    pub total_runs: u64,
    pub consecutive_failures: u32,
    pub last_updated: String,
    pub alpha: f64,
    pub beta: f64,
    pub posterior_mean: f64,
    pub posterior_variance: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
}

#[cfg(test)]
fn default_raw_test_run() -> RawTestRunRow {
    RawTestRunRow {
        id: "00000000-0000-0000-0000-000000000000".to_string(),
        test_name: "test".to_string(),
        test_path: "/tmp/bin".to_string(),
        outcome: "passed".to_string(),
        duration_ms: 10,
        timestamp: "2026-01-01T00:00:00Z".to_string(),
        commit_hash: String::new(),
        branch: String::new(),
        retry_count: 0,
        error_message: None,
        stack_trace: None,
        env_os: "linux".to_string(),
        env_rust_version: "1.85.0".to_string(),
        env_cpu_count: 8,
        env_memory_gb: 16.0,
        env_is_ci: false,
        env_ci_provider: None,
    }
}

impl RawScoreRow {
    pub fn into_score(self) -> FlakinessScore {
        FlakinessScore {
            test_name: TestName::from(self.test_name),
            probability_flaky: self.probability_flaky,
            confidence: self.confidence,
            pass_rate: self.pass_rate,
            fail_rate: self.fail_rate,
            total_runs: self.total_runs,
            consecutive_failures: self.consecutive_failures,
            last_updated: parse_timestamp(&self.last_updated),
            bayesian_params: BayesianParams {
                alpha: self.alpha,
                beta: self.beta,
                posterior_mean: self.posterior_mean,
                posterior_variance: self.posterior_variance,
                credible_interval_lower: self.ci_lower,
                credible_interval_upper: self.ci_upper,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_test_run_handles_corrupt_uuid() {
        let raw = RawTestRunRow {
            id: "not-a-uuid".to_string(),
            ..default_raw_test_run()
        };
        let run = raw.into_test_run();
        assert_ne!(run.id.to_string(), "not-a-uuid");
    }

    #[test]
    fn into_test_run_handles_unrecognized_outcome() {
        let raw = RawTestRunRow {
            outcome: "Exploded".to_string(),
            ..default_raw_test_run()
        };
        let run = raw.into_test_run();
        assert_eq!(run.outcome, TestOutcome::Failed);
    }

    #[test]
    fn into_test_run_preserves_valid_data() {
        let raw = default_raw_test_run();
        let run = raw.into_test_run();
        assert_eq!(run.outcome, TestOutcome::Passed);
        assert_eq!(run.test_name, "test");
        assert_eq!(run.retry_count, 0);
    }
}
