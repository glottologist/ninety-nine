use std::path::Path;
use std::time::Duration;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use cargo_ninety_nine::analysis::pattern::detect_patterns;
use cargo_ninety_nine::analysis::trend::calculate_trend;
use cargo_ninety_nine::ci::{generate_github_actions, generate_gitlab_ci};
use cargo_ninety_nine::cli::export::{export_csv, export_html, export_json, export_junit};
use cargo_ninety_nine::cli::output::{
    print_diagnose_report, print_duration_regression_summary, print_flakiness_report,
    print_quarantine_list, print_run_summary, print_session_report, print_summary,
    print_test_detail,
};
use cargo_ninety_nine::cli::{
    CargoSubcommand, CiCommand, CiTarget, Cli, Commands, ExportFormat, OutputFormat,
    QuarantineCommand,
};
use cargo_ninety_nine::config;
use cargo_ninety_nine::config::model::Config;
use cargo_ninety_nine::detector::BayesianDetector;
use cargo_ninety_nine::diagnose::{DiagnoseOpts, run_diagnose};
use cargo_ninety_nine::discovery::{SelectOpts, discover_and_filter_tests};
use cargo_ninety_nine::env::detect_git_info;
use cargo_ninety_nine::error::NinetyNineError;
use cargo_ninety_nine::orchestrator::{
    SuiteResults, auto_quarantine, execute_test_suite, finalize_session, print_analysis,
};
use cargo_ninety_nine::runner::executor::ExecutionConfig;
use cargo_ninety_nine::runner::{RunnerBackend, cargo_available};
use cargo_ninety_nine::storage::{Storage, StorageBackend, open_storage};
use cargo_ninety_nine::types::ActiveSession;

struct TestOpts<'a> {
    filter_expr: Option<&'a str>,
    iterations: u32,
    confidence: f64,
    output_format: OutputFormat,
    non_interactive: bool,
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

    let project_dir = args.project_dir.canonicalize().unwrap_or(args.project_dir);

    if let Err(e) = run(
        &project_dir,
        args.command,
        args.output,
        args.non_interactive,
    )
    .await
    {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run(
    project_dir: &Path,
    command: Commands,
    output_format: OutputFormat,
    non_interactive: bool,
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
                non_interactive,
            };
            run_test(project_dir, &config, &storage, &opts).await
        }
        Commands::Diagnose {
            filter_expr,
            stress_runs,
            isolation_runs,
            record,
            no_record,
            record_dir,
            confidence,
        } => {
            let storage = open_storage(&config).await?;
            run_diagnose_cmd(
                project_dir,
                &config,
                &storage,
                DiagnoseCli {
                    filter_expr: filter_expr.as_deref(),
                    stress_runs,
                    isolation_runs,
                    record,
                    no_record,
                    record_dir,
                    confidence,
                    output_format,
                },
            )
            .await
        }
        Commands::Init { force } => run_init(project_dir, force),
        Commands::History { filter, limit } => {
            let storage = open_storage(&config).await?;
            run_history(
                &storage,
                filter.as_deref(),
                limit,
                output_format,
                non_interactive,
            )
            .await
        }
        Commands::Status { test_name } => {
            let storage = open_storage(&config).await?;
            run_status(
                &storage,
                &config,
                test_name.as_deref(),
                output_format,
                non_interactive,
            )
            .await
        }
        Commands::Export { format, path } => {
            let storage = open_storage(&config).await?;
            run_export(&storage, &config, format, &path).await
        }
        Commands::Quarantine(subcmd) => {
            let storage = open_storage(&config).await?;
            run_quarantine(&storage, subcmd, output_format).await
        }
        Commands::Ci(subcmd) => run_ci(&config, subcmd),
    }
}

struct DiagnoseCli<'a> {
    filter_expr: Option<&'a str>,
    stress_runs: Option<u32>,
    isolation_runs: Option<u32>,
    record: bool,
    no_record: bool,
    record_dir: Option<std::path::PathBuf>,
    confidence: Option<f64>,
    output_format: OutputFormat,
}

