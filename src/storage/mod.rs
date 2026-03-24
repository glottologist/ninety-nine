pub mod backend;
pub(crate) mod mapping;
pub mod postgres;
pub mod schema;
pub mod sqlite;

pub use backend::StorageBackend;
pub use postgres::PostgresStorage;
pub use sqlite::SqliteStorage;

use std::time::Duration;

use uuid::Uuid;

use crate::config::model::Config;
use crate::error::NinetyNineError;
use crate::types::{FlakinessScore, QuarantineEntry, RunSession, TestRun};

/// Trait for test result storage backends.
pub trait Storage: Send + Sync {
    /// Stores a new run session.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the session cannot be written.
    async fn store_session(&self, session: &RunSession) -> Result<(), NinetyNineError>;

    /// Marks a session as finished with final counts.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the session cannot be updated.
    async fn finish_session(
        &self,
        session_id: &Uuid,
        test_count: u32,
        flaky_count: u32,
    ) -> Result<(), NinetyNineError>;

    /// Stores a single test run result.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the run cannot be written.
    async fn store_test_run(&self, run: &TestRun, session_id: &Uuid)
    -> Result<(), NinetyNineError>;

    /// Stores or updates a flakiness score.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the score cannot be written.
    async fn store_flakiness_score(&self, score: &FlakinessScore) -> Result<(), NinetyNineError>;

    /// Retrieves test runs for a given test name, most recent first.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the query fails.
    async fn get_test_runs(
        &self,
        test_name: &str,
        limit: u32,
    ) -> Result<Vec<TestRun>, NinetyNineError>;

    /// Retrieves recent run sessions, most recent first.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the query fails.
    async fn get_recent_sessions(&self, limit: u32) -> Result<Vec<RunSession>, NinetyNineError>;

    /// Retrieves all flakiness scores, ordered by probability descending.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the query fails.
    async fn get_all_scores(&self) -> Result<Vec<FlakinessScore>, NinetyNineError>;

    /// Retrieves the flakiness score for a specific test.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the query fails.
    async fn get_score(&self, test_name: &str) -> Result<Option<FlakinessScore>, NinetyNineError>;

    /// Adds a test to quarantine.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the quarantine entry cannot be written.
    async fn quarantine_test(
        &self,
        test_name: &str,
        reason: &str,
        score: f64,
        auto: bool,
    ) -> Result<(), NinetyNineError>;

    /// Removes a test from quarantine.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the entry cannot be deleted.
    async fn unquarantine_test(&self, test_name: &str) -> Result<(), NinetyNineError>;

    /// Lists all quarantined tests.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the query fails.
    async fn get_quarantined_tests(&self) -> Result<Vec<QuarantineEntry>, NinetyNineError>;

    /// Checks whether a test is quarantined.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the query fails.
    async fn is_quarantined(&self, test_name: &str) -> Result<bool, NinetyNineError>;

    /// Retrieves test runs belonging to a specific session.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the query fails.
    async fn get_session_runs(&self, session_id: &Uuid) -> Result<Vec<TestRun>, NinetyNineError>;

    /// Deletes test runs older than the specified number of days.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the delete fails.
    async fn purge_older_than(&self, days: u32) -> Result<u64, NinetyNineError>;
}

/// Opens the configured storage backend.
///
/// # Errors
///
/// Returns an error if the storage backend cannot be initialized.
pub async fn open_storage(config: &Config) -> Result<StorageBackend, NinetyNineError> {
    use crate::config::model::StorageBackendType;

    match config.storage.backend {
        StorageBackendType::Sqlite => {
            let db_path = config.storage.sqlite.as_ref().map_or_else(
                || {
                    dirs::data_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join("ninety-nine")
                        .join("ninety-nine.db")
                },
                |s| s.database_path.clone(), // clone: extracting path from config for storage init
            );
            let storage = SqliteStorage::open(&db_path)?;
            Ok(StorageBackend::Sqlite(storage))
        }
        StorageBackendType::Postgres => {
            let pg_config =
                config
                    .storage
                    .postgres
                    .as_ref()
                    .ok_or_else(|| NinetyNineError::InvalidConfig {
                        message:
                            "postgres backend selected but no [storage.postgres] config provided"
                                .to_string(),
                    })?;
            let storage =
                PostgresStorage::connect(&pg_config.connection_string, pg_config.pool_size).await?;
            Ok(StorageBackend::Postgres(storage))
        }
    }
}

pub(crate) fn parse_timestamp(s: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map_or_else(|_| chrono::Utc::now(), |dt| dt.with_timezone(&chrono::Utc))
}

pub(crate) fn duration_to_ms(d: Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

pub(crate) fn ms_to_duration(ms: i64) -> Duration {
    Duration::from_millis(u64::try_from(ms).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::{Config, StorageBackendType};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn duration_roundtrip(ms in 0i64..i64::MAX / 2) {
            let duration = ms_to_duration(ms);
            let back = duration_to_ms(duration);
            prop_assert_eq!(back, ms);
        }

        #[test]
        fn parse_timestamp_always_returns_valid_datetime(s in ".*") {
            let dt = parse_timestamp(&s);
            assert!(dt.timestamp() > 0);
        }
    }

    #[tokio::test]
    async fn open_storage_postgres_without_config_errors() {
        let mut config = Config::default();
        config.storage.backend = StorageBackendType::Postgres;
        config.storage.postgres = None;
        let result = open_storage(&config).await;
        assert!(result.is_err());
    }

    #[test]
    fn parse_timestamp_valid_rfc3339() {
        use chrono::Datelike;
        let dt = parse_timestamp("2026-01-01T00:00:00Z");
        assert_eq!(dt.year(), 2026);
    }
}
