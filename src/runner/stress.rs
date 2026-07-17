use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::time::Duration;

use crate::error::NinetyNineError;
use crate::runner::listing::TestCase;
use crate::runner::process::{CommandSpec, run_timed};
use crate::types::{PhaseCounts, TestId};

#[derive(Debug, Clone, Default)]
pub struct StressIteration {
    pub failure_names: BTreeSet<String>,
    pub timed_out: bool,
    pub inconclusive: bool,
}

/// Builds the command for a full-binary multi-threaded stress run (no `--exact`).
#[must_use]
pub fn stress_command(binary: &Path, threads: usize) -> CommandSpec {
    CommandSpec {
        program: binary.to_path_buf(),
        args: vec!["--test-threads".into(), threads.to_string()],
        cwd: None,
    }
}

/// Parses libtest failure names from suite output.
///
/// Prefers the trailing `failures:` list section; falls back to lines of the
/// form `test <name> ... FAILED`.
#[must_use]
pub fn parse_libtest_failure_names(stdout: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    if let Some(list) = extract_failures_list_section(stdout) {
        for line in list.lines() {
            let name = line.trim();
            if !name.is_empty() && !name.starts_with("failures:") {
                names.insert(name.to_string());
            }
        }
        if !names.is_empty() {
            return names;
        }
    }

    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("test ") {
            if rest.contains(" ... FAILED") || rest.ends_with("... FAILED") {
                if let Some(name) = rest.split(" ... ").next() {
                    let name = name.trim();
                    if !name.is_empty() {
                        names.insert(name.to_string());
                    }
                }
            }
        }
    }

    names
}

fn extract_failures_list_section(stdout: &str) -> Option<&str> {
    // Prefer the compact trailing list:
    //   failures:
    //       foo::bar
    //   test result: ...
    let marker = "\nfailures:\n";
    let mut search = stdout;
    let mut last = None;
    while let Some(idx) = search.find(marker) {
        let after = &search[idx + marker.len()..];
        if let Some(end) = after.find("\ntest result:") {
            last = Some(&after[..end]);
        } else if let Some(end) = after.find("\n\n") {
            last = Some(&after[..end]);
        } else {
            last = Some(after);
        }
        search = &search[idx + 1..];
    }
    last
}

/// Runs one full-binary stress iteration.
///
/// # Errors
///
/// Returns `RunnerExecution` if the binary cannot be spawned.
pub fn run_stress_iteration(
    binary: &Path,
    threads: usize,
    timeout: Duration,
) -> Result<StressIteration, NinetyNineError> {
    let spec = stress_command(binary, threads);
    let outcome = run_timed(&spec, timeout)?;

    if outcome.timed_out {
        return Ok(StressIteration {
            failure_names: BTreeSet::new(),
            timed_out: true,
            inconclusive: true,
        });
    }

    let combined = format!("{}{}", outcome.stdout, outcome.stderr);
    let failure_names = parse_libtest_failure_names(&combined);

    // Nonzero exit with no parseable failures is inconclusive for per-test attribution.
    let inconclusive = !outcome.status.success() && failure_names.is_empty();

    Ok(StressIteration {
        failure_names,
        timed_out: false,
        inconclusive,
    })
}

