pub mod binary;
pub mod detection;
pub mod executor;
pub mod listing;

pub use detection::{AvailableRunner, detect_available_runner};

use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use crate::error::NinetyNineError;
use crate::runner::binary::discover_test_binaries;
use crate::runner::executor::{ExecutionConfig, Executor, TestResult};
use crate::runner::listing::{TestCase, TestKind, list_tests_parallel};
use crate::types::{TestEnvironment, TestOutcome, TestRun};

pub struct NativeRunner {
    project_root: PathBuf,
    execution_config: ExecutionConfig,
}

impl NativeRunner {
    #[must_use]
    pub fn new(project_root: &Path, config: ExecutionConfig) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            execution_config: config,
        }
    }

    /// Discovers test cases matching the filter.
    ///
    /// # Errors
    ///
    /// Returns `BinaryDiscovery` or `TestListing` errors.
    pub async fn discover_tests(&self, filter: &str) -> Result<Vec<TestCase>, NinetyNineError> {
        let project_root = self.project_root.clone(); // clone: moved into blocking task
        let binaries = tokio::task::spawn_blocking(move || discover_test_binaries(&project_root))
            .await
            .map_err(|e| NinetyNineError::BinaryDiscovery {
                message: format!("task join error: {e}"),
            })??;

        if binaries.is_empty() {
            return Ok(Vec::new());
        }

        let concurrency = self.execution_config.concurrency;
        let mut cases = list_tests_parallel(&binaries, concurrency).await?;

        cases.retain(|tc| tc.kind == TestKind::Test);

        if !filter.is_empty() {
            cases.retain(|tc| tc.name.contains(filter));
        }

        Ok(cases)
    }

    /// Runs a single test case.
    ///
    /// # Errors
    ///
    /// Returns `RunnerExecution` if the test fails to execute.
    pub fn run_test_sync(&self, test_case: &TestCase) -> Result<TestResult, NinetyNineError> {
        let executor = Executor::new(&self.execution_config);
        executor.run_single(test_case)
    }

    /// Runs a test case multiple times.
    ///
    /// # Errors
    ///
    /// Returns `RunnerExecution` if any iteration fails to execute.
    pub fn run_test_repeatedly(
        &self,
        test_case: &TestCase,
        iterations: u32,
        environment: &TestEnvironment,
    ) -> Result<Vec<TestRun>, NinetyNineError> {
        let mut runs = Vec::with_capacity(usize::try_from(iterations).unwrap_or(usize::MAX));

        for _ in 0..iterations {
            let result = self.run_test_sync(test_case)?;
            runs.push(test_result_to_run(&result, environment));
        }

        Ok(runs)
    }
}

pub enum RunnerBackend {
    Native(NativeRunner),
}

impl RunnerBackend {
    #[must_use]
    pub fn native(project_root: &Path, config: ExecutionConfig) -> Self {
        Self::Native(NativeRunner::new(project_root, config))
    }

    #[must_use]
    pub const fn execution_config(&self) -> &ExecutionConfig {
        match self {
            Self::Native(r) => &r.execution_config,
        }
    }

    /// Discovers test cases matching the filter.
    ///
    /// # Errors
    ///
    /// Returns errors from the underlying runner.
    pub async fn discover_tests(&self, filter: &str) -> Result<Vec<TestCase>, NinetyNineError> {
        match self {
            Self::Native(r) => r.discover_tests(filter).await,
        }
    }

    /// Runs a test case multiple times.
    ///
    /// # Errors
    ///
    /// Returns errors from the underlying runner.
    pub fn run_test_repeatedly(
        &self,
        test_case: &TestCase,
        iterations: u32,
        environment: &TestEnvironment,
    ) -> Result<Vec<TestRun>, NinetyNineError> {
        match self {
            Self::Native(r) => r.run_test_repeatedly(test_case, iterations, environment),
        }
    }
}

/// Executes a test for `iterations` iterations and returns the results.
///
/// # Errors
///
/// Returns `RunnerExecution` if any iteration fails to execute.
pub fn execute_iterations(
    test_case: &TestCase,
    iterations: u32,
    config: &ExecutionConfig,
    environment: &TestEnvironment,
) -> Result<Vec<TestRun>, NinetyNineError> {
    let executor = Executor::new(config);
    let mut runs = Vec::with_capacity(usize::try_from(iterations).unwrap_or(usize::MAX));

    for _ in 0..iterations {
        let result = executor.run_single(test_case)?;
        runs.push(test_result_to_run(&result, environment));
    }

    Ok(runs)
}

fn test_result_to_run(result: &TestResult, environment: &TestEnvironment) -> TestRun {
    let error_message = match result.outcome {
        TestOutcome::Passed | TestOutcome::Ignored => None,
        TestOutcome::Timeout => Some(format!("test timed out after {:?}", result.duration)),
        _ => {
            if result.stderr.is_empty() {
                if result.stdout.is_empty() {
                    None
                } else {
                    Some(result.stdout.clone()) // clone: extracting owned string from result
                }
            } else {
                Some(result.stderr.clone()) // clone: extracting owned string from result
            }
        }
    };

    TestRun {
        id: Uuid::new_v4(),
        test_name: result.test_case.name.clone(), // clone: TestRun owns its name independently
        test_path: result.test_case.binary_path.clone(), // clone: TestRun owns its path independently
        outcome: result.outcome,
        duration: result.duration,
        timestamp: Utc::now(),
        commit_hash: String::new(),
        branch: String::new(),
        environment: environment.clone(), // clone: TestEnvironment reused across multiple test runs
        retry_count: result.attempt.saturating_sub(1),
        error_message,
        stack_trace: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::binary::BinaryKind;
    use crate::runner::listing::TestKind;
    use crate::test_helpers::test_environment;
    use crate::types::TestName;
    use rstest::rstest;
    use std::time::Duration;

    fn mock_result(outcome: TestOutcome, stdout: &str, stderr: &str) -> TestResult {
        TestResult {
            test_case: TestCase {
                name: TestName::from("tests::example"),
                binary_path: std::path::PathBuf::from("/tmp/bin"),
                binary_name: "bin".to_string(),
                package_name: "pkg".to_string(),
                binary_kind: BinaryKind::Test,
                kind: TestKind::Test,
            },
            outcome,
            duration: Duration::from_millis(42),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            attempt: 1,
        }
    }

    #[rstest]
    #[case(TestOutcome::Passed, "", "", None)]
    #[case(TestOutcome::Ignored, "", "", None)]
    #[case(TestOutcome::Failed, "", "assertion failed", Some("assertion failed"))]
    #[case(TestOutcome::Failed, "stdout msg", "", Some("stdout msg"))]
    #[case(TestOutcome::Failed, "", "", None)]
    #[case(TestOutcome::Panic, "", "panicked at", Some("panicked at"))]
    fn test_result_to_run_extracts_error_message(
        #[case] outcome: TestOutcome,
        #[case] stdout: &str,
        #[case] stderr: &str,
        #[case] expected_msg: Option<&str>,
    ) {
        let result = mock_result(outcome, stdout, stderr);
        let env = test_environment();
        let run = test_result_to_run(&result, &env);
        assert_eq!(run.outcome, outcome);
        assert_eq!(run.error_message.as_deref(), expected_msg);
    }

    #[test]
    fn test_result_to_run_timeout_message() {
        let result = mock_result(TestOutcome::Timeout, "", "");
        let env = test_environment();
        let run = test_result_to_run(&result, &env);
        assert!(run.error_message.as_ref().unwrap().contains("timed out"));
    }
}
