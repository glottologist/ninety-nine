pub mod schema;
pub mod sqlite;

pub use sqlite::SqliteStorage;

use uuid::Uuid;

use crate::error::NinetyNineError;
use crate::types::{FlakinessScore, QuarantineEntry, RunSession, TestRun};

pub trait Storage {
    fn store_session(&self, session: &RunSession) -> Result<(), NinetyNineError>;
    fn finish_session(
        &self,
        session_id: &Uuid,
        test_count: u32,
        flaky_count: u32,
    ) -> Result<(), NinetyNineError>;
    fn store_test_run(&self, run: &TestRun, session_id: &Uuid) -> Result<(), NinetyNineError>;
    fn store_flakiness_score(&self, score: &FlakinessScore) -> Result<(), NinetyNineError>;

    fn get_test_runs(&self, test_name: &str, limit: u32) -> Result<Vec<TestRun>, NinetyNineError>;
    fn get_recent_sessions(&self, limit: u32) -> Result<Vec<RunSession>, NinetyNineError>;
    fn get_all_scores(&self) -> Result<Vec<FlakinessScore>, NinetyNineError>;
    fn get_score(&self, test_name: &str) -> Result<Option<FlakinessScore>, NinetyNineError>;

    fn quarantine_test(
        &self,
        test_name: &str,
        reason: &str,
        score: f64,
        auto: bool,
    ) -> Result<(), NinetyNineError>;
    fn unquarantine_test(&self, test_name: &str) -> Result<(), NinetyNineError>;
    fn get_quarantined_tests(&self) -> Result<Vec<QuarantineEntry>, NinetyNineError>;
    fn is_quarantined(&self, test_name: &str) -> Result<bool, NinetyNineError>;

    fn purge_older_than(&self, days: u32) -> Result<u64, NinetyNineError>;
}
