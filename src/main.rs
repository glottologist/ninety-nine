use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use tracing_subscriber::EnvFilter;

use cargo_ninety_nine::analysis::duration::{DurationRegression, detect_duration_regressions};
use cargo_ninety_nine::analysis::pattern::detect_patterns;
use cargo_ninety_nine::analysis::trend::calculate_trend;
use cargo_ninety_nine::ci::{generate_github_actions, generate_gitlab_ci};
use cargo_ninety_nine::cli::export::{export_csv, export_html, export_json, export_junit};
use cargo_ninety_nine::cli::output::{
    format_test_result_line, print_duration_regression_summary, print_duration_warning,
    print_flakiness_report, print_quarantine_list, print_run_header, print_run_summary,
    print_session_report, print_summary, print_test_detail,
};
use cargo_ninety_nine::cli::{
    CargoSubcommand, CiCommand, CiTarget, Cli, Commands, ExportFormat, OutputFormat,
    QuarantineCommand,
};
use cargo_ninety_nine::config;
use cargo_ninety_nine::config::model::Config;
use cargo_ninety_nine::detector::BayesianDetector;
use cargo_ninety_nine::error::NinetyNineError;
use cargo_ninety_nine::filter;
use cargo_ninety_nine::filter::eval::{TestMetadata, eval};
use cargo_ninety_nine::runner::executor::ExecutionConfig;
use cargo_ninety_nine::runner::listing::TestCase;
use cargo_ninety_nine::runner::{RunnerBackend, detect_available_runner, execute_iterations};
use cargo_ninety_nine::storage::{Storage, StorageBackend, open_storage};
use cargo_ninety_nine::types::{
    ActiveSession, FlakinessScore, TestEnvironment, TestOutcome, TestRun,
};

struct TestOpts<'a> {
    filter_expr: Option<&'a str>,
    iterations: u32,
    confidence: f64,
    output_format: OutputFormat,
}

struct SuiteResults {
    scores: Vec<FlakinessScore>,
    all_runs: Vec<TestRun>,
    pass_count: usize,
    flaky_count: usize,
    fail_count: usize,
    duration_regressions: Vec<DurationRegression>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let CargoSubcommand::NinetyNine(args) = cli.command;

    let filter = if args.verbose {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("cargo_ninety_nine=debug"))
    } else {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("cargo_ninety_nine=warn"))
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();

    if let Err(e) = run(&args.project_dir, args.command, args.output).await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run(
    project_dir: &Path,
    command: Commands,
    output_format: OutputFormat,
) -> Result<(), NinetyNineError> {
    let config = config::load_config(project_dir)?;

    match command {
        Commands::Test {
            filter_expr,
            iterations,
            confidence,
        } => {
            let storage = open_storage(&config).await?;
            let opts = TestOpts {
                filter_expr: filter_expr.as_deref(),
                iterations: iterations.unwrap_or(config.detection.min_runs),
                confidence: confidence.unwrap_or(config.detection.confidence_threshold),
                output_format,
            };
            run_test(project_dir, &config, &storage, &opts).await
        }
        Commands::Init { force } => run_init(project_dir, force),
        Commands::History { filter, limit } => {
            let storage = open_storage(&config).await?;
            run_history(&storage, filter.as_deref(), limit, output_format).await
        }
        Commands::Status { test_name } => {
            let storage = open_storage(&config).await?;
            run_status(&storage, &config, test_name.as_deref(), output_format).await
        }
        Commands::Export { format, path } => {
            let storage = open_storage(&config).await?;
            run_export(&storage, format, &path).await
        }
        Commands::Quarantine(subcmd) => {
            let storage = open_storage(&config).await?;
            run_quarantine(&storage, subcmd, output_format).await
        }
        Commands::Ci(subcmd) => run_ci(&config, subcmd),
    }
}

