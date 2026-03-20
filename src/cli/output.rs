use std::borrow::Cow;
use std::time::Duration;

use clap::ValueEnum;
use colored::Colorize;

use crate::detector::BayesianDetector;
use crate::types::{
    FailurePattern, FlakinessCategory, FlakinessScore, QuarantineEntry, RunSession, TestRun,
    TrendSummary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Console,
    Json,
}

pub fn print_flakiness_report(scores: &[FlakinessScore], format: OutputFormat) {
    match format {
        OutputFormat::Console => print_console_report(scores),
        OutputFormat::Json => print_json_report(scores),
    }
}

pub fn print_summary(scores: &[FlakinessScore], detector: &BayesianDetector) {
    let total = scores.len();
    let flaky = scores.iter().filter(|s| detector.is_flaky(s)).count();
    let stable = total - flaky;
    println!(
        "\n{}: {} tests, {} flaky, {} stable",
        "Summary".bold(),
        total,
        flaky,
        stable,
    );
}

pub fn print_run_header(test_count: usize, iterations: u32) {
    println!(
        "\n{} {} tests ({} iterations each)...\n",
        "Running".bold(),
        test_count,
        iterations,
    );
}

pub fn format_test_result_line(
    test_name: &str,
    passed: u32,
    total: u32,
    duration: Duration,
    completed: usize,
    test_count: usize,
) -> String {
    let secs = duration.as_secs_f64();
    let name = truncate_name(test_name, 60);
    let counter = format!("[{completed}/{test_count}]").dimmed();

    if passed == total {
        format!(
            "{counter}  {} [{passed}/{total}] [{secs:.2}s] {name}",
            "PASS".green().bold(),
        )
    } else if passed == 0 {
        format!(
            "{counter}  {} [{passed}/{total}] [{secs:.2}s] {name}",
            "FAIL".red().bold(),
        )
    } else {
        format!(
            "{counter} {} [{passed}/{total}] [{secs:.2}s] {name}",
            "FLAKY".yellow().bold(),
        )
    }
}

pub fn print_run_summary(total: usize, passed: usize, flaky: usize, failed: usize) {
    println!(
        "\n{}: {} total, {} passed, {} flaky, {} failed",
        "Results".bold(),
        total,
        passed,
        flaky,
        failed,
    );
}

pub fn print_duration_warning(test_name: &str, current_ms: f64, mean_ms: f64, factor: f64) {
    println!(
        "  {} {} — {:.0}ms (mean: {:.0}ms, {:.1}x slower)",
        "SLOW".yellow().bold(),
        test_name,
        current_ms,
        mean_ms,
        factor,
    );
}

pub fn print_duration_regression_summary(count: usize) {
    if count > 0 {
        println!(
            "\n{}: {} tests with duration regression",
            "Duration".bold(),
            count,
        );
    }
}

fn print_console_report(scores: &[FlakinessScore]) {
    if scores.is_empty() {
        println!("{}", "No test results to display.".dimmed());
        return;
    }

    println!("\n{}\n", "Flaky Test Detection Report".bold().underline());

    println!(
        "{:<50} {:>10} {:>10} {:>10} {:>12}",
        "Test".bold(),
        "Runs".bold(),
        "Pass%".bold(),
        "P(flaky)".bold(),
        "Category".bold(),
    );
    println!("{}", "-".repeat(92));

    for score in scores {
        let category = FlakinessCategory::from_score(score.probability_flaky);
        let category_str = format_category(category);

        println!(
            "{:<50} {:>10} {:>9.1}% {:>9.1}% {:>12}",
            truncate_name(&score.test_name, 50),
            score.total_runs,
            score.pass_rate * 100.0,
            score.probability_flaky * 100.0,
            category_str,
        );
    }

    println!();
}

fn print_json_report(scores: &[FlakinessScore]) {
    match serde_json::to_string_pretty(scores) {
        Ok(json) => println!("{json}"),
        Err(e) => tracing::warn!("failed to serialize scores to JSON: {e}"),
    }
}

fn format_category(category: FlakinessCategory) -> String {
    let label = category.label();
    match category {
        FlakinessCategory::Stable => label.green().to_string(),
        FlakinessCategory::Occasional => label.yellow().to_string(),
        FlakinessCategory::Moderate => label.red().to_string(),
        FlakinessCategory::Frequent => label.red().bold().to_string(),
        FlakinessCategory::Critical => label.on_red().white().bold().to_string(),
    }
}

pub fn print_session_report(sessions: &[RunSession], format: OutputFormat) {
    match format {
        OutputFormat::Console => print_console_sessions(sessions),
        OutputFormat::Json => match serde_json::to_string_pretty(sessions) {
            Ok(json) => println!("{json}"),
            Err(e) => tracing::warn!("failed to serialize sessions to JSON: {e}"),
        },
    }
}

fn print_console_sessions(sessions: &[RunSession]) {
    if sessions.is_empty() {
        println!("{}", "No detection sessions found.".dimmed());
        return;
    }

    println!("\n{}\n", "Detection History".bold().underline());

    println!(
        "{:<24} {:>8} {:>8} {:<16} {:<12}",
        "Date".bold(),
        "Tests".bold(),
        "Flaky".bold(),
        "Branch".bold(),
        "Commit".bold(),
    );
    println!("{}", "-".repeat(70));

    for session in sessions {
        let date = session.started_at.format("%Y-%m-%d %H:%M:%S");
        let commit_short = if session.commit_hash.len() >= 7 {
            &session.commit_hash[..7]
        } else {
            &session.commit_hash
        };

        println!(
            "{:<24} {:>8} {:>8} {:<16} {:<12}",
            date,
            session.test_count,
            session.flaky_count,
            truncate_name(&session.branch, 16),
            commit_short,
        );
    }

    println!();
}

