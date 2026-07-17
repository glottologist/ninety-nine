use crate::types::{FlakeClass, PhaseCounts};

/// Classifies a test from stress and isolation phase counts.
///
/// Returns `None` when the test never failed under stress (or isolation was not run).
#[must_use]
pub fn classify(counts: &PhaseCounts) -> Option<FlakeClass> {
    if counts.stress_failures == 0 || counts.isolation_runs == 0 {
        return None;
    }
    if counts.isolation_failures == 0 {
        return Some(FlakeClass::Contention);
    }
    if counts.isolation_failures >= counts.isolation_runs {
        return Some(FlakeClass::Broken);
    }
    Some(FlakeClass::Intrinsic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn classify_contention_when_stress_failed_isolation_clean() {
        let c = PhaseCounts {
            stress_runs: 3,
            stress_failures: 1,
            isolation_runs: 10,
            isolation_failures: 0,
        };
        assert_eq!(classify(&c), Some(FlakeClass::Contention));
    }

    #[test]
    fn classify_intrinsic_mixed_isolation() {
        let c = PhaseCounts {
            stress_runs: 3,
            stress_failures: 1,
            isolation_runs: 10,
            isolation_failures: 3,
        };
        assert_eq!(classify(&c), Some(FlakeClass::Intrinsic));
    }

    #[test]
    fn classify_broken_all_isolation_fail() {
        let c = PhaseCounts {
            stress_runs: 2,
            stress_failures: 2,
            isolation_runs: 5,
            isolation_failures: 5,
        };
        assert_eq!(classify(&c), Some(FlakeClass::Broken));
    }

    #[test]
    fn classify_none_when_no_stress_failures() {
        let c = PhaseCounts {
            stress_runs: 3,
            stress_failures: 0,
            isolation_runs: 0,
            isolation_failures: 0,
        };
        assert_eq!(classify(&c), None);
    }

    proptest! {
        #[test]
        fn zero_stress_failures_never_classifies(
            stress_runs in 0u32..50,
            isolation_runs in 0u32..50,
            isolation_failures in 0u32..50,
        ) {
            let c = PhaseCounts {
                stress_runs,
                stress_failures: 0,
                isolation_runs,
                isolation_failures: isolation_failures.min(isolation_runs),
            };
            prop_assert_eq!(classify(&c), None);
        }
    }
}
