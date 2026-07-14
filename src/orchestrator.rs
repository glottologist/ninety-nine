use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::analysis::duration::{
    DurationRegression, RegressionThreshold, detect_duration_regressions,
};
use crate::analysis::pattern::detect_patterns;
use crate::analysis::trend::calculate_trend;
use crate::cli::output::{format_test_result_line, print_duration_warning, print_run_header};
use crate::config::model::{Config, DurationRegressionConfig, DurationThreshold};
use crate::detector::BayesianDetector;
use crate::env::detect_environment;
use crate::error::NinetyNineError;
use crate::runner::listing::TestCase;
use crate::runner::{RunnerBackend, execute_iterations};
use crate::storage::{Storage, StorageBackend};
use crate::types::{ActiveSession, FlakinessScore, OutcomeCounts, TestRun, TestVerdict};

pub struct SuiteResults {
    pub scores: Vec<FlakinessScore>,
    pub all_runs: Vec<TestRun>,
    pub pass_count: usize,
    pub flaky_count: usize,
    pub fail_count: usize,
    pub skip_count: usize,
    pub duration_regressions: Vec<DurationRegression>,
}

/// Runs every test in `tests` for `iterations` iterations, collecting results.
///
/// # Errors
///
/// Returns `RunnerExecution` if task spawning or test execution fails,
/// or a storage error if persisting results fails.
pub async fn execute_test_suite(
    backend: &RunnerBackend,
    storage: &StorageBackend,
    tests: &[TestCase],
    iterations: u32,
    detector: &BayesianDetector,
    session: &ActiveSession,
    duration_cfg: Option<&DurationRegressionConfig>,
) -> Result<SuiteResults, NinetyNineError> {
    let environment = Arc::new(detect_environment());
    let config = Arc::new(backend.execution_config().clone()); // clone: shared across spawn_blocking tasks
    let semaphore = Arc::new(tokio::sync::Semaphore::new(config.concurrency));
    let total = tests.len();

    print_run_header(total, iterations);
    let pb = suite_progress_bar(total);

    let mut join_set = tokio::task::JoinSet::new();
    for test_case in tests {
        let sem = Arc::clone(&semaphore);
        let tc = test_case.clone(); // clone: moved into spawned task
        let cfg = Arc::clone(&config);
        let env = Arc::clone(&environment);

        join_set.spawn(async move {
            let permit =
                sem.acquire_owned()
                    .await
                    .map_err(|e| NinetyNineError::RunnerExecution {
                        message: format!("semaphore closed: {e}"),
                    })?;

            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let start = Instant::now();
                let runs = execute_iterations(&tc, iterations, &cfg, &env)?;
                let elapsed = start.elapsed();
                Ok::<_, NinetyNineError>((tc, runs, elapsed))
            })
            .await
            .map_err(|e| NinetyNineError::RunnerExecution {
                message: format!("task join error: {e}"),
            })?
        });
    }

    let mut scores = Vec::with_capacity(total);
    let mut all_runs = Vec::new();
    let mut pass_count = 0usize;
    let mut flaky_count = 0usize;
    let mut fail_count = 0usize;
    let mut skip_count = 0usize;
    let mut completed = 0usize;
    let mut duration_regressions = Vec::new();

    while let Some(result) = join_set.join_next().await {
        let (test_case, runs, elapsed) =
            result.map_err(|e| NinetyNineError::RunnerExecution {
                message: format!("task join error: {e}"),
            })??;

        completed += 1;

        let counts = OutcomeCounts::from_runs(&runs);
        match counts.verdict() {
            TestVerdict::Passed => pass_count += 1,
            TestVerdict::Flaky => flaky_count += 1,
            TestVerdict::Failed => fail_count += 1,
            TestVerdict::Skipped => skip_count += 1,
        }

        let line = format_test_result_line(
            &test_case.name,
            counts.verdict(),
            counts.passed,
            iterations,
            elapsed,
            completed,
            total,
        );
        pb.println(line);
        pb.set_position(u64::try_from(completed).unwrap_or(u64::MAX));
        pb.set_message(format!("{flaky_count} flaky, {fail_count} failed"));

        for run in &runs {
            storage.store_test_run(run, session.id()).await?;
        }

        let score = detector.calculate_flakiness_score(&test_case.name, &runs);
        storage.store_flakiness_score(&score).await?;

        check_duration_regression(
            storage,
            &test_case.name,
            duration_cfg,
            &mut duration_regressions,
        )
        .await?;

        all_runs.extend(runs);
        scores.push(score);
    }

    pb.finish_and_clear();

    Ok(SuiteResults {
        scores,
        all_runs,
        pass_count,
        flaky_count,
        fail_count,
        skip_count,
        duration_regressions,
    })
}

fn suite_progress_bar(total: usize) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new(u64::try_from(total).unwrap_or(u64::MAX));
    pb.set_style(
        indicatif::ProgressStyle::with_template(
            "{bar:40.cyan/blue} {pos}/{len} | {msg} | {elapsed_precise}",
        )
        .unwrap_or_else(|_| indicatif::ProgressStyle::default_bar())
        .progress_chars("##-"),
    );
    pb.set_message("0 flaky, 0 failed");
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