async fn run_diagnose_cmd(
    project_dir: &Path,
    config: &Config,
    storage: &StorageBackend,
    cli: DiagnoseCli<'_>,
) -> Result<(), NinetyNineError> {
    if !cargo_available() {
        return Err(NinetyNineError::NoRunnerAvailable);
    }

    config.diagnose.validate()?;

    if cli.record && cli.no_record {
        return Err(NinetyNineError::InvalidConfig {
            message: "cannot pass both --record and --no-record".into(),
        });
    }

    let confidence = cli
        .confidence
        .unwrap_or(config.detection.confidence_threshold);
    let select = SelectOpts {
        filter_expr: cli.filter_expr,
        confidence,
    };
    let backend = build_backend(config, project_dir);
    let Some(tests) = discover_and_filter_tests(&backend, storage, &select).await? else {
        return Ok(());
    };

    let do_record = if cli.no_record {
        false
    } else if cli.record {
        true
    } else {
        config.diagnose.record
    };

    let opts = DiagnoseOpts {
        stress_runs: cli.stress_runs.unwrap_or(config.diagnose.stress_runs),
        isolation_runs: cli.isolation_runs.unwrap_or(config.diagnose.isolation_runs),
        stress_threads: config.diagnose.effective_stress_threads(),
        stress_timeout: Duration::from_secs(config.diagnose.stress_timeout_secs),
        isolation_timeout: Duration::from_secs(config.retry.max_retry_time_secs),
        record: do_record,
        record_dir: cli
            .record_dir
            .unwrap_or_else(|| config.diagnose.record_dir.clone()), // clone: DiagnoseOpts owns path
        record_attempts: config.diagnose.record_attempts,
        confidence,
    };

    let results = run_diagnose(project_dir, &backend, storage, &tests, &opts).await?;
    print_diagnose_report(&results, cli.output_format);
    Ok(())
}

async fn run_test(
    project_dir: &Path,
    config: &Config,
    storage: &StorageBackend,
    opts: &TestOpts<'_>,
) -> Result<(), NinetyNineError> {
    if !cargo_available() {
        return Err(NinetyNineError::NoRunnerAvailable);
    }

    let backend = build_backend(config, project_dir);
    let select = SelectOpts {
        filter_expr: opts.filter_expr,
        confidence: opts.confidence,
    };
    let Some(tests) = discover_and_filter_tests(&backend, storage, &select).await? else {
        return Ok(());
    };

    let (commit_hash, branch) = detect_git_info();
    let session = ActiveSession::start(&commit_hash, &branch);
    storage.store_session(&session.to_run_session()).await?;

    let detector = BayesianDetector::new(opts.confidence);
    let mut results = execute_test_suite(
        &backend,
        storage,
        &tests,
        opts.iterations,
        &detector,
        &session,
        config.detection.duration_regression.as_ref(),
    )
    .await?;

    print_run_summary(
        tests.len(),
        results.pass_count,
        results.flaky_count,
        results.fail_count,
        results.skip_count,
    );
    print_duration_regression_summary(results.duration_regressions.len());

    finalize_session(storage, session, &detector, &results.scores, config).await?;
    auto_quarantine(config, storage, &detector, &results.scores).await?;

    if opts.non_interactive {
        print_results(&mut results, config, &detector, opts.output_format);
    } else {
        cargo_ninety_nine::tui::run_scores(results.scores, opts.confidence, storage, config)?;
    }

    Ok(())
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

async fn run_history(
    storage: &StorageBackend,
    filter: Option<&str>,
    limit: u32,
    output_format: OutputFormat,
    non_interactive: bool,
) -> Result<(), NinetyNineError> {
    let sessions = storage.get_recent_sessions(limit).await?;

    let sessions = match filter {
        Some(f) => sessions
            .into_iter()
            .filter(|s| s.branch.contains(f) || s.commit_hash.contains(f))
            .collect(),
        None => sessions,
    };

    if non_interactive {
        print_session_report(&sessions, output_format);
    } else {
        cargo_ninety_nine::tui::run_history(sessions, storage)?;
    }
    Ok(())
}

async fn run_status(
    storage: &StorageBackend,
    config: &Config,
    test_name: Option<&str>,
    output_format: OutputFormat,
    non_interactive: bool,
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

        print_test_detail(
            &score,
            &runs,
            trend.as_ref(),
            &patterns,
            output_format,
            config.detection.confidence_threshold,
        );
    } else if non_interactive {
        let scores = storage.get_all_scores().await?;
        print_flakiness_report(
            &scores,
            output_format,
            config.detection.confidence_threshold,
        );
    } else {
        let scores = storage.get_all_scores().await?;
        cargo_ninety_nine::tui::run_scores(
            scores,
            config.detection.confidence_threshold,
            storage,
            config,
        )?;
    }
    Ok(())
}

async fn run_export(
    storage: &StorageBackend,
    config: &Config,
    format: ExportFormat,
    path: &Path,
) -> Result<(), NinetyNineError> {
    let scores = storage.get_all_scores().await?;
    let threshold = config.detection.confidence_threshold;

    match format {
        ExportFormat::Junit => export_junit(&scores, path, threshold)?,
        ExportFormat::Html => export_html(&scores, path, threshold)?,
        ExportFormat::Csv => export_csv(&scores, path, threshold)?,
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
