use crate::config::model::{QuarantineByClass, QuarantineConfig};
use crate::error::NinetyNineError;
use crate::storage::{Storage, StorageBackend};
use crate::types::{DiagnosticResult, FlakeClass};

/// Whether a diagnose class is eligible for auto-quarantine under `by`.
#[must_use]
pub fn should_quarantine_class(class: FlakeClass, by: &QuarantineByClass) -> bool {
    match class {
        FlakeClass::Intrinsic => by.intrinsic,
        FlakeClass::Contention => by.contention,
        FlakeClass::Broken => by.broken,
    }
}

/// Auto-quarantines diagnose results according to config.
///
/// Uses the short test name (last segment of identity) for quarantine keys so
/// they stay consistent with Bayesian score names from isolation.
///
/// # Errors
///
/// Returns storage errors when quarantine writes fail.
pub async fn auto_quarantine_by_class(
    config: &QuarantineConfig,
    storage: &StorageBackend,
    results: &[DiagnosticResult],
) -> Result<(), NinetyNineError> {
    if !config.enabled || !config.auto_quarantine {
        return Ok(());
    }

    for result in results {
        if !should_quarantine_class(result.class, &config.by_class) {
            continue;
        }
        let name = result.test_id.test_name.as_ref();
        if storage.is_quarantined(name).await? {
            continue;
        }
        let reason = format!("auto:{}", result.class.as_str());
        storage.quarantine_test(name, &reason, 0.0, true).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PhaseCounts, RecordOutcome, TestId};

    #[test]
    fn should_auto_quarantine_intrinsic_when_enabled() {
        let cfg = QuarantineByClass {
            intrinsic: true,
            contention: false,
            broken: true,
        };
        assert!(should_quarantine_class(FlakeClass::Intrinsic, &cfg));
    }

    #[test]
    fn should_not_auto_quarantine_contention_by_default() {
        assert!(!should_quarantine_class(
            FlakeClass::Contention,
            &QuarantineByClass::default()
        ));
    }

    #[tokio::test]
    async fn auto_quarantine_writes_reason_with_class() {
        let storage = crate::storage::SqliteStorage::in_memory().unwrap();
        let storage = StorageBackend::Sqlite(storage);
        let config = QuarantineConfig {
            enabled: true,
            auto_quarantine: true,
            by_class: QuarantineByClass::default(),
            ..QuarantineConfig::default()
        };
        let results = vec![DiagnosticResult {
            test_id: TestId::new("pkg", "bin", "tests::intrinsic"),
            class: FlakeClass::Intrinsic,
            counts: PhaseCounts {
                stress_runs: 3,
                stress_failures: 1,
                isolation_runs: 10,
                isolation_failures: 3,
            },
            recording: RecordOutcome::SkippedNoRequest,
        }];
        auto_quarantine_by_class(&config, &storage, &results)
            .await
            .unwrap();
        assert!(storage.is_quarantined("tests::intrinsic").await.unwrap());
        let entries = storage.get_quarantined_tests().await.unwrap();
        assert!(entries[0].reason.contains("intrinsic"));
    }

    #[tokio::test]
    async fn auto_quarantine_skips_contention_by_default() {
        let storage = crate::storage::SqliteStorage::in_memory().unwrap();
        let storage = StorageBackend::Sqlite(storage);
        let config = QuarantineConfig {
            enabled: true,
            auto_quarantine: true,
            ..QuarantineConfig::default()
        };
        let results = vec![DiagnosticResult {
            test_id: TestId::new("pkg", "bin", "tests::racy"),
            class: FlakeClass::Contention,
            counts: PhaseCounts {
                stress_runs: 3,
                stress_failures: 2,
                isolation_runs: 10,
                isolation_failures: 0,
            },
            recording: RecordOutcome::SkippedNoRequest,
        }];
        auto_quarantine_by_class(&config, &storage, &results)
            .await
            .unwrap();
        assert!(!storage.is_quarantined("tests::racy").await.unwrap());
    }
}
