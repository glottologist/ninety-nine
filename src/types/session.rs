use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::test_name::TestName;

/// A session returned from storage queries. May be running or finished.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSession {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub test_count: u32,
    pub flaky_count: u32,
    pub commit_hash: String,
    pub branch: String,
}

/// A session that is actively running. Created via `start()`, consumed by
/// `into_run_session()` to produce a storable `RunSession`.
/// Once converted or dropped, it cannot be reused — preventing double-finish
/// at the orchestration layer.
#[derive(Debug)]
pub struct ActiveSession {
    id: Uuid,
    started_at: DateTime<Utc>,
    commit_hash: String,
    branch: String,
}

impl ActiveSession {
    #[must_use]
    pub fn start(commit_hash: &str, branch: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            started_at: Utc::now(),
            commit_hash: commit_hash.to_string(),
            branch: branch.to_string(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> &Uuid {
        &self.id
    }

    /// Converts to a `RunSession` suitable for storage.
    #[must_use]
    pub fn into_run_session(self) -> RunSession {
        RunSession {
            id: self.id,
            started_at: self.started_at,
            finished_at: None,
            test_count: 0,
            flaky_count: 0,
            commit_hash: self.commit_hash,
            branch: self.branch,
        }
    }

    /// Creates a storable `RunSession` without consuming self.
    #[must_use]
    pub fn to_run_session(&self) -> RunSession {
        RunSession {
            id: self.id,
            started_at: self.started_at,
            finished_at: None,
            test_count: 0,
            flaky_count: 0,
            commit_hash: self.commit_hash.clone(), // clone: needed to create owned RunSession
            branch: self.branch.clone(),           // clone: needed to create owned RunSession
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineEntry {
    pub test_name: TestName,
    pub quarantined_at: DateTime<Utc>,
    pub reason: String,
    pub flakiness_score: f64,
    pub auto_quarantined: bool,
}
