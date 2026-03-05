use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

impl RunSession {
    pub fn start(commit_hash: &str, branch: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            started_at: Utc::now(),
            finished_at: None,
            test_count: 0,
            flaky_count: 0,
            commit_hash: commit_hash.to_string(),
            branch: branch.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineEntry {
    pub test_name: String,
    pub quarantined_at: DateTime<Utc>,
    pub reason: String,
    pub flakiness_score: f64,
    pub auto_quarantined: bool,
}
