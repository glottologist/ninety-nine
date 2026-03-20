use std::path::PathBuf;

use crate::error::NinetyNineError;
use crate::runner::binary::{BinaryKind, TestBinary};
use crate::types::TestName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestKind {
    Test,
    Benchmark,
}

#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: TestName,
    pub binary_path: PathBuf,
    pub binary_name: String,
    pub package_name: String,
    pub binary_kind: BinaryKind,
    pub kind: TestKind,
}

/// Lists all tests in the given binary by running it with `--list --format terse`.
///
/// # Errors
///
/// Returns `TestListing` if the binary cannot be executed or exits with failure.
pub fn list_tests(binary: &TestBinary) -> Result<Vec<TestCase>, NinetyNineError> {
    let output = std::process::Command::new(&binary.path)
        .args(["--list", "--format", "terse"])
        .output()
        .map_err(|e| NinetyNineError::TestListing {
            binary: binary.path.clone(), // clone: PathBuf needed for error context
            message: format!("failed to execute binary: {e}"),
        })?;

    if !output.status.success() {
        return Err(NinetyNineError::TestListing {
            binary: binary.path.clone(), // clone: PathBuf needed for error context
            message: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_test_listing(&stdout, binary))
}

fn parse_test_listing(output: &str, binary: &TestBinary) -> Vec<TestCase> {
    let mut cases = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some((name, kind)) = parse_listing_line(trimmed) {
            cases.push(TestCase {
                name: TestName::from(name),
                binary_path: binary.path.clone(), // clone: each TestCase owns its path for independent execution
                binary_name: binary.binary_name.clone(), // clone: each TestCase owns its binary name
                package_name: binary.package_name.clone(), // clone: each TestCase owns its package name
                binary_kind: binary.kind,
                kind,
            });
        }
    }

    cases
}

fn parse_listing_line(line: &str) -> Option<(String, TestKind)> {
    if let Some(name) = line.strip_suffix(": test") {
        if !name.is_empty() {
            return Some((name.to_string(), TestKind::Test));
        }
    }

    if let Some(name) = line.strip_suffix(": benchmark") {
        if !name.is_empty() {
            return Some((name.to_string(), TestKind::Benchmark));
        }
    }

    None
}

/// Lists tests from multiple binaries in parallel, limited by concurrency.
///
/// # Errors
///
/// Returns `TestListing` if any binary fails to list its tests, or if a
/// spawned task panics.
pub async fn list_tests_parallel(
    binaries: &[TestBinary],
    concurrency: usize,
) -> Result<Vec<TestCase>, NinetyNineError> {
    use tokio::sync::Semaphore;

    let semaphore = std::sync::Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::with_capacity(binaries.len());

    for binary in binaries {
        let sem = std::sync::Arc::clone(&semaphore); // clone: Arc shared across tasks
        let binary = binary.clone(); // clone: moved into spawned task

        let handle = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| NinetyNineError::TestListing {
                    binary: binary.path.clone(), // clone: PathBuf needed for error context
                    message: format!("semaphore error: {e}"),
                })?;

            tokio::task::spawn_blocking(move || list_tests(&binary))
                .await
                .map_err(|e| NinetyNineError::TestListing {
                    binary: PathBuf::new(),
                    message: format!("task join error: {e}"),
                })?
        });

        handles.push(handle);
    }

    let mut all_cases = Vec::new();
    for handle in handles {
        let result = handle.await.map_err(|e| NinetyNineError::TestListing {
            binary: PathBuf::new(),
            message: format!("task join error: {e}"),
        })?;

        match result {
            Ok(cases) => all_cases.extend(cases),
            Err(NinetyNineError::TestListing { binary, message }) => {
                tracing::warn!(
                    binary = %binary.display(),
                    "skipping binary that failed to list tests: {message}"
                );
            }
            Err(e) => return Err(e),
        }
    }

    Ok(all_cases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case("tests::test_one: test", Some(("tests::test_one".to_string(), TestKind::Test)))]
    #[case("benches::bench_one: benchmark", Some(("benches::bench_one".to_string(), TestKind::Benchmark)))]
    #[case("not_a_test_line", None)]
    #[case("", None)]
    #[case(": test", None)]
    #[case(": benchmark", None)]
    #[case("3 tests, 0 benchmarks", None)]
    fn parses_listing_lines(#[case] input: &str, #[case] expected: Option<(String, TestKind)>) {
        assert_eq!(parse_listing_line(input), expected);
    }

    proptest! {
        #[test]
        fn parse_listing_line_never_panics(input in ".*") {
            let _ = parse_listing_line(&input);
        }

        #[test]
        fn parse_test_listing_never_panics(input in ".*") {
            let binary = TestBinary {
                path: PathBuf::from("/fake/binary"),
                package_name: "fake".to_string(),
                binary_name: "fake".to_string(),
                kind: crate::runner::binary::BinaryKind::Test,
            };
            let _ = parse_test_listing(&input, &binary);
        }

        #[test]
        fn valid_test_lines_always_parsed(name in "[a-z_:]{1,50}") {
            let line = format!("{name}: test");
            let result = parse_listing_line(&line);
            prop_assert!(result.is_some());
            let (parsed_name, kind) = result.unwrap();
            prop_assert_eq!(parsed_name, name);
            prop_assert_eq!(kind, TestKind::Test);
        }
    }
}
