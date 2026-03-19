use crate::types::TestRun;
use crate::types::test_name::TestName;
use crate::types::trend::{TrendDirection, TrendSummary};

const TREND_THRESHOLD: f64 = 0.05;

#[must_use]
pub fn calculate_trend(test_name: &str, runs: &[TestRun], window: u32) -> Option<TrendSummary> {
    let window_size = usize::try_from(window).unwrap_or(usize::MAX);
    let relevant: Vec<&TestRun> = runs
        .iter()
        .filter(|r| r.test_name == test_name)
        .take(window_size)
        .collect();

    if relevant.len() < 4 {
        return None;
    }

    let midpoint = relevant.len() / 2;
    let recent = &relevant[..midpoint];
    let previous = &relevant[midpoint..];

    let recent_rate = failure_rate(recent);
    let previous_rate = failure_rate(previous);
    let delta = recent_rate - previous_rate;

    let direction = if delta > TREND_THRESHOLD {
        TrendDirection::Degrading
    } else if delta < -TREND_THRESHOLD {
        TrendDirection::Improving
    } else {
        TrendDirection::Stable
    };

    Some(TrendSummary {
        test_name: TestName::from(test_name),
        direction,
        recent_score: recent_rate,
        previous_score: previous_rate,
        score_delta: delta,
        window_runs: u64::try_from(relevant.len()).unwrap_or(0),
    })
}

fn failure_rate(runs: &[&TestRun]) -> f64 {
    super::failure_rate(runs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_run;
    use crate::types::TestOutcome;
    use proptest::prelude::*;
    use rstest::rstest;

    fn make_runs_with_outcomes(name: &str, outcomes: &[TestOutcome]) -> Vec<TestRun> {
        outcomes.iter().map(|o| test_run(name, *o)).collect()
    }

    #[test]
    fn too_few_runs_returns_none() {
        let runs = make_runs_with_outcomes("test::a", &[TestOutcome::Passed, TestOutcome::Passed]);
        assert!(calculate_trend("test::a", &runs, 100).is_none());
    }

    #[rstest]
    #[case(
        &[TestOutcome::Failed, TestOutcome::Failed, TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed],
        TrendDirection::Degrading
    )]
    #[case(
        &[TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Failed, TestOutcome::Failed, TestOutcome::Failed, TestOutcome::Failed],
        TrendDirection::Improving
    )]
    #[case(
        &[TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed],
        TrendDirection::Stable
    )]
    fn trend_direction_from_outcomes(
        #[case] outcomes: &[TestOutcome],
        #[case] expected: TrendDirection,
    ) {
        let runs = make_runs_with_outcomes("test::trend", outcomes);
        let trend = calculate_trend("test::trend", &runs, 100).unwrap();
        assert_eq!(trend.direction, expected);
    }

    proptest! {
        #[test]
        fn trend_delta_matches_scores(
            recent_failures in 0u32..50,
            recent_passes in 0u32..50,
            prev_failures in 0u32..50,
            prev_passes in 0u32..50,
        ) {
            let recent_total = recent_failures + recent_passes;
            let prev_total = prev_failures + prev_passes;

            prop_assume!(recent_total >= 2 && prev_total >= 2);

            let mut runs = Vec::new();
            for _ in 0..recent_failures {
                runs.push(test_run("test::prop", TestOutcome::Failed));
            }
            for _ in 0..recent_passes {
                runs.push(test_run("test::prop", TestOutcome::Passed));
            }
            for _ in 0..prev_failures {
                runs.push(test_run("test::prop", TestOutcome::Failed));
            }
            for _ in 0..prev_passes {
                runs.push(test_run("test::prop", TestOutcome::Passed));
            }

            let total = runs.len();
            prop_assume!(total >= 4);

            if let Some(trend) = calculate_trend("test::prop", &runs, u32::try_from(total).unwrap_or(u32::MAX)) {
                let expected_delta = trend.recent_score - trend.previous_score;
                prop_assert!((trend.score_delta - expected_delta).abs() < f64::EPSILON);
            }
        }
    }
}
