pub mod duration;
pub mod pattern;
pub mod trend;

use crate::types::{TestOutcome, TestRun};

pub(crate) fn failure_rate(runs: &[&TestRun]) -> f64 {
    if runs.is_empty() {
        return 0.0;
    }
    let failures = runs
        .iter()
        .filter(|r| {
            matches!(
                r.outcome,
                TestOutcome::Failed | TestOutcome::Panic | TestOutcome::Timeout
            )
        })
        .count();

    f64::from(u32::try_from(failures).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(runs.len()).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_run;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn failure_rate_in_unit_range(
            passes in 0u32..50,
            failures in 0u32..50,
        ) {
            prop_assume!(passes + failures > 0);
            let mut runs = Vec::new();
            for _ in 0..passes {
                runs.push(test_run("test::prop", TestOutcome::Passed));
            }
            for _ in 0..failures {
                runs.push(test_run("test::prop", TestOutcome::Failed));
            }
            let refs: Vec<&TestRun> = runs.iter().collect();
            let rate = failure_rate(&refs);
            prop_assert!(rate >= 0.0);
            prop_assert!(rate <= 1.0);
        }
    }

    #[test]
    fn failure_rate_empty_is_zero() {
        let refs: Vec<&TestRun> = vec![];
        assert!((failure_rate(&refs) - 0.0).abs() < f64::EPSILON);
    }
}
