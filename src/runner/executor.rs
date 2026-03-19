use std::time::{Duration, Instant};

use crate::error::NinetyNineError;
use crate::runner::listing::TestCase;
use crate::types::TestOutcome;

#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub concurrency: usize,
    pub timeout: Duration,
    pub retries: u32,
    pub retry_delay: Duration,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            concurrency: std::thread::available_parallelism()
                .map(std::num::NonZeroUsize::get)
                .unwrap_or(4),
            timeout: Duration::from_secs(300),
            retries: 0,
            retry_delay: Duration::from_millis(100),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub test_case: TestCase,
    pub outcome: TestOutcome,
    pub duration: Duration,
    pub stdout: String,
    pub stderr: String,
    pub attempt: u32,
}

pub struct Executor<'a> {
    config: &'a ExecutionConfig,
}

impl<'a> Executor<'a> {
    #[must_use]
    pub const fn new(config: &'a ExecutionConfig) -> Self {
        Self { config }
    }

    /// Runs a single test case, retrying according to the execution config.
    ///
    /// # Errors
    ///
    /// Returns `RunnerExecution` if the test binary cannot be spawned or
    /// if no execution attempts are made.
    pub fn run_single(&self, test_case: &TestCase) -> Result<TestResult, NinetyNineError> {
        let mut last_result = None;

        for attempt in 0..=self.config.retries {
            if attempt > 0 {
                std::thread::sleep(self.config.retry_delay);
            }

            let result = execute_test(test_case, self.config.timeout)?;
            let passed = result.outcome == TestOutcome::Passed;

            last_result = Some(TestResult {
                attempt: attempt + 1,
                ..result
            });

            if passed {
                break;
            }
        }

        last_result.ok_or_else(|| NinetyNineError::RunnerExecution {
            message: "no execution attempts made".to_string(),
        })
    }
}

fn execute_test(test_case: &TestCase, timeout: Duration) -> Result<TestResult, NinetyNineError> {
    let start = Instant::now();

    let expression = duct::cmd!(
        &test_case.binary_path,
        "--exact",
        test_case.name.as_ref(),
        "--nocapture"
    )
    .unchecked()
    .stdout_capture()
    .stderr_capture();

    let handle = expression
        .start()
        .map_err(|e| NinetyNineError::RunnerExecution {
            message: format!(
                "failed to spawn test binary {}: {e}",
                test_case.binary_path.display()
            ),
        })?;

    let deadline = Instant::now() + timeout;
    let poll_interval = Duration::from_millis(50);

    loop {
        match handle.try_wait() {
            Ok(Some(output)) => {
                let duration = start.elapsed();
                return Ok(build_result(test_case, output, duration));
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    if let Err(e) = handle.kill() {
                        tracing::warn!("failed to kill test process: {e}");
                    }
                    let duration = start.elapsed();
                    return Ok(TestResult {
                        test_case: test_case.clone(), // clone: result takes ownership for independent use
                        outcome: TestOutcome::Timeout,
                        duration,
                        stdout: String::new(),
                        stderr: String::new(),
                        attempt: 1,
                    });
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                return Err(NinetyNineError::RunnerExecution {
                    message: format!("error waiting for test: {e}"),
                });
            }
        }
    }
}

fn build_result(
    test_case: &TestCase,
    output: &std::process::Output,
    duration: Duration,
) -> TestResult {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let outcome = if output.status.success() {
        TestOutcome::Passed
    } else if stderr.contains("panicked at") || stdout.contains("panicked at") {
        TestOutcome::Panic
    } else {
        TestOutcome::Failed
    };

    TestResult {
        test_case: test_case.clone(), // clone: result takes ownership for independent use
        outcome,
        duration,
        stdout,
        stderr,
        attempt: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::binary::BinaryKind;
    use crate::runner::listing::TestKind;
    use crate::types::TestName;
    use rstest::rstest;
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::{ExitStatus, Output};

    fn mock_test_case() -> TestCase {
        TestCase {
            name: TestName::from("tests::example"),
            binary_path: PathBuf::from("/tmp/test-bin"),
            binary_name: "test-bin".to_string(),
            package_name: "my-crate".to_string(),
            binary_kind: BinaryKind::Test,
            kind: TestKind::Test,
        }
    }

    fn mock_output(status_code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(status_code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[rstest]
    #[case(0, "", "", TestOutcome::Passed)]
    #[case(1, "", "", TestOutcome::Failed)]
    #[case(1, "", "thread 'main' panicked at 'assertion'", TestOutcome::Panic)]
    #[case(1, "thread 'main' panicked at 'boom'", "", TestOutcome::Panic)]
    fn build_result_classifies_outcome(
        #[case] exit_code: i32,
        #[case] stdout: &str,
        #[case] stderr: &str,
        #[case] expected: TestOutcome,
    ) {
        let tc = mock_test_case();
        let output = mock_output(exit_code, stdout, stderr);
        let result = build_result(&tc, &output, Duration::from_millis(10));
        assert_eq!(result.outcome, expected);
    }
}
