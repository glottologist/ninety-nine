use chrono::Timelike;

use crate::types::{FailurePattern, PatternType, TestOutcome, TestRun};

#[must_use]
pub fn detect_patterns(runs: &[TestRun]) -> Vec<FailurePattern> {
    let failures: Vec<&TestRun> = runs
        .iter()
        .filter(|r| {
            matches!(
                r.outcome,
                TestOutcome::Failed | TestOutcome::Panic | TestOutcome::Timeout
            )
        })
        .collect();

    if failures.is_empty() {
        return Vec::new();
    }

    let mut patterns = Vec::new();

    if let Some(p) = detect_time_of_day_pattern(&failures) {
        patterns.push(p);
    }

    if let Some(p) = detect_environmental_pattern(runs) {
        patterns.push(p);
    }

    if patterns.is_empty() && !failures.is_empty() {
        patterns.push(FailurePattern {
            pattern_type: PatternType::Random,
            occurrences: u32::try_from(failures.len()).unwrap_or(u32::MAX),
            correlation: 0.0,
            examples: failures
                .iter()
                .take(3)
                .map(|r| format!("{} at {}", r.test_name, r.timestamp))
                .collect(),
        });
    }

    patterns
}

fn detect_time_of_day_pattern(failures: &[&TestRun]) -> Option<FailurePattern> {
    if failures.len() < 5 {
        return None;
    }

    let mut hour_counts = [0u32; 24];
    for run in failures {
        let hour = usize::try_from(run.timestamp.hour()).unwrap_or(0);
        if hour < 24 {
            hour_counts[hour] += 1;
        }
    }

    let total = u32::try_from(failures.len()).unwrap_or(1);
    let expected = f64::from(total) / 24.0;
    let max_count = hour_counts.iter().max().copied().unwrap_or(0);

    let concentration = f64::from(max_count) / expected;
    if concentration < 3.0 {
        return None;
    }

    let peak_hour = hour_counts
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| *c)
        .map_or(0, |(h, _)| h);

    Some(FailurePattern {
        pattern_type: PatternType::TimeOfDay,
        occurrences: max_count,
        correlation: (concentration - 1.0).min(1.0),
        examples: vec![format!(
            "failures concentrated around hour {peak_hour}:00 UTC"
        )],
    })
}

fn detect_environmental_pattern(runs: &[TestRun]) -> Option<FailurePattern> {
    let ci_runs: Vec<&TestRun> = runs.iter().filter(|r| r.environment.is_ci).collect();
    let local_runs: Vec<&TestRun> = runs.iter().filter(|r| !r.environment.is_ci).collect();

    if ci_runs.len() < 3 || local_runs.len() < 3 {
        return None;
    }

    let ci_fail_rate = failure_rate_of(&ci_runs);
    let local_fail_rate = failure_rate_of(&local_runs);
    let diff = (ci_fail_rate - local_fail_rate).abs();

    if diff < 0.15 {
        return None;
    }

    let higher_env = if ci_fail_rate > local_fail_rate {
        "CI"
    } else {
        "local"
    };

    Some(FailurePattern {
        pattern_type: PatternType::Environmental,
        occurrences: u32::try_from(runs.len()).unwrap_or(u32::MAX),
        correlation: diff.min(1.0),
        examples: vec![format!(
            "failure rate {:.0}% higher in {higher_env} (CI: {:.1}%, local: {:.1}%)",
            diff * 100.0,
            ci_fail_rate * 100.0,
            local_fail_rate * 100.0,
        )],
    })
}

fn failure_rate_of(runs: &[&TestRun]) -> f64 {
    super::failure_rate(runs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_run_at_hour, test_run_in_env};
    use crate::types::TestRun;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case(10, 0, &[], None)]
    #[case(8, 0, &[(3, true), (15, true)], Some(PatternType::Random))]
    #[case(20, 10, &[], Some(PatternType::TimeOfDay))]
    fn pattern_detection_scenarios(
        #[case] passed_count: u32,
        #[case] concentrated_failures_at_h3: u32,
        #[case] extra_failures: &[(u32, bool)],
        #[case] expected: Option<PatternType>,
    ) {
        let mut runs: Vec<TestRun> = (0..passed_count)
            .map(|h| test_run_at_hour("test::a", TestOutcome::Passed, h % 24))
            .collect();
        for _ in 0..concentrated_failures_at_h3 {
            runs.push(test_run_at_hour("test::a", TestOutcome::Failed, 3));
        }
        for &(hour, _) in extra_failures {
            runs.push(test_run_at_hour("test::a", TestOutcome::Failed, hour));
        }
        let patterns = detect_patterns(&runs);
        match expected {
            None => assert!(patterns.is_empty()),
            Some(pt) => assert!(patterns.iter().any(|p| p.pattern_type == pt)),
        }
    }

    #[rstest]
    #[case(0.8, 0.1, true)]
    #[case(0.1, 0.8, true)]
    #[case(0.3, 0.3, false)]
    fn environmental_pattern_detection(
        #[case] ci_fail_rate: f64,
        #[case] local_fail_rate: f64,
        #[case] expect_pattern: bool,
    ) {
        let mut runs = Vec::new();

        for i in 0..20 {
            let fail = f64::from(i) / 20.0 < ci_fail_rate;
            let outcome = if fail {
                TestOutcome::Failed
            } else {
                TestOutcome::Passed
            };
            runs.push(test_run_in_env("test::a", outcome, true));
        }
        for i in 0..20 {
            let fail = f64::from(i) / 20.0 < local_fail_rate;
            let outcome = if fail {
                TestOutcome::Failed
            } else {
                TestOutcome::Passed
            };
            runs.push(test_run_in_env("test::a", outcome, false));
        }

        let patterns = detect_patterns(&runs);
        let env_pattern = patterns
            .iter()
            .any(|p| p.pattern_type == PatternType::Environmental);
        assert_eq!(env_pattern, expect_pattern);
    }

    proptest! {
        #[test]
        fn detect_patterns_never_panics(
            num_passed in 0u32..50,
            num_failed in 0u32..50,
        ) {
            let mut runs = Vec::new();
            for h in 0..num_passed {
                runs.push(test_run_at_hour("test::prop", TestOutcome::Passed, h % 24));
            }
            for h in 0..num_failed {
                runs.push(test_run_at_hour("test::prop", TestOutcome::Failed, h % 24));
            }
            let patterns = detect_patterns(&runs);
            for p in &patterns {
                prop_assert!(p.correlation >= 0.0);
                prop_assert!(p.correlation <= 1.0);
            }
        }
    }
}
