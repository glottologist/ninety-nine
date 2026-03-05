use std::path::Path;
use std::time::Duration;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use tracing_subscriber::EnvFilter;

use cargo_ninety_nine::analysis::pattern::detect_patterns;
use cargo_ninety_nine::analysis::trend::calculate_trend;
use cargo_ninety_nine::ci::{generate_github_actions, generate_gitlab_ci};
use cargo_ninety_nine::cli::export::{export_csv, export_html, export_junit};
use cargo_ninety_nine::cli::output::{
    print_flakiness_report, print_quarantine_list, print_session_report, print_summary,
    print_test_detail,
};
use cargo_ninety_nine::cli::{
    CargoSubcommand, CiCommand, CiTarget, Cli, Commands, ExportFormat, OutputFormat,
    QuarantineCommand,
};
use cargo_ninety_nine::config;
use cargo_ninety_nine::config::model::{BackoffStrategy, Config};
use cargo_ninety_nine::detector::BayesianDetector;
use cargo_ninety_nine::error::NinetyNineError;
use cargo_ninety_nine::runner::executor::ExecutionConfig;
use cargo_ninety_nine::runner::{RunnerBackend, detect_available_runner};
use cargo_ninety_nine::storage::{SqliteStorage, Storage};
use cargo_ninety_nine::types::{FlakinessCategory, RunSession, TestEnvironment, TestRun};

struct DetectOpts<'a> {
    filter: Option<&'a str>,
    iterations: u32,
    confidence: f64,
    output_format: OutputFormat,
    verbose: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let CargoSubcommand::NinetyNine(args) = cli.command;

    if let Err(e) = run(&args.project_dir, args.command, args.output, args.verbose).await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run(
    project_dir: &Path,
    command: Commands,
    output_format: OutputFormat,
    verbose: bool,
) -> Result<(), NinetyNineError> {
    let config = config::load_config(project_dir)?;

    match command {
        Commands::Detect {
            filter,
            iterations,
            confidence,
        } => {
            let storage = SqliteStorage::open(&config.storage.database_path)?;
            let opts = DetectOpts {
                filter: filter.as_deref(),
                iterations,
                confidence,
                output_format,
                verbose,
            };
            run_detect(project_dir, &config, &storage, &opts).await
        }
        Commands::Init { force } => run_init(project_dir, force),
        Commands::History { filter, limit } => {
            let storage = SqliteStorage::open(&config.storage.database_path)?;
            run_history(&storage, filter.as_deref(), limit, output_format)
        }
        Commands::Status { test_name } => {
            let storage = SqliteStorage::open(&config.storage.database_path)?;
            run_status(&storage, &config, test_name.as_deref(), output_format)
        }
        Commands::Export { format, path } => {
            let storage = SqliteStorage::open(&config.storage.database_path)?;
            run_export(&storage, format, &path)
        }
        Commands::Quarantine(subcmd) => {
            let storage = SqliteStorage::open(&config.storage.database_path)?;
            run_quarantine(&storage, subcmd, output_format)
        }
        Commands::Ci(subcmd) => run_ci(&config, subcmd),
    }
}

