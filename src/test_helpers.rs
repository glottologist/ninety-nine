use crate::types::{
    ActiveSession, BayesianParams, FlakinessScore, RunSession, TestEnvironment, TestName,
    TestOutcome, TestRun,
};
use chrono::{TimeZone, Utc};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

#[must_use]
pub fn test_environment() -> TestEnvironment {
    TestEnvironment {
        os: "linux".to_string(),
        rust_version: "1.85.0".to_string(),
        cpu_count: 8,
        memory_gb: 16.0,
        is_ci: false,
        ci_provider: None,
    }
}

#[must_use]
pub fn test_environment_ci(is_ci: bool) -> TestEnvironment {
    TestEnvironment {
        is_ci,
        ci_provider: if is_ci {
            Some("GitHub Actions".to_string())
        } else {
            None
        },
        ..test_environment()
    }
}

#[must_use]
pub fn test_run(name: &str, outcome: TestOutcome) -> TestRun {
    TestRun {
        id: Uuid::new_v4(),
        test_name: TestName::from(name),
        test_path: PathBuf::new(),
        outcome,
        duration: Duration::from_millis(10),
        timestamp: Utc::now(),
        commit_hash: String::new(),
        branch: String::new(),
        environment: test_environment(),
        retry_count: 0,
        error_message: None,
        stack_trace: None,
    }
}

#[must_use]
pub fn test_run_with_duration(name: &str, duration_ms: u64) -> TestRun {
    TestRun {
        duration: Duration::from_millis(duration_ms),
        outcome: TestOutcome::Passed,
        ..test_run(name, TestOutcome::Passed)
    }
}

#[must_use]
pub fn test_run_at_hour(name: &str, outcome: TestOutcome, hour: u32) -> TestRun {
    TestRun {
        timestamp: Utc.with_ymd_and_hms(2026, 3, 5, hour, 0, 0).unwrap(),
        environment: test_environment_ci(false),
        ..test_run(name, outcome)
    }
}

#[must_use]
pub fn test_run_in_env(name: &str, outcome: TestOutcome, is_ci: bool) -> TestRun {
    TestRun {
        environment: test_environment_ci(is_ci),
        ..test_run(name, outcome)
    }
}

#[must_use]
pub fn test_run_for_storage(name: &str, outcome: TestOutcome) -> TestRun {
    TestRun {
        test_path: PathBuf::from("/tmp/test-binary"),
        duration: Duration::from_millis(42),
        commit_hash: "abc123".to_string(),
        branch: "main".to_string(),
        ..test_run(name, outcome)
    }
}

#[must_use]
pub fn test_runs_by_outcome(passes: u32, failures: u32) -> Vec<TestRun> {
    let total = usize::try_from(passes + failures).unwrap_or(0);
    let mut runs = Vec::with_capacity(total);
    for _ in 0..passes {
        runs.push(test_run("test::example", TestOutcome::Passed));
    }
    for _ in 0..failures {
        runs.push(test_run("test::example", TestOutcome::Failed));
    }
    runs
}

#[must_use]
pub fn test_score(name: &str, probability: f64) -> FlakinessScore {
    FlakinessScore {
        test_name: TestName::from(name),
        probability_flaky: probability,
        confidence: 0.95,
        pass_rate: 1.0 - probability,
        fail_rate: probability,
        total_runs: 100,
        consecutive_failures: 0,
        last_updated: Utc::now(),
        bayesian_params: BayesianParams {
            alpha: 2.0,
            beta: 98.0,
            posterior_mean: probability,
            posterior_variance: 0.001,
            credible_interval_lower: 0.01,
            credible_interval_upper: 0.05,
        },
    }
}

#[must_use]
pub fn test_session(commit_hash: &str, branch: &str) -> RunSession {
    ActiveSession::start(commit_hash, branch).to_run_session()
}
