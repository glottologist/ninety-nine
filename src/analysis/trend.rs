use crate::types::TestRun;
use crate::types::test_name::TestName;
use crate::types::trend::{TrendDirection, TrendSummary};

const TREND_THRESHOLD: f64 = 0.05;

#[must_use]
pub fn calculate_trend(test_name: &str, runs: &[TestRun], window: u32) -> Option<TrendSummary> {
    let window_size = usize::try_from(window).unwrap_or(usize::MAX);
    let mut relevant: Vec<&TestRun> = runs.iter().filter(|r| r.test_name == test_name).collect();
    // Callers pass runs in whichever order they hold them (storage queries are
    // newest-first, in-memory session results oldest-first); the recent/previous
    // split below is only correct for newest-first, so order here.
    relevant.sort_by_key(|r| std::cmp::Reverse(r.timestamp));
    relevant.truncate(window_size);

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
    use crate::test_helpers::{test_run, test_run_at_hour};
    use crate::types::TestOutcome;
    use proptest::prelude::*;
    use rstest::rstest;

    /// Builds runs in chronological order: index i is stamped i hours after
    /// the base, so the last element is the newest.
    fn chronological_runs(name: &str, outcomes: &[TestOutcome]) -> Vec<TestRun> {
        outcomes
            .iter()
            .enumerate()
            .map(|(i, o)| test_run_at_hour(name, *o, u32::try_from(i).unwrap_or(0)))
            .collect()
    }

    #[test]
    fn too_few_runs_returns_none() {
        let runs = chronological_runs("test::a", &[TestOutcome::Passed, TestOutcome::Passed]);
        assert!(calculate_trend("test::a", &runs, 100).is_none());
    }

    #[rstest]
    #[case(
        &[TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Failed, TestOutcome::Failed, TestOutcome::Failed],
        TrendDirection::Degrading
    )]
    #[case(
        &[TestOutcome::Failed, TestOutcome::Failed, TestOutcome::Failed, TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed],
        TrendDirection::Improving
    )]
    #[case(
        &[TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed, TestOutcome::Passed],
        TrendDirection::Stable
    )]
    fn trend_direction_from_chronological_outcomes(
        #[case] outcomes: &[TestOutcome],
        #[case] expected: TrendDirection,
    ) {
        let runs = chronological_runs("test::trend", outcomes);
        let trend = calculate_trend("test::trend", &runs, 100).unwrap();
        assert_eq!(trend.direction, expected);
    }

    /// Regression test for the H-1 inversion: the direction must not depend
    /// on the order in which callers happen to hold the runs.
    #[rstest]
    #[case(false)]
    #[case(true)]
    fn trend_direction_is_order_independent(#[case] reversed: bool) {
        let outcomes = [
            TestOutcome::Passed,
            TestOutcome::Passed,
            TestOutcome::Passed,
            TestOutcome::Failed,
            TestOutcome::Failed,
            TestOutcome::Failed,
        ];
        let mut runs = chronological_runs("test::order", &outcomes);
        if reversed {
            runs.reverse();
        }
        let trend = calculate_trend("test::order", &runs, 100).unwrap();
        assert_eq!(trend.direction, TrendDirection::Degrading);
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
