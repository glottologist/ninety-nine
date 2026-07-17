use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::analysis::classify;
use crate::detector::BayesianDetector;
use crate::env::detect_environment;
use crate::error::NinetyNineError;
use crate::runner::executor::ExecutionConfig;
use crate::runner::listing::TestCase;
use crate::runner::record::{SystemRrEnvironment, attempt_record};
use crate::runner::stress::{apply_stress_iteration, run_stress_iteration};
use crate::runner::{RunnerBackend, execute_iterations};
use crate::storage::{Storage, StorageBackend};
use crate::types::{
    ActiveSession, DiagnosticResult, FlakeClass, PhaseCounts, RecordOutcome, RunPhase, TestId,
    TestOutcome,
};

pub mod quarantine;

/// Resolves multi-phase flag: CLI overrides config.
#[must_use]
pub fn resolve_multi_phase(cli_multi: bool, cli_no_multi: bool, config_multi: bool) -> bool {
    if cli_no_multi {
        false
    } else if cli_multi {
        true
    } else {
        config_multi
    }
}

pub struct DiagnoseOpts {
    pub stress_runs: u32,
    pub isolation_runs: u32,
    pub stress_threads: usize,
    pub stress_timeout: Duration,
    pub isolation_timeout: Duration,
    pub record: bool,
    pub record_dir: PathBuf,
    pub record_attempts: u32,
    pub chaos: bool,
    pub confidence: f64,
}

/// Isolation execution config for diagnose: serial, no retries.
#[must_use]
pub fn isolation_execution_config(timeout: Duration) -> ExecutionConfig {
    ExecutionConfig {
        concurrency: 1,
        retries: 0,
        timeout,
        retry_delay: Duration::ZERO,
    }
}

/// Builds diagnostic results from stress/isolation count maps (pure).
#[must_use]
pub fn build_results_from_counts(
    counts: &HashMap<TestId, PhaseCounts>,
) -> Vec<(TestId, FlakeClass, PhaseCounts)> {
    let mut out = Vec::new();
    for (id, c) in counts {
        if let Some(class) = classify(c) {
            out.push((id.clone(), class, *c)); // clone: result owns TestId
        }
    }
    out.sort_by(|a, b| a.0.key().cmp(&b.0.key()));
    out
}