async fn run_test(
    project_dir: &Path,
    config: &Config,
    storage: &StorageBackend,
    opts: &TestOpts<'_>,
) -> Result<(), NinetyNineError> {
    detect_available_runner().ok_or(NinetyNineError::NoRunnerAvailable)?;

    let backend = build_backend(config, project_dir);
    let Some(tests) = discover_and_filter_tests(&backend, storage, opts).await? else {
        return Ok(());
    };

    let (commit_hash, branch) = detect_git_info();
    let session = ActiveSession::start(&commit_hash, &branch);
    storage.store_session(&session.to_run_session()).await?;

    let detector = BayesianDetector::new(opts.confidence);
    let mut results =
        execute_test_suite(&backend, storage, &tests, opts, &detector, &session).await?;

    print_run_summary(
        tests.len(),
        results.pass_count,
        results.flaky_count,
        results.fail_count,
    );
    print_duration_regression_summary(results.duration_regressions.len());

    finalize_session(storage, session, &detector, &results.scores, config).await?;
    print_results(&mut results, config, &detector, opts.output_format);
    auto_quarantine(config, storage, &detector, &results.scores).await
}

fn print_results(
    results: &mut SuiteResults,
    config: &Config,
    detector: &BayesianDetector,
    output_format: OutputFormat,
) {
    let threshold = config.detection.confidence_threshold;
    results.scores.sort_by(|a, b| {
        b.effective_score(threshold)
            .partial_cmp(&a.effective_score(threshold))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if config.reporting.console.summary_only {
        print_summary(&results.scores, detector);
    } else {
        print_flakiness_report(
            &results.scores,
            output_format,
            config.detection.confidence_threshold,
        );
        print_analysis(&results.all_runs, config);
    }
}

/// Discovers tests and applies filter expression, returning `None` if no tests remain.
async fn discover_and_filter_tests(
    backend: &RunnerBackend,
    storage: &StorageBackend,
    opts: &TestOpts<'_>,
) -> Result<Option<Vec<TestCase>>, NinetyNineError> {
    let mut tests = backend.discover_tests("").await?;

    if tests.is_empty() {
        println!("No tests found.");
        return Ok(None);
    }

    let eval_ctx = filter::build_eval_context(storage, opts.confidence).await?;

    if let Some(expr_str) = opts.filter_expr {
        let expr = filter::compile_filter(expr_str)?;
        tests.retain(|tc| {
            let meta = TestMetadata {
                name: &tc.name,
                package_name: &tc.package_name,
                binary_name: &tc.binary_name,
                kind: &tc.binary_kind,
            };
            eval(&expr, &meta, &eval_ctx)
        });
    }

    if tests.is_empty() {
        println!("No tests matching filter.");
        return Ok(None);
    }

    tracing::info!(count = tests.len(), "discovered tests");
    Ok(Some(tests))
}

async fn execute_test_suite(
    backend: &RunnerBackend,
    storage: &StorageBackend,
    tests: &[TestCase],
    opts: &TestOpts<'_>,
    detector: &BayesianDetector,
    session: &ActiveSession,
) -> Result<SuiteResults, NinetyNineError> {
    let environment = Arc::new(detect_environment());
    let config = Arc::new(backend.execution_config().clone()); // clone: shared across spawn_blocking tasks
    let semaphore = Arc::new(tokio::sync::Semaphore::new(config.concurrency));
    let iterations = opts.iterations;
    let total = tests.len();

    print_run_header(total, iterations);

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
    let mut completed = 0usize;
    let mut duration_regressions = Vec::new();

    while let Some(result) = join_set.join_next().await {
        let (test_case, runs, elapsed) =
            result.map_err(|e| NinetyNineError::RunnerExecution {
                message: format!("task join error: {e}"),
            })??;

        completed += 1;

        let passed = u32::try_from(
            runs.iter()
                .filter(|r| r.outcome == TestOutcome::Passed)
                .count(),
        )
        .unwrap_or(u32::MAX);

        if passed == iterations {
            pass_count += 1;
        } else if passed == 0 {
            fail_count += 1;
        } else {
            flaky_count += 1;
        }

        let line = format_test_result_line(
            &test_case.name,
            passed,
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

        check_duration_regression(storage, &test_case.name, &mut duration_regressions).await?;

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
        duration_regressions,
    })
}

async fn check_duration_regression(
    storage: &StorageBackend,
    test_name: &str,
    regressions: &mut Vec<DurationRegression>,
) -> Result<(), NinetyNineError> {
    let historical_runs = storage.get_test_runs(test_name, 50).await?;
    if let Some(regression) = detect_duration_regressions(test_name, &historical_runs, 10, 2.0) {
        print_duration_warning(
            test_name,
            regression.current_ms,
            regression.mean_ms,
            regression.deviation_factor,
        );
        regressions.push(regression);
    }
    Ok(())
}

fn print_analysis(runs: &[TestRun], config: &Config) {
    let patterns = detect_patterns(runs);
    if !patterns.is_empty() {
        println!("\nDetected Patterns:");
        for p in &patterns {
            println!(
                "  [{:.0}% corr] {} — {}",
                p.correlation * 100.0,
                p.pattern_type,
                p.examples.first().unwrap_or(&String::new())
            );
        }
    }

    let mut test_names: Vec<&str> = runs.iter().map(|r| r.test_name.as_ref()).collect();
    test_names.sort_unstable();
    test_names.dedup();

    let mut degrading = Vec::new();
    for name in &test_names {
        if let Some(trend) = calculate_trend(name, runs, config.detection.window_size) {
            if trend.direction == cargo_ninety_nine::types::TrendDirection::Degrading {
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

async fn auto_quarantine(
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

async fn finalize_session(
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

async fn run_history(
    storage: &StorageBackend,
    filter: Option<&str>,
    limit: u32,
    output_format: OutputFormat,
) -> Result<(), NinetyNineError> {
    let sessions = storage.get_recent_sessions(limit).await?;

    let sessions = match filter {
        Some(f) => sessions
            .into_iter()
            .filter(|s| s.branch.contains(f) || s.commit_hash.contains(f))
            .collect(),
        None => sessions,
    };

    print_session_report(&sessions, output_format);
    Ok(())
}

async fn run_status(
    storage: &StorageBackend,
    config: &Config,
    test_name: Option<&str>,
    output_format: OutputFormat,
) -> Result<(), NinetyNineError> {
    if let Some(name) = test_name {
        let score =
            storage
                .get_score(name)
                .await?
                .ok_or_else(|| NinetyNineError::TestNotFound {
                    name: name.to_string(),
                })?;
        let runs = storage.get_test_runs(name, 20).await?;

        let trend = calculate_trend(name, &runs, config.detection.window_size);
        let patterns = detect_patterns(&runs);

        print_test_detail(&score, &runs, trend.as_ref(), &patterns, output_format);
    } else {
        let scores = storage.get_all_scores().await?;
        print_flakiness_report(
            &scores,
            output_format,
            config.detection.confidence_threshold,
        );
    }
    Ok(())
}

async fn run_export(
    storage: &StorageBackend,
    format: ExportFormat,
    path: &Path,
) -> Result<(), NinetyNineError> {
    let scores = storage.get_all_scores().await?;

    match format {
        ExportFormat::Junit => export_junit(&scores, path)?,
        ExportFormat::Html => export_html(&scores, path)?,
        ExportFormat::Csv => export_csv(&scores, path)?,
        ExportFormat::Json => export_json(&scores, path)?,
    }

    println!(
        "Exported {} test scores to {}",
        scores.len(),
        path.display()
    );
    Ok(())
}

async fn run_quarantine(
    storage: &StorageBackend,
    command: QuarantineCommand,
    output_format: OutputFormat,
) -> Result<(), NinetyNineError> {
    match command {
        QuarantineCommand::List => {
            let entries = storage.get_quarantined_tests().await?;
            print_quarantine_list(&entries, output_format);
        }
        QuarantineCommand::Add { test_name, reason } => {
            let score = storage
                .get_score(&test_name)
                .await?
                .map_or(0.0, |s| s.probability_flaky);
            storage
                .quarantine_test(&test_name, &reason, score, false)
                .await?;
            println!("Quarantined: {test_name}");
        }
        QuarantineCommand::Remove { test_name } => {
            storage.unquarantine_test(&test_name).await?;
            println!("Unquarantined: {test_name}");
        }
    }
    Ok(())
}

fn run_ci(config: &Config, command: CiCommand) -> Result<(), NinetyNineError> {
    match command {
        CiCommand::Generate { provider, path } => {
            let yaml = match provider {
                CiTarget::Github => generate_github_actions(config),
                CiTarget::Gitlab => generate_gitlab_ci(config),
            };

            match path {
                Some(p) => {
                    std::fs::write(&p, &yaml)?;
                    println!("Generated CI workflow: {}", p.display());
                }
                None => print!("{yaml}"),
            }
        }
    }
    Ok(())
}

fn build_backend(config: &Config, project_dir: &Path) -> RunnerBackend {
    let exec_config = ExecutionConfig {
        concurrency: usize::try_from(config.detection.parallel_runs).unwrap_or(4),
        timeout: Duration::from_secs(config.retry.max_retry_time_secs),
        retries: config.retry.unit_test_retries,
        retry_delay: config::backoff_base_delay(&config.retry.backoff_strategy),
    };
    RunnerBackend::native(project_dir, exec_config)
}

fn run_init(project_dir: &Path, force: bool) -> Result<(), NinetyNineError> {
    let config_path = project_dir.join(".ninety-nine.toml");

    if config_path.exists() && !force {
        return Err(NinetyNineError::InvalidConfig {
            message: format!(
                "config file already exists: {}. Use --force to overwrite.",
                config_path.display()
            ),
        });
    }

    let toml_str = config::default_config_toml()?;
    std::fs::write(&config_path, toml_str)?;

    println!("Created {}", config_path.display());
    Ok(())
}

fn detect_git_info() -> (String, String) {
    let commit = duct::cmd!("git", "rev-parse", "HEAD")
        .stdout_capture()
        .stderr_null()
        .read()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let branch = duct::cmd!("git", "branch", "--show-current")
        .stdout_capture()
        .stderr_null()
        .read()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    (commit, branch)
}

fn detect_environment() -> TestEnvironment {
    let rust_version = duct::cmd!("rustc", "--version")
        .stdout_capture()
        .stderr_null()
        .read()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    TestEnvironment {
        os: std::env::consts::OS.to_string(),
        rust_version,
        cpu_count: std::thread::available_parallelism()
            .map(|n| u32::try_from(n.get()).unwrap_or(u32::MAX))
            .unwrap_or(1),
        memory_gb: detect_memory_gb(),
        is_ci: std::env::var("CI").is_ok(),
        ci_provider: detect_ci_provider(),
    }
}

fn detect_memory_gb() -> f64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .find(|line| line.starts_with("MemTotal:"))
                .and_then(|line| {
                    line.split_whitespace()
                        .nth(1)
                        .and_then(|kb| kb.parse::<u64>().ok())
                })
                .map(|kb| f64::from(u32::try_from(kb / 1024).unwrap_or(u32::MAX)) / 1024.0)
        })
        .unwrap_or(0.0)
}

fn detect_ci_provider() -> Option<String> {
    if std::env::var("GITHUB_ACTIONS").is_ok() {
        Some("GitHub Actions".to_string())
    } else if std::env::var("GITLAB_CI").is_ok() {
        Some("GitLab CI".to_string())
    } else if std::env::var("JENKINS_URL").is_ok() {
        Some("Jenkins".to_string())
    } else if std::env::var("CIRCLECI").is_ok() {
        Some("CircleCI".to_string())
    } else if std::env::var("BUILDKITE").is_ok() {
        Some("Buildkite".to_string())
    } else if std::env::var("TF_BUILD").is_ok() {
        Some("Azure DevOps".to_string())
    } else {
        None
    }
}
