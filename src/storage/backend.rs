use uuid::Uuid;

use crate::error::NinetyNineError;
use crate::storage::Storage;
use crate::storage::postgres::PostgresStorage;
use crate::storage::sqlite::SqliteStorage;
use crate::types::{FlakinessScore, QuarantineEntry, RunSession, TestRun};

pub enum StorageBackend {
    Sqlite(SqliteStorage),
    Postgres(PostgresStorage),
}

macro_rules! dispatch {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            Self::Sqlite(s) => s.$method($($arg),*).await,
            Self::Postgres(s) => s.$method($($arg),*).await,
        }
    };
}

impl Storage for StorageBackend {
    async fn store_session(&self, session: &RunSession) -> Result<(), NinetyNineError> {
        dispatch!(self, store_session, session)
    }

    async fn finish_session(
        &self,
        session_id: &Uuid,
        test_count: u32,
        flaky_count: u32,
    ) -> Result<(), NinetyNineError> {
        dispatch!(self, finish_session, session_id, test_count, flaky_count)
    }

    async fn store_test_run(
        &self,
        run: &TestRun,
        session_id: &Uuid,
    ) -> Result<(), NinetyNineError> {
        dispatch!(self, store_test_run, run, session_id)
    }

    async fn store_flakiness_score(&self, score: &FlakinessScore) -> Result<(), NinetyNineError> {
        dispatch!(self, store_flakiness_score, score)
    }

    async fn get_test_runs(
        &self,
        test_name: &str,
        limit: u32,
    ) -> Result<Vec<TestRun>, NinetyNineError> {
        dispatch!(self, get_test_runs, test_name, limit)
    }

    async fn get_recent_sessions(&self, limit: u32) -> Result<Vec<RunSession>, NinetyNineError> {
        dispatch!(self, get_recent_sessions, limit)
    }

    async fn get_all_scores(&self) -> Result<Vec<FlakinessScore>, NinetyNineError> {
        dispatch!(self, get_all_scores)
    }

    async fn get_score(&self, test_name: &str) -> Result<Option<FlakinessScore>, NinetyNineError> {
        dispatch!(self, get_score, test_name)
    }

    async fn quarantine_test(
        &self,
        test_name: &str,
        reason: &str,
        score: f64,
        auto: bool,
    ) -> Result<(), NinetyNineError> {
        dispatch!(self, quarantine_test, test_name, reason, score, auto)
    }

    async fn unquarantine_test(&self, test_name: &str) -> Result<(), NinetyNineError> {
        dispatch!(self, unquarantine_test, test_name)
    }

    async fn get_quarantined_tests(&self) -> Result<Vec<QuarantineEntry>, NinetyNineError> {
        dispatch!(self, get_quarantined_tests)
    }

    async fn is_quarantined(&self, test_name: &str) -> Result<bool, NinetyNineError> {
        dispatch!(self, is_quarantined, test_name)
    }

    async fn get_session_runs(&self, session_id: &Uuid) -> Result<Vec<TestRun>, NinetyNineError> {
        dispatch!(self, get_session_runs, session_id)
    }

    async fn purge_older_than(&self, days: u32) -> Result<u64, NinetyNineError> {
        dispatch!(self, purge_older_than, days)
    }
}
