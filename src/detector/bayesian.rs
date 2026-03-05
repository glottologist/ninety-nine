use chrono::Utc;
use statrs::distribution::{Beta, ContinuousCDF};

use crate::types::{BayesianParams, FlakinessScore, TestOutcome, TestRun};

pub struct BayesianDetector {
    prior_alpha: f64,
    prior_beta: f64,
    confidence_threshold: f64,
}

impl BayesianDetector {
    pub fn new(confidence_threshold: f64) -> Self {
        Self {
            prior_alpha: 1.0,
            prior_beta: 1.0,
            confidence_threshold,
        }
    }

    pub fn calculate_flakiness_score(&self, test_name: &str, runs: &[TestRun]) -> FlakinessScore {
        let (passes, failures) = count_outcomes(runs);

        let alpha = self.prior_alpha + f64::from(failures);
        let beta = self.prior_beta + f64::from(passes);

        let total = alpha + beta;
        let posterior_mean = alpha / total;
        let posterior_variance = (alpha * beta) / (total * total * (total + 1.0));

        let (ci_lower, ci_upper) = credible_interval(alpha, beta);

        let total_runs = u64::from(passes + failures);
        let pass_rate = if total_runs > 0 {
            f64::from(passes) / total_runs as f64
        } else {
            0.0
        };

        let consecutive_failures = count_consecutive_trailing_failures(runs);

        FlakinessScore {
            test_name: test_name.to_string(),
            probability_flaky: posterior_mean,
            confidence: 1.0 - (ci_upper - ci_lower),
            pass_rate,
            fail_rate: 1.0 - pass_rate,
            total_runs,
            consecutive_failures,
            last_updated: Utc::now(),
            bayesian_params: BayesianParams {
                alpha,
                beta,
                posterior_mean,
                posterior_variance,
                credible_interval_lower: ci_lower,
                credible_interval_upper: ci_upper,
            },
        }
    }

    pub fn is_flaky(&self, score: &FlakinessScore) -> bool {
        score.probability_flaky > 0.01 && score.confidence >= self.confidence_threshold
    }
}

fn count_outcomes(runs: &[TestRun]) -> (u32, u32) {
    let mut passes = 0u32;
    let mut failures = 0u32;

    for run in runs {
        match run.outcome {
            TestOutcome::Passed => passes += 1,
            TestOutcome::Failed | TestOutcome::Panic | TestOutcome::Timeout => failures += 1,
            TestOutcome::Ignored => {}
        }
    }

    (passes, failures)
}

fn count_consecutive_trailing_failures(runs: &[TestRun]) -> u32 {
    let mut count = 0u32;

    for run in runs.iter().rev() {
        match run.outcome {
            TestOutcome::Failed | TestOutcome::Panic | TestOutcome::Timeout => count += 1,
            TestOutcome::Passed => break,
            TestOutcome::Ignored => {}
        }
    }

    count
}

fn credible_interval(alpha: f64, beta_param: f64) -> (f64, f64) {
    if let Ok(dist) = Beta::new(alpha, beta_param) {
        let lower = dist.inverse_cdf(0.025);
        let upper = dist.inverse_cdf(0.975);
        (lower, upper)
    } else {
        (0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TestEnvironment, TestRun};
    use chrono::Utc;
    use proptest::prelude::*;
    use std::path::PathBuf;
    use std::time::Duration;
    use uuid::Uuid;

    fn make_run(outcome: TestOutcome) -> TestRun {
        TestRun {
            id: Uuid::new_v4(),
            test_name: "test::example".to_string(),
            test_path: PathBuf::new(),
            outcome,
            duration: Duration::from_millis(100),
            timestamp: Utc::now(),
            commit_hash: String::new(),
            branch: String::new(),
            environment: TestEnvironment {
                os: "linux".to_string(),
                rust_version: "1.85.0".to_string(),
                cpu_count: 8,
                memory_gb: 16.0,
                is_ci: false,
                ci_provider: None,
            },
            retry_count: 0,
            error_message: None,
            stack_trace: None,
        }
    }

    fn make_runs(passes: u32, failures: u32) -> Vec<TestRun> {
        let mut runs = Vec::new();
        for _ in 0..passes {
            runs.push(make_run(TestOutcome::Passed));
        }
        for _ in 0..failures {
            runs.push(make_run(TestOutcome::Failed));
        }
        runs
    }

    #[test]
    fn all_passes_yields_low_flakiness() {
        let detector = BayesianDetector::new(0.95);
        let runs = make_runs(100, 0);
        let score = detector.calculate_flakiness_score("test::stable", &runs);

        assert!(score.probability_flaky < 0.05);
        assert_eq!(score.total_runs, 100);
    }

    #[test]
    fn mixed_results_yield_higher_flakiness() {
        let detector = BayesianDetector::new(0.95);
        let runs = make_runs(50, 50);
        let score = detector.calculate_flakiness_score("test::flaky", &runs);

        assert!(score.probability_flaky > 0.3);
    }

    #[test]
    fn all_failures_yields_high_flakiness() {
        let detector = BayesianDetector::new(0.95);
        let runs = make_runs(0, 100);
        let score = detector.calculate_flakiness_score("test::broken", &runs);

        assert!(score.probability_flaky > 0.9);
    }

    #[test]
    fn empty_runs_uses_prior() {
        let detector = BayesianDetector::new(0.95);
        let score = detector.calculate_flakiness_score("test::empty", &[]);

        assert!((score.probability_flaky - 0.5).abs() < 0.01);
        assert_eq!(score.total_runs, 0);
    }

    #[test]
    fn consecutive_failures_counted_from_tail() {
        let mut runs = make_runs(5, 0);
        runs.extend(make_runs(0, 3));

        let detector = BayesianDetector::new(0.95);
        let score = detector.calculate_flakiness_score("test::tail", &runs);

        assert_eq!(score.consecutive_failures, 3);
    }

    proptest! {
        #[test]
        fn probability_always_between_zero_and_one(
            passes in 0u32..200,
            failures in 0u32..200,
        ) {
            let detector = BayesianDetector::new(0.95);
            let runs = make_runs(passes, failures);
            let score = detector.calculate_flakiness_score("test::prop", &runs);

            prop_assert!(score.probability_flaky >= 0.0);
            prop_assert!(score.probability_flaky <= 1.0);
            prop_assert!(score.confidence >= 0.0);
            prop_assert!(score.confidence <= 1.0);
            prop_assert!(score.pass_rate >= 0.0);
            prop_assert!(score.pass_rate <= 1.0);
        }

        #[test]
        fn more_failures_means_higher_flakiness(
            passes in 10u32..100,
        ) {
            let detector = BayesianDetector::new(0.95);
            let low_fail = make_runs(passes, 1);
            let high_fail = make_runs(passes, passes);

            let low_score = detector.calculate_flakiness_score("low", &low_fail);
            let high_score = detector.calculate_flakiness_score("high", &high_fail);

            prop_assert!(high_score.probability_flaky > low_score.probability_flaky);
        }

        #[test]
        fn credible_interval_is_valid(
            alpha in 0.1f64..100.0,
            beta_param in 0.1f64..100.0,
        ) {
            let (lower, upper) = credible_interval(alpha, beta_param);
            prop_assert!(lower <= upper);
            prop_assert!(lower >= 0.0);
            prop_assert!(upper <= 1.0);
        }
    }
}