/// Runs the multi-phase diagnose pipeline.
///
/// # Errors
///
/// Returns runner or storage errors.
pub async fn run_diagnose(
    _project_dir: &Path,
    _backend: &RunnerBackend,
    storage: &StorageBackend,
    tests: &[TestCase],
    opts: &DiagnoseOpts,
) -> Result<Vec<DiagnosticResult>, NinetyNineError> {
    let (commit_hash, branch) = crate::env::detect_git_info();
    let session = ActiveSession::start_diagnose(&commit_hash, &branch);
    storage.store_session(&session.to_run_session()).await?;

    let mut stress_counts: HashMap<TestId, PhaseCounts> = HashMap::new();

    // Group selected tests by binary path.
    let mut by_binary: HashMap<PathBuf, Vec<TestCase>> = HashMap::new();
    for tc in tests {
        by_binary
            .entry(tc.binary_path.clone()) // clone: HashMap key ownership
            .or_default()
            .push(tc.clone()); // clone: moved into binary groups
    }

    for (binary, cases) in &by_binary {
        println!(
            "Stress: {} ({} selected test(s), {} run(s))",
            binary.display(),
            cases.len(),
            opts.stress_runs
        );
        for _ in 0..opts.stress_runs {
            let iter = run_stress_iteration(binary, opts.stress_threads, opts.stress_timeout)?;
            apply_stress_iteration(cases, &iter, &mut stress_counts);
        }
    }

    let candidates: Vec<&TestCase> = tests
        .iter()
        .filter(|tc| {
            let id = TestId::new(
                tc.package_name.as_str(),
                tc.binary_name.as_str(),
                tc.name.clone(), // clone: temporary id for lookup
            );
            stress_counts
                .get(&id)
                .is_some_and(|c| c.stress_failures > 0)
        })
        .collect();

    println!(
        "Isolation: {} candidate(s), {} run(s) each (serial)",
        candidates.len(),
        opts.isolation_runs
    );

    let iso_cfg = isolation_execution_config(opts.isolation_timeout);
    let environment = detect_environment();
    let detector = BayesianDetector::new(opts.confidence);
    let mut results = Vec::new();

    for tc in candidates {
        let id = TestId::new(
            tc.package_name.as_str(),
            tc.binary_name.as_str(),
            tc.name.clone(), // clone: result identity
        );
        let mut counts = stress_counts.get(&id).copied().unwrap_or(PhaseCounts {
            stress_runs: 0,
            stress_failures: 0,
            isolation_runs: 0,
            isolation_failures: 0,
        });

        let mut runs = execute_iterations(tc, opts.isolation_runs, &iso_cfg, &environment)?;
        for run in &mut runs {
            run.phase = Some(RunPhase::Isolation);
            storage.store_test_run(run, session.id()).await?;
        }

        counts.isolation_runs = u32::try_from(runs.len()).unwrap_or(u32::MAX);
        counts.isolation_failures = u32::try_from(
            runs.iter()
                .filter(|r| {
                    matches!(
                        r.outcome,
                        TestOutcome::Failed | TestOutcome::Panic | TestOutcome::Timeout
                    )
                })
                .count(),
        )
        .unwrap_or(u32::MAX);

        let score = detector.calculate_flakiness_score(tc.name.as_ref(), &runs);
        storage.store_flakiness_score(&score).await?;

        let Some(class) = classify(&counts) else {
            continue;
        };

        let recording = if opts.record && class == FlakeClass::Intrinsic {
            attempt_record(
                tc,
                &opts.record_dir,
                opts.isolation_timeout,
                opts.record_attempts,
                opts.chaos,
                &SystemRrEnvironment,
            )?
        } else {
            RecordOutcome::SkippedNoRequest
        };

        let result = DiagnosticResult {
            test_id: id,
            class,
            counts,
            recording,
        };
        storage
            .store_diagnostic_result(&result, session.id())
            .await?;
        results.push(result);
    }

    // Also classify stress-failed tests that somehow skipped isolation? handled by continue.

    // Mark tests that only failed stress but we ran isolation for all candidates above.

    let classified: HashSet<String> = results.iter().map(|r| r.test_id.key()).collect();
    let _ = classified;

    storage
        .finish_session(
            session.id(),
            u32::try_from(tests.len()).unwrap_or(u32::MAX),
            u32::try_from(results.len()).unwrap_or(u32::MAX),
        )
        .await?;

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TestName;
    use std::time::Duration;

    #[test]
    fn resolve_multi_phase_cli_over_config() {
        assert!(resolve_multi_phase(true, false, false));
        assert!(!resolve_multi_phase(false, true, true));
        assert!(resolve_multi_phase(false, false, true));
        assert!(!resolve_multi_phase(false, false, false));
    }

    #[test]
    fn isolation_config_disables_retries() {
        let cfg = isolation_execution_config(Duration::from_secs(10));
        assert_eq!(cfg.retries, 0);
        assert_eq!(cfg.concurrency, 1);
    }

    #[test]
    fn build_results_classifies_from_counts_map() {
        let mut map = HashMap::new();
        map.insert(
            TestId::new("p", "b", TestName::from("t::contention")),
            PhaseCounts {
                stress_runs: 3,
                stress_failures: 1,
                isolation_runs: 10,
                isolation_failures: 0,
            },
        );
        map.insert(
            TestId::new("p", "b", TestName::from("t::clean")),
            PhaseCounts {
                stress_runs: 3,
                stress_failures: 0,
                isolation_runs: 0,
                isolation_failures: 0,
            },
        );
        map.insert(
            TestId::new("p", "b", TestName::from("t::broken")),
            PhaseCounts {
                stress_runs: 2,
                stress_failures: 2,
                isolation_runs: 5,
                isolation_failures: 5,
            },
        );

        let results = build_results_from_counts(&map);
        assert_eq!(results.len(), 2);
        let classes: HashSet<_> = results.iter().map(|(_, c, _)| *c).collect();
        assert!(classes.contains(&FlakeClass::Contention));
        assert!(classes.contains(&FlakeClass::Broken));
    }
}