pub fn print_test_detail(
    score: &FlakinessScore,
    runs: &[TestRun],
    trend: Option<&TrendSummary>,
    patterns: &[FailurePattern],
    format: OutputFormat,
) {
    match format {
        OutputFormat::Console => print_console_test_detail(score, runs, trend, patterns),
        OutputFormat::Json => {
            let detail = serde_json::json!({
                "score": score,
                "recent_runs": runs,
                "trend": trend,
                "patterns": patterns,
            });
            match serde_json::to_string_pretty(&detail) {
                Ok(json) => println!("{json}"),
                Err(e) => tracing::warn!("failed to serialize test detail to JSON: {e}"),
            }
        }
    }
}

fn print_console_test_detail(
    score: &FlakinessScore,
    runs: &[TestRun],
    trend: Option<&TrendSummary>,
    patterns: &[FailurePattern],
) {
    let category = FlakinessCategory::from_score(score.probability_flaky);

    println!("\n{}\n", score.test_name.bold().underline());
    println!("  Category:      {}", format_category(category));
    println!("  P(flaky):      {:.1}%", score.probability_flaky * 100.0);
    println!("  Pass rate:     {:.1}%", score.pass_rate * 100.0);
    println!("  Total runs:    {}", score.total_runs);
    println!("  Consec. fails: {}", score.consecutive_failures);
    println!(
        "  Credible int:  [{:.3}, {:.3}]",
        score.bayesian_params.credible_interval_lower,
        score.bayesian_params.credible_interval_upper,
    );

    if let Some(t) = trend {
        let arrow = match t.direction {
            crate::types::TrendDirection::Improving => "improving".green(),
            crate::types::TrendDirection::Stable => "stable".dimmed(),
            crate::types::TrendDirection::Degrading => "degrading".red(),
        };
        println!(
            "  Trend:         {} (delta: {:+.1}%)",
            arrow,
            t.score_delta * 100.0,
        );
    }

    if !patterns.is_empty() {
        println!("\n  {}\n", "Failure Patterns".bold());
        for p in patterns {
            println!(
                "    [{:.0}% corr] {}",
                p.correlation * 100.0,
                p.pattern_type,
            );
            for example in &p.examples {
                println!("      {example}");
            }
        }
    }

    if !runs.is_empty() {
        println!("\n  {}\n", "Recent Runs".bold());
        for run in runs.iter().take(10) {
            let symbol = match run.outcome {
                crate::types::TestOutcome::Passed => "PASS".green(),
                crate::types::TestOutcome::Failed => "FAIL".red(),
                crate::types::TestOutcome::Timeout => "TIME".yellow(),
                crate::types::TestOutcome::Panic => "PANC".red().bold(),
                crate::types::TestOutcome::Ignored => "SKIP".dimmed(),
            };
            println!(
                "    [{symbol}] {:>6}ms  {}",
                run.duration.as_millis(),
                run.timestamp.format("%Y-%m-%d %H:%M:%S"),
            );
        }
    }

    println!();
}

pub fn print_quarantine_list(entries: &[QuarantineEntry], format: OutputFormat) {
    match format {
        OutputFormat::Console => print_console_quarantine(entries),
        OutputFormat::Json => match serde_json::to_string_pretty(entries) {
            Ok(json) => println!("{json}"),
            Err(e) => tracing::warn!("failed to serialize quarantine list to JSON: {e}"),
        },
    }
}

fn print_console_quarantine(entries: &[QuarantineEntry]) {
    if entries.is_empty() {
        println!("{}", "No quarantined tests.".dimmed());
        return;
    }

    println!("\n{}\n", "Quarantined Tests".bold().underline());

    println!(
        "{:<50} {:>10} {:>10} {:<24}",
        "Test".bold(),
        "Score".bold(),
        "Auto".bold(),
        "Since".bold(),
    );
    println!("{}", "-".repeat(96));

    for entry in entries {
        let auto_label = if entry.auto_quarantined { "yes" } else { "no" };
        println!(
            "{:<50} {:>9.1}% {:>10} {:<24}",
            truncate_name(&entry.test_name, 50),
            entry.flakiness_score * 100.0,
            auto_label,
            entry.quarantined_at.format("%Y-%m-%d %H:%M:%S"),
        );
    }

    println!();
}

fn truncate_name(name: &str, max_len: usize) -> Cow<'_, str> {
    if name.len() <= max_len {
        Cow::Borrowed(name)
    } else {
        let target = max_len.saturating_sub(3);
        let boundary = name
            .char_indices()
            .take_while(|(i, _)| *i <= target)
            .last()
            .map_or(0, |(i, _)| i);
        let truncated = &name[..boundary];
        Cow::Owned(format!("{truncated}..."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod truncate_name_tests {
        use super::*;
        use proptest::prelude::*;
        use rstest::rstest;

        #[rstest]
        #[case("short", 10, "short")]
        #[case("exactly_ten", 11, "exactly_ten")]
        #[case("this_is_a_very_long_test_name", 10, "this_is...")]
        fn truncates_correctly(
            #[case] input: &str,
            #[case] max_len: usize,
            #[case] expected: &str,
        ) {
            assert_eq!(truncate_name(input, max_len).as_ref(), expected);
        }

        proptest! {
            #[test]
            fn never_exceeds_max_length(
                name in ".{0,200}",
                max_len in 4usize..100,
            ) {
                let result = truncate_name(&name, max_len);
                prop_assert!(result.len() <= max_len);
            }
        }
    }
}