async fn check_duration_regression(
    storage: &StorageBackend,
    test_name: &str,
    duration_cfg: Option<&DurationRegressionConfig>,
    regressions: &mut Vec<DurationRegression>,
) -> Result<(), NinetyNineError> {
    // Absent config keeps the long-standing default: enabled, 10-run history,
    // 2 standard deviations.
    let (min_history, threshold) = match duration_cfg {
        None => (10, RegressionThreshold::StdDevs(2.0)),
        Some(cfg) if !cfg.enabled => return Ok(()),
        Some(cfg) => (
            usize::try_from(cfg.min_history_runs).unwrap_or(10),
            match cfg.threshold {
                DurationThreshold::Multiplier(m) => RegressionThreshold::Multiplier(m),
                DurationThreshold::StdDev(z) => RegressionThreshold::StdDevs(z),
            },
        ),
    };

    let historical_runs = storage.get_test_runs(test_name, 50).await?;
    if let Some(regression) =
        detect_duration_regressions(test_name, &historical_runs, min_history, threshold)
    {
        print_duration_warning(test_name, regression.current_ms, regression.mean_ms);
        regressions.push(regression);
    }
    Ok(())
}

/// Marks the session finished and purges stale data.
///
/// # Errors
///
/// Returns a storage error if the session update or purge fails.
pub async fn finalize_session(
    storage: &StorageBackend,
    session: ActiveSession,
    detector: &BayesianDetector,
    scores: &[FlakinessScore],
    config: &Config,
) -> Result<(), NinetyNineError> {
    let test_count = u32::try_from(scores.len()).unwrap_or(u32::MAX);
    let flaky_count =
        u32::try_from(scores.iter().filter(|s| detector.is_flaky(s)).count()).unwrap_or(u32::MAX);

    storage
        .finish_session(session.id(), test_count, flaky_count)
        .await?;

    let purged = storage
        .purge_older_than(config.storage.retention_days)
        .await?;
    if purged > 0 {
        tracing::info!(purged, "purged old test runs");
    }

    Ok(())
}

/// Quarantines tests exceeding configured flakiness thresholds.
///
/// # Errors
///
/// Returns a storage error if quarantine queries fail.
pub async fn auto_quarantine(
    config: &Config,
    storage: &StorageBackend,
    detector: &BayesianDetector,
    scores: &[FlakinessScore],
) -> Result<(), NinetyNineError> {
    if !config.quarantine.enabled || !config.quarantine.auto_quarantine {
        return Ok(());
    }

    let threshold = &config.quarantine.threshold;
    for score in scores {
        if !detector.is_flaky(score) {
            continue;
        }

        let exceeds_score = score.probability_flaky >= threshold.flakiness_score;
        let exceeds_failures = score.consecutive_failures >= threshold.consecutive_failures;
        let exceeds_rate = score.fail_rate >= threshold.failure_rate;

        if (exceeds_score || exceeds_failures || exceeds_rate)
            && !storage.is_quarantined(&score.test_name).await?
        {
            storage
                .quarantine_test(
                    &score.test_name,
                    "auto-quarantined: exceeded flakiness threshold",
                    score.probability_flaky,
                    true,
                )
                .await?;
            println!("Auto-quarantined: {}", score.test_name);
        }
    }

    Ok(())
}

