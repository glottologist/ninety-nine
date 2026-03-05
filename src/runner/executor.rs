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
                .map(|n| n.get())
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

pub struct Executor {
    config: ExecutionConfig,
}

impl Executor {
    pub fn new(config: ExecutionConfig) -> Self {
        Self { config }
    }

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
        &test_case.name,
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
                    handle.kill().ok();
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
