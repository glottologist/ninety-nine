use crate::types::TestRun;
use crate::types::test_name::TestName;

#[derive(Debug, Clone)]
pub struct DurationRegression {
    pub test_name: TestName,
    pub current_ms: f64,
    pub mean_ms: f64,
    pub std_dev_ms: f64,
    pub deviation_factor: f64,
}

/// Detects tests whose recent duration significantly exceeds their historical mean.
///
/// Computes mean and standard deviation from historical runs only (excluding the
/// latest). When historical std\_dev is zero (all identical durations), a floor of
/// 1% of the mean is used so that large spikes are still detected.
///
/// Requires at least `min_history` total runs (1 latest + historical).
#[must_use]
pub fn detect_duration_regressions(
    test_name: &str,
    runs: &[TestRun],
    min_history: usize,
    threshold_std_devs: f64,
) -> Option<DurationRegression> {
    if runs.len() < min_history {
        return None;
    }

    let durations_ms: Vec<f64> = runs
        .iter()
        .map(|r| {
            let millis = u32::try_from(r.duration.as_millis()).unwrap_or(u32::MAX);
            f64::from(millis)
        })
        .collect();

    let latest = durations_ms[0];
    let historical = &durations_ms[1..];

    if historical.is_empty() {
        return None;
    }

    let hist_mean = mean(historical);
    let raw_std_dev = std_deviation(historical, hist_mean);
    let effective_std_dev = raw_std_dev.max(hist_mean * 0.01);

    if effective_std_dev < f64::EPSILON {
        return None;
    }

    let deviation = (latest - hist_mean) / effective_std_dev;

    if deviation > threshold_std_devs {
        Some(DurationRegression {
            test_name: TestName::from(test_name),
            current_ms: latest,
            mean_ms: hist_mean,
            std_dev_ms: raw_std_dev,
            deviation_factor: deviation,
        })
    } else {
        None
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let n = u32::try_from(values.len()).unwrap_or(u32::MAX);
    values.iter().sum::<f64>() / f64::from(n)
}

fn std_deviation(values: &[f64], mean: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let n = u32::try_from(values.len()).unwrap_or(u32::MAX);
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / f64::from(n - 1);
    variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_run_with_duration;
    use proptest::prelude::*;
    use rstest::rstest;

    fn make_run(duration_ms: u64) -> TestRun {
        test_run_with_duration("test::dur", duration_ms)
    }

    proptest! {
        #[test]
        fn mean_within_bounds(values in proptest::collection::vec(0.0f64..1000.0, 1..100)) {
            let m = mean(&values);
            let min_v = values.iter().copied().fold(f64::INFINITY, f64::min);
            let max_v = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            prop_assert!(m >= min_v - f64::EPSILON);
            prop_assert!(m <= max_v + f64::EPSILON);
        }

        #[test]
        fn std_deviation_non_negative(values in proptest::collection::vec(0.0f64..1000.0, 2..100)) {
            let m = mean(&values);
            prop_assert!(std_deviation(&values, m) >= 0.0);
        }

        #[test]
        fn regression_deviation_always_positive_when_returned(
            spike in 200u64..1000,
            baseline in 50u64..150,
            count in 5u32..20,
        ) {
            let mut runs = vec![make_run(spike)];
            for _ in 0..count {
                runs.push(make_run(baseline));
            }
            if let Some(reg) = detect_duration_regressions("test::dur", &runs, 5, 0.0) {
                prop_assert!(reg.deviation_factor >= 0.0);
                prop_assert!(reg.mean_ms >= 0.0);
                prop_assert!(reg.std_dev_ms >= 0.0);
            }
        }
    }

    #[test]
    fn mean_of_empty_is_zero() {
        assert!((mean(&[]) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn std_deviation_of_single_is_zero() {
        assert!((std_deviation(&[42.0], 42.0) - 0.0).abs() < f64::EPSILON);
    }

    #[rstest]
    #[case(500, &[100, 100, 100, 100, 100, 100, 100, 100, 100], 5, 2.0, true)]
    #[case(100, &[100, 100, 100, 100, 100, 100, 100, 100, 100], 5, 2.0, false)]
    #[case(100, &[100], 5, 2.0, false)]
    fn duration_regression_detection(
        #[case] latest_ms: u64,
        #[case] history_ms: &[u64],
        #[case] min_history: usize,
        #[case] threshold: f64,
        #[case] expect_some: bool,
    ) {
        let mut runs = vec![make_run(latest_ms)];
        for &ms in history_ms {
            runs.push(make_run(ms));
        }
        let result = detect_duration_regressions("test::dur", &runs, min_history, threshold);
        assert_eq!(result.is_some(), expect_some);
        if let Some(reg) = result {
            assert!(reg.deviation_factor > threshold);
        }
    }
}
