use chrono::Utc;
use statrs::distribution::{Beta, ContinuousCDF};

use crate::types::{BayesianParams, FlakinessScore, TestName, TestOutcome, TestRun};

pub struct BayesianDetector {
    prior_alpha: f64,
    prior_beta: f64,
    confidence_threshold: f64,
}

impl BayesianDetector {
    #[must_use]
    pub const fn new(confidence_threshold: f64) -> Self {
        Self {
            prior_alpha: 1.0,
            prior_beta: 1.0,
            confidence_threshold,
        }
    }

    #[must_use]
    pub fn calculate_flakiness_score(&self, test_name: &str, runs: &[TestRun]) -> FlakinessScore {
        let (passes, failures) = count_outcomes(runs);

        let alpha = self.prior_alpha + f64::from(failures);
        let beta = self.prior_beta + f64::from(passes);

        let total = alpha + beta;
        let posterior_mean = alpha / total;
        let posterior_variance = (alpha * beta) / (total.powi(2) * (total + 1.0));

        let (ci_lower, ci_upper) = credible_interval(alpha, beta);

        let total_runs = u64::from(passes + failures);
        let total_runs_f = f64::from(passes + failures);
        let pass_rate = if total_runs > 0 {
            f64::from(passes) / total_runs_f
        } else {
            0.0
        };

        let consecutive_failures = count_consecutive_trailing_failures(runs);

        FlakinessScore {
            test_name: TestName::from(test_name),
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

    #[must_use]
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
    Beta::new(alpha, beta_param).map_or((0.0, 1.0), |dist| {
        let lower = dist.inverse_cdf(0.025);
        let upper = dist.inverse_cdf(0.975);
        (lower, upper)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_runs_by_outcome;
    use proptest::prelude::*;

    use rstest::rstest;

    fn make_runs(passes: u32, failures: u32) -> Vec<TestRun> {
        test_runs_by_outcome(passes, failures)
    }

    #[rstest]
    #[case(50, 0, 0.0, 0.05)]
    #[case(25, 25, 0.3, 1.0)]
    #[case(0, 50, 0.9, 1.0)]
    #[case(0, 0, 0.49, 0.51)]
    fn flakiness_boundary_cases(
        #[case] passes: u32,
        #[case] failures: u32,
        #[case] min_prob: f64,
        #[case] max_prob: f64,
    ) {
        let detector = BayesianDetector::new(0.95);
        let runs = make_runs(passes, failures);
        let score = detector.calculate_flakiness_score("tests::boundary", &runs);
        assert!(
            score.probability_flaky >= min_prob && score.probability_flaky <= max_prob,
            "p={} not in [{min_prob}, {max_prob}]",
            score.probability_flaky
        );
    }

    #[test]
    fn consecutive_failures_tracked() {
        let runs = make_runs(5, 3);
        let count = count_consecutive_trailing_failures(&runs);
        assert_eq!(count, 3);
    }

    #[test]
    fn is_flaky_respects_threshold() {
        let detector = BayesianDetector::new(0.95);
        let runs = make_runs(10, 10);
        let score = detector.calculate_flakiness_score("tests::maybe", &runs);
        let flaky = detector.is_flaky(&score);
        assert!(flaky == (score.confidence >= 0.95 && score.probability_flaky > 0.01));
    }

    proptest! {
        #[test]
        fn score_probability_in_unit_range(
            passes in 0u32..100,
            failures in 0u32..100,
        ) {
            let detector = BayesianDetector::new(0.95);
            let runs = make_runs(passes, failures);
            let score = detector.calculate_flakiness_score("tests::prop", &runs);
            prop_assert!(score.probability_flaky >= 0.0);
            prop_assert!(score.probability_flaky <= 1.0);
            prop_assert!(score.confidence >= 0.0);
            prop_assert!(score.confidence <= 1.0);
        }

        #[test]
        fn more_failures_means_higher_flakiness(
            passes in 10u32..50,
            extra_failures in 1u32..20,
        ) {
            let detector = BayesianDetector::new(0.95);
            let low = detector.calculate_flakiness_score("tests::low", &make_runs(passes, 1));
            let high = detector.calculate_flakiness_score("tests::high", &make_runs(passes, extra_failures + 1));
            prop_assert!(high.probability_flaky >= low.probability_flaky);
        }

        #[test]
        fn credible_interval_is_valid(
            passes in 1u32..100,
            failures in 1u32..100,
        ) {
            let detector = BayesianDetector::new(0.95);
            let score = detector.calculate_flakiness_score("tests::ci", &make_runs(passes, failures));
            prop_assert!(score.bayesian_params.credible_interval_lower >= 0.0);
            prop_assert!(score.bayesian_params.credible_interval_upper <= 1.0);
            prop_assert!(score.bayesian_params.credible_interval_lower <= score.bayesian_params.credible_interval_upper);
        }
    }
}