async fn run_detect(
    project_dir: &Path,
    config: &Config,
    storage: &SqliteStorage,
    opts: &DetectOpts<'_>,
) -> Result<(), NinetyNineError> {
    detect_available_runner().ok_or(NinetyNineError::NoRunnerAvailable)?;

    let backend = build_backend(config, project_dir);
    let tests = backend.discover_tests(opts.filter.unwrap_or("")).await?;

    if tests.is_empty() {
        println!("No tests found matching filter.");
        return Ok(());
    }

    tracing::info!(count = tests.len(), "discovered tests");

    let (commit_hash, branch) = detect_git_info();
    let session = RunSession::start(&commit_hash, &branch);
    storage.store_session(&session)?;

    let environment = detect_environment();
    let detector = BayesianDetector::new(opts.confidence);
    let show_bar = config.reporting.console.progress_bar && !opts.verbose;
    let pb = create_progress_bar(tests.len(), show_bar);
    let mut scores = Vec::with_capacity(tests.len());
    let mut all_runs: Vec<TestRun> = Vec::new();

    for test_case in &tests {
        pb.set_message(test_case.name.clone()); // clone: progress bar takes ownership for display
        let runs = backend
            .run_test_repeatedly(test_case, opts.iterations, &environment)
            .await?;

        for run in &runs {
            storage.store_test_run(run, &session.id)?;
        }

        let score = detector.calculate_flakiness_score(&test_case.name, &runs);
        storage.store_flakiness_score(&score)?;

        if opts.verbose {
            let category = FlakinessCategory::from_score(score.probability_flaky);
            println!(
                "  [{category}] {} — {:.1}% flaky",
                test_case.name,
                score.probability_flaky * 100.0
            );
        }

        all_runs.extend(runs);
        scores.push(score);
        pb.inc(1);
    }

    pb.finish_and_clear();
    finalize_session(
        storage,
        &session,
        &detector,
        &scores,
        config.storage.retention_days,
    )?;

    scores.sort_by(|a, b| {
        b.probability_flaky
            .partial_cmp(&a.probability_flaky)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if config.reporting.console.summary_only {
        print_summary(&scores, &detector);
    } else {
        print_flakiness_report(&scores, opts.output_format);
        print_analysis(&all_runs, config);
    }

    auto_quarantine(config, storage, &detector, &scores)?;

    if config.ci.fail_on_flaky {
        let flaky_found = scores.iter().any(|s| detector.is_flaky(s));
        if flaky_found {
            return Err(NinetyNineError::InvalidConfig {
                message: "flaky tests detected (fail_on_flaky is enabled)".to_string(),
            });
        }
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

    let mut test_names: Vec<&str> = runs.iter().map(|r| r.test_name.as_str()).collect();
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

fn auto_quarantine(
    config: &Config,
    storage: &SqliteStorage,
    detector: &BayesianDetector,
    scores: &[cargo_ninety_nine::types::FlakinessScore],
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
            && !storage.is_quarantined(&score.test_name)?
        {
            storage.quarantine_test(
                &score.test_name,
                "auto-quarantined: exceeded flakiness threshold",
                score.probability_flaky,
                true,
            )?;
            println!("Auto-quarantined: {}", score.test_name);
        }
    }

    Ok(())
}

fn finalize_session(
    storage: &SqliteStorage,
    session: &RunSession,
    detector: &BayesianDetector,
    scores: &[cargo_ninety_nine::types::FlakinessScore],
    retention_days: u32,
) -> Result<(), NinetyNineError> {
    let test_count = u32::try_from(scores.len()).unwrap_or(u32::MAX);
    let flaky_count =
        u32::try_from(scores.iter().filter(|s| detector.is_flaky(s)).count()).unwrap_or(u32::MAX);

    storage.finish_session(&session.id, test_count, flaky_count)?;

    let purged = storage.purge_older_than(retention_days)?;
    if purged > 0 {
        tracing::info!(purged, "purged old test runs");
    }

    Ok(())
}

fn run_history(
    storage: &SqliteStorage,
    filter: Option<&str>,
    limit: u32,
    output_format: OutputFormat,
) -> Result<(), NinetyNineError> {
    let sessions = storage.get_recent_sessions(limit)?;

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

fn run_status(
    storage: &SqliteStorage,
    config: &Config,
    test_name: Option<&str>,
    output_format: OutputFormat,
) -> Result<(), NinetyNineError> {
    match test_name {
        Some(name) => {
            let score = storage
                .get_score(name)?
                .ok_or_else(|| NinetyNineError::InvalidConfig {
                    message: format!("no data for test: {name}"),
                })?;
            let runs = storage.get_test_runs(name, 20)?;

            let trend = calculate_trend(name, &runs, config.detection.window_size);
            let patterns = detect_patterns(&runs);

            print_test_detail(&score, &runs, trend.as_ref(), &patterns, output_format);
        }
        None => {
            let scores = storage.get_all_scores()?;
            print_flakiness_report(&scores, output_format);
        }
    }
    Ok(())
}

fn run_export(
    storage: &SqliteStorage,
    format: ExportFormat,
    path: &Path,
) -> Result<(), NinetyNineError> {
    let scores = storage.get_all_scores()?;

    match format {
        ExportFormat::Junit => export_junit(&scores, path)?,
        ExportFormat::Html => export_html(&scores, path)?,
        ExportFormat::Csv => export_csv(&scores, path)?,
    }

    println!(
        "Exported {} test scores to {}",
        scores.len(),
        path.display()
    );
    Ok(())
}

fn run_quarantine(
    storage: &SqliteStorage,
    command: QuarantineCommand,
    output_format: OutputFormat,
) -> Result<(), NinetyNineError> {
    match command {
        QuarantineCommand::List => {
            let entries = storage.get_quarantined_tests()?;
            print_quarantine_list(&entries, output_format);
        }
        QuarantineCommand::Add { test_name, reason } => {
            let score = storage
                .get_score(&test_name)?
                .map(|s| s.probability_flaky)
                .unwrap_or(0.0);
            storage.quarantine_test(&test_name, &reason, score, false)?;
            println!("Quarantined: {test_name}");
        }
        QuarantineCommand::Remove { test_name } => {
            storage.unquarantine_test(&test_name)?;
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
        retry_delay: backoff_base_delay(&config.retry.backoff_strategy),
    };
    RunnerBackend::native(project_dir, exec_config)
}

fn backoff_base_delay(strategy: &BackoffStrategy) -> Duration {
    match strategy {
        BackoffStrategy::None => Duration::ZERO,
        BackoffStrategy::Linear { delay_ms } => Duration::from_millis(*delay_ms),
        BackoffStrategy::Exponential { base_ms, .. } => Duration::from_millis(*base_ms),
        BackoffStrategy::Fibonacci { start_ms, .. } => Duration::from_millis(*start_ms),
    }
}

fn create_progress_bar(len: usize, enabled: bool) -> ProgressBar {
    if !enabled {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new(u64::try_from(len).unwrap_or(0));
    let style =
        ProgressStyle::with_template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-");
    pb.set_style(style);
    pb
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