pub fn print_analysis(runs: &[TestRun], config: &Config) {
    let patterns = detect_patterns(runs);
    if !patterns.is_empty() {
        println!("\nDetected Patterns:");
        for p in &patterns {
            println!(
                "  [{:.0}% corr] {} — {}",
                p.correlation * 100.0,
                p.pattern_type,
                p.examples.first().map_or("", String::as_str)
            );
        }
    }

    let test_names: BTreeSet<&str> = runs.iter().map(|r| r.test_name.as_ref()).collect();

    let mut degrading = Vec::new();
    for name in &test_names {
        if let Some(trend) = calculate_trend(name, runs, config.detection.window_size) {
            if trend.direction == crate::types::TrendDirection::Degrading {
                degrading.push(trend);
            }
        }
    }

    if !degrading.is_empty() {
        println!("\nDegrading Trends:");
        for t in &degrading {
            println!(
                "  {} — {:.1}% -> {:.1}% (delta: {:+.1}%)",
                t.test_name,
                t.previous_score * 100.0,
                t.recent_score * 100.0,
                t.score_delta * 100.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::Config;
    use crate::storage::sqlite::SqliteStorage;
    use crate::test_helpers::{test_run_with_duration, test_score, test_session};
    use crate::types::TestName;

    fn flaky_score(name: &str, probability: f64) -> FlakinessScore {
        let mut s = test_score(name, probability);
        s.confidence = 0.99;
        s.fail_rate = probability;
        s.consecutive_failures = 5;
        s
    }

    async fn storage_with_duration_spike() -> StorageBackend {
        let storage = StorageBackend::Sqlite(SqliteStorage::in_memory().unwrap());
        let session = test_session("abc", "main");
        storage.store_session(&session).await.unwrap();
        for _ in 0..10 {
            let run = test_run_with_duration("tests::slow", 100);
            storage.store_test_run(&run, &session.id).await.unwrap();
        }
        let spike = test_run_with_duration("tests::slow", 500);
        storage.store_test_run(&spike, &session.id).await.unwrap();
        storage
    }

    #[tokio::test]
    async fn duration_check_disabled_by_config() {
        let storage = storage_with_duration_spike().await;
        let cfg = DurationRegressionConfig {
            enabled: false,
            min_history_runs: 5,
            threshold: DurationThreshold::StdDev(2.0),
        };
        let mut regressions = Vec::new();
        check_duration_regression(&storage, "tests::slow", Some(&cfg), &mut regressions)
            .await
            .unwrap();
        assert!(regressions.is_empty());
    }

    #[tokio::test]
    async fn duration_check_honours_configured_threshold() {
        let storage = storage_with_duration_spike().await;
        let cfg = DurationRegressionConfig {
            enabled: true,
            min_history_runs: 5,
            threshold: DurationThreshold::StdDev(2.0),
        };
        let mut regressions = Vec::new();
        check_duration_regression(&storage, "tests::slow", Some(&cfg), &mut regressions)
            .await
            .unwrap();
        assert_eq!(regressions.len(), 1);
        assert_eq!(regressions[0].test_name, TestName::from("tests::slow"));
    }

    #[tokio::test]
    async fn duration_check_defaults_apply_without_config() {
        let storage = storage_with_duration_spike().await;
        let mut regressions = Vec::new();
        check_duration_regression(&storage, "tests::slow", None, &mut regressions)
            .await
            .unwrap();
        assert_eq!(regressions.len(), 1);
    }

    #[tokio::test]
    async fn auto_quarantine_skips_when_disabled() {
        let mut config = Config::default();
        config.quarantine.enabled = false;
        let storage = StorageBackend::Sqlite(SqliteStorage::in_memory().unwrap());
        let detector = BayesianDetector::new(0.95);
        let scores = vec![flaky_score("tests::flaky", 0.9)];

        auto_quarantine(&config, &storage, &detector, &scores)
            .await
            .unwrap();
        assert!(!storage.is_quarantined("tests::flaky").await.unwrap());
    }

    #[tokio::test]
    async fn auto_quarantine_skips_when_auto_off() {
        let mut config = Config::default();
        config.quarantine.enabled = true;
        config.quarantine.auto_quarantine = false;
        let storage = StorageBackend::Sqlite(SqliteStorage::in_memory().unwrap());
        let detector = BayesianDetector::new(0.95);
        let scores = vec![flaky_score("tests::flaky", 0.9)];

        auto_quarantine(&config, &storage, &detector, &scores)
            .await
            .unwrap();
        assert!(!storage.is_quarantined("tests::flaky").await.unwrap());
    }

    #[tokio::test]
    async fn auto_quarantine_quarantines_above_threshold() {
        let mut config = Config::default();
        config.quarantine.enabled = true;
        config.quarantine.auto_quarantine = true;
        config.quarantine.threshold.flakiness_score = 0.5;
        let storage = StorageBackend::Sqlite(SqliteStorage::in_memory().unwrap());
        let detector = BayesianDetector::new(0.95);
        let scores = vec![flaky_score("tests::flaky", 0.9)];

        auto_quarantine(&config, &storage, &detector, &scores)
            .await
            .unwrap();
        assert!(storage.is_quarantined("tests::flaky").await.unwrap());
    }

    #[tokio::test]
    async fn auto_quarantine_skips_non_flaky() {
        let mut config = Config::default();
        config.quarantine.enabled = true;
        config.quarantine.auto_quarantine = true;
        let storage = StorageBackend::Sqlite(SqliteStorage::in_memory().unwrap());
        let detector = BayesianDetector::new(0.95);
        let scores = vec![test_score("tests::stable", 0.001)];

        auto_quarantine(&config, &storage, &detector, &scores)
            .await
            .unwrap();
        assert!(!storage.is_quarantined("tests::stable").await.unwrap());
    }

    #[tokio::test]
    async fn auto_quarantine_skips_already_quarantined() {
        let mut config = Config::default();
        config.quarantine.enabled = true;
        config.quarantine.auto_quarantine = true;
        config.quarantine.threshold.flakiness_score = 0.5;
        let storage = StorageBackend::Sqlite(SqliteStorage::in_memory().unwrap());
        storage
            .quarantine_test("tests::flaky", "manual", 0.9, false)
            .await
            .unwrap();
        let detector = BayesianDetector::new(0.95);
        let scores = vec![flaky_score("tests::flaky", 0.9)];

        auto_quarantine(&config, &storage, &detector, &scores)
            .await
            .unwrap();

        let entries = storage.get_quarantined_tests().await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].reason, "manual");
    }
}
