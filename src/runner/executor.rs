use std::time::Duration;

use crate::error::NinetyNineError;
use crate::runner::listing::TestCase;
use crate::runner::process::{CommandSpec, run_timed};
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

    /// Runs a single test case, retrying failures according to the execution
    /// config. Every attempt is returned, oldest first, so that fail-then-pass
    /// sequences remain visible to flakiness detection rather than being
    /// collapsed into their final outcome.
    ///
    /// # Errors
    ///
    /// Returns `RunnerExecution` if the test binary cannot be spawned.
    pub fn run_attempts(&self, test_case: &TestCase) -> Result<Vec<TestResult>, NinetyNineError> {
        collect_attempts(
            || execute_test(test_case, self.config.timeout),
            self.config.retries,
            self.config.retry_delay,
        )
    }
}

fn collect_attempts(
    mut run_once: impl FnMut() -> Result<TestResult, NinetyNineError>,
    retries: u32,
    retry_delay: Duration,
) -> Result<Vec<TestResult>, NinetyNineError> {
    let mut attempts = Vec::with_capacity(1);

    for attempt in 0..=retries {
        if attempt > 0 {
            std::thread::sleep(retry_delay);
        }

        let result = run_once()?;
        let retryable = matches!(
            result.outcome,
            TestOutcome::Failed | TestOutcome::Panic | TestOutcome::Timeout
        );

        attempts.push(TestResult {
            attempt: attempt + 1,
            ..result
        });

        if !retryable {
            break;
        }
    }

    Ok(attempts)
}

fn execute_test(test_case: &TestCase, timeout: Duration) -> Result<TestResult, NinetyNineError> {
    let spec = CommandSpec {
        program: test_case.binary_path.clone(), // clone: CommandSpec owns the path
        args: vec![
            "--exact".into(),
            test_case.name.as_ref().to_owned(),
            "--nocapture".into(),
        ],
        cwd: None,
    };

    let outcome = run_timed(&spec, timeout)?;
    Ok(build_result(test_case, &outcome))
}

fn build_result(
    test_case: &TestCase,
    outcome: &crate::runner::process::ProcessOutcome,
) -> TestResult {
    if outcome.timed_out {
        return TestResult {
            test_case: test_case.clone(), // clone: result takes ownership for independent use
            outcome: TestOutcome::Timeout,
            duration: outcome.duration,
            stdout: outcome.stdout.clone(), // clone: TestResult owns captured output
            stderr: outcome.stderr.clone(), // clone: TestResult owns captured output
            attempt: 1,
        };
    }

    let classified = if outcome.status.success() {
        // An `#[ignore]` test selected via --exact exits successfully without
        // running; libtest's result line is the only signal that nothing ran.
        if outcome.stdout.contains("test result: ok. 0 passed;") {
            TestOutcome::Ignored
        } else {
            TestOutcome::Passed
        }
    } else if outcome.stderr.contains("panicked at") || outcome.stdout.contains("panicked at") {
        TestOutcome::Panic
    } else {
        TestOutcome::Failed
    };

    TestResult {
        test_case: test_case.clone(), // clone: result takes ownership for independent use
        outcome: classified,
        duration: outcome.duration,
        stdout: outcome.stdout.clone(), // clone: TestResult owns captured output
        stderr: outcome.stderr.clone(), // clone: TestResult owns captured output
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
    use std::process::ExitStatus;

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

    fn mock_process(
        status_code: i32,
        stdout: &str,
        stderr: &str,
        timed_out: bool,
    ) -> crate::runner::process::ProcessOutcome {
        crate::runner::process::ProcessOutcome {
            status: ExitStatus::from_raw(status_code << 8),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            duration: Duration::from_millis(10),
            timed_out,
        }
    }

    #[rstest]
    #[case(0, "", "", TestOutcome::Passed)]
    #[case(
        0,
        "running 1 test\ntest tests::example ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s\n",
        "",
        TestOutcome::Passed
    )]
    #[case(
        0,
        "running 1 test\ntest tests::example ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
        "",
        TestOutcome::Ignored
    )]
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
        let output = mock_process(exit_code, stdout, stderr, false);
        let result = build_result(&tc, &output);
        assert_eq!(result.outcome, expected);
    }

    #[test]
    fn build_result_timeout() {
        let tc = mock_test_case();
        let output = mock_process(1, "", "", true);
        let result = build_result(&tc, &output);
        assert_eq!(result.outcome, TestOutcome::Timeout);
    }

    fn scripted_result(outcome: TestOutcome) -> TestResult {
        TestResult {
            test_case: mock_test_case(),
            outcome,
            duration: Duration::from_millis(1),
            stdout: String::new(),
            stderr: String::new(),
            attempt: 1,
        }
    }

    fn scripted_runner(
        outcomes: Vec<TestOutcome>,
    ) -> impl FnMut() -> Result<TestResult, NinetyNineError> {
        let mut queue = outcomes.into_iter();
        move || {
            Ok(scripted_result(
                queue
                    .next()
                    .expect("script exhausted: loop ran too many attempts"),
            ))
        }
    }

    #[rstest]
    #[case(vec![TestOutcome::Passed], 2, vec![TestOutcome::Passed])]
    #[case(
        vec![TestOutcome::Failed, TestOutcome::Failed, TestOutcome::Passed],
        2,
        vec![TestOutcome::Failed, TestOutcome::Failed, TestOutcome::Passed]
    )]
    #[case(
        vec![TestOutcome::Failed, TestOutcome::Passed],
        2,
        vec![TestOutcome::Failed, TestOutcome::Passed]
    )]
    #[case(
        vec![TestOutcome::Failed, TestOutcome::Failed],
        1,
        vec![TestOutcome::Failed, TestOutcome::Failed]
    )]
    #[case(vec![TestOutcome::Ignored], 2, vec![TestOutcome::Ignored])]
    fn collect_attempts_records_every_attempt(
        #[case] script: Vec<TestOutcome>,
        #[case] retries: u32,
        #[case] expected: Vec<TestOutcome>,
    ) {
        let attempts = collect_attempts(scripted_runner(script), retries, Duration::ZERO).unwrap();
        let outcomes: Vec<TestOutcome> = attempts.iter().map(|a| a.outcome).collect();
        assert_eq!(outcomes, expected);
        let numbers: Vec<u32> = attempts.iter().map(|a| a.attempt).collect();
        let expected_numbers: Vec<u32> = (1..=u32::try_from(expected.len()).unwrap()).collect();
        assert_eq!(numbers, expected_numbers);
    }

    #[test]
    fn collect_attempts_propagates_errors() {
        let result = collect_attempts(
            || {
                Err(NinetyNineError::RunnerExecution {
                    message: "spawn failed".to_string(),
                })
            },
            2,
            Duration::ZERO,
        );
        assert!(result.is_err());
    }
}
