use serde::{Deserialize, Serialize};

use super::test_run::{TestOutcome, TestRun};

/// How a test's recorded runs (including retry attempts) classify it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestVerdict {
    Passed,
    Flaky,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OutcomeCounts {
    pub passed: u32,
    pub failed: u32,
    pub ignored: u32,
}

impl OutcomeCounts {
    #[must_use]
    pub fn from_runs(runs: &[TestRun]) -> Self {
        let mut counts = Self::default();
        for run in runs {
            match run.outcome {
                TestOutcome::Passed => counts.passed += 1,
                TestOutcome::Failed | TestOutcome::Panic | TestOutcome::Timeout => {
                    counts.failed += 1;
                }
                TestOutcome::Ignored => counts.ignored += 1,
            }
        }
        counts
    }

    /// Any recorded failure alongside a pass marks the test flaky, even when
    /// every iteration eventually passed on retry — that recovery is exactly
    /// the flaky signal.
    #[must_use]
    pub const fn verdict(self) -> TestVerdict {
        match (self.passed > 0, self.failed > 0) {
            (true, true) => TestVerdict::Flaky,
            (true, false) => TestVerdict::Passed,
            (false, true) => TestVerdict::Failed,
            (false, false) => TestVerdict::Skipped,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    pub pattern_type: PatternType,
    pub occurrences: u32,
    pub correlation: f64,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternType {
    TimeOfDay,
    Environmental,
    Random,
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimeOfDay => write!(f, "time-of-day"),
            Self::Environmental => write!(f, "environmental"),
            Self::Random => write!(f, "random"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_run;
    use rstest::rstest;

    /// The flaky arm is the H-3 regression case: an iteration that failed
    /// twice and then passed on retry must classify as flaky, not passed.
    #[rstest]
    #[case(&[TestOutcome::Passed, TestOutcome::Passed], TestVerdict::Passed)]
    #[case(&[TestOutcome::Failed, TestOutcome::Failed, TestOutcome::Passed], TestVerdict::Flaky)]
    #[case(&[TestOutcome::Failed, TestOutcome::Timeout], TestVerdict::Failed)]
    #[case(&[TestOutcome::Panic, TestOutcome::Passed], TestVerdict::Flaky)]
    #[case(&[TestOutcome::Ignored, TestOutcome::Ignored], TestVerdict::Skipped)]
    #[case(&[], TestVerdict::Skipped)]
    #[case(&[TestOutcome::Ignored, TestOutcome::Passed], TestVerdict::Passed)]
    fn verdict_from_recorded_outcomes(
        #[case] outcomes: &[TestOutcome],
        #[case] expected: TestVerdict,
    ) {
        let runs: Vec<TestRun> = outcomes.iter().map(|o| test_run("t", *o)).collect();
        assert_eq!(OutcomeCounts::from_runs(&runs).verdict(), expected);
    }
}