/// Applies one stress iteration to selected tests on a binary.
///
/// Always increments `stress_runs` for each selected test. Increments
/// `stress_failures` only when a selected short name appears in the failure set.
/// Timeout / inconclusive iterations never fabricate named failures.
pub fn apply_stress_iteration(
    selected_on_binary: &[TestCase],
    iter: &StressIteration,
    acc: &mut HashMap<TestId, PhaseCounts>,
) {
    for tc in selected_on_binary {
        let id = TestId::new(
            tc.package_name.as_str(),
            tc.binary_name.as_str(),
            tc.name.clone(), // clone: TestId owns TestName
        );
        let entry = acc.entry(id).or_insert(PhaseCounts {
            stress_runs: 0,
            stress_failures: 0,
            isolation_runs: 0,
            isolation_failures: 0,
        });
        entry.stress_runs = entry.stress_runs.saturating_add(1);
        if !iter.inconclusive && iter.failure_names.contains(tc.name.as_ref()) {
            entry.stress_failures = entry.stress_failures.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::binary::BinaryKind;
    use crate::runner::listing::TestKind;
    use crate::types::TestName;
    use std::path::PathBuf;

    fn case(package: &str, binary: &str, name: &str) -> TestCase {
        TestCase {
            name: TestName::from(name),
            binary_path: PathBuf::from(format!("/tmp/{binary}")),
            binary_name: binary.to_string(),
            package_name: package.to_string(),
            binary_kind: BinaryKind::Test,
            kind: TestKind::Test,
        }
    }

    #[test]
    fn parse_libtest_failures_extracts_names() {
        let stdout = "\
running 2 tests
test foo::bar ... FAILED
test foo::ok ... ok

failures:

---- foo::bar stdout ----
assertion failed

failures:
    foo::bar

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
";
        let names = parse_libtest_failure_names(stdout);
        assert_eq!(names, BTreeSet::from(["foo::bar".to_string()]));
    }

    #[test]
    fn parse_empty_output_no_failures() {
        assert!(parse_libtest_failure_names("").is_empty());
    }

    #[test]
    fn stress_command_has_threads_not_exact() {
        let c = stress_command(Path::new("/bin/t"), 4);
        assert!(c.args.iter().any(|a| a == "--test-threads"));
        assert!(c.args.iter().any(|a| a == "4"));
        assert!(!c.args.iter().any(|a| a == "--exact"));
    }

    #[test]
    fn apply_stress_ignores_non_selected_failure_names() {
        let selected = vec![case("pkg", "bin", "tests::a")];
        let iter = StressIteration {
            failure_names: BTreeSet::from(["tests::b".to_string()]),
            timed_out: false,
            inconclusive: false,
        };
        let mut acc = HashMap::new();
        apply_stress_iteration(&selected, &iter, &mut acc);
        let id = TestId::new("pkg", "bin", "tests::a");
        let counts = acc.get(&id).unwrap();
        assert_eq!(counts.stress_runs, 1);
        assert_eq!(counts.stress_failures, 0);
    }

    #[test]
    fn apply_stress_two_binaries_same_short_name_are_distinct() {
        let a = case("pkg1", "bin1", "t");
        let b = case("pkg2", "bin2", "t");
        let iter = StressIteration {
            failure_names: BTreeSet::from(["t".to_string()]),
            timed_out: false,
            inconclusive: false,
        };
        let mut acc = HashMap::new();
        apply_stress_iteration(std::slice::from_ref(&a), &iter, &mut acc);
        apply_stress_iteration(std::slice::from_ref(&b), &iter, &mut acc);
        assert_eq!(acc.len(), 2);
        assert_eq!(
            acc.get(&TestId::new("pkg1", "bin1", "t"))
                .unwrap()
                .stress_failures,
            1
        );
        assert_eq!(
            acc.get(&TestId::new("pkg2", "bin2", "t"))
                .unwrap()
                .stress_failures,
            1
        );
    }

    #[test]
    fn apply_stress_timeout_inconclusive_no_fabricated_failures() {
        let selected = vec![case("pkg", "bin", "tests::a")];
        let iter = StressIteration {
            failure_names: BTreeSet::new(),
            timed_out: true,
            inconclusive: true,
        };
        let mut acc = HashMap::new();
        apply_stress_iteration(&selected, &iter, &mut acc);
        let counts = acc.get(&TestId::new("pkg", "bin", "tests::a")).unwrap();
        assert_eq!(counts.stress_runs, 1);
        assert_eq!(counts.stress_failures, 0);
    }
}
