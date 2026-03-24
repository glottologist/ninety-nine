use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::error::NinetyNineError;
use crate::storage::mapping::{RawScoreRow, RawTestRunRow};
use crate::storage::{Storage, duration_to_ms, schema};
use crate::types::{FlakinessScore, QuarantineEntry, RunSession, TestName, TestRun};

pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

impl SqliteStorage {
    /// Opens a `SQLite` database at the given path, creating it if needed.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the database cannot be opened or migrated.
    pub fn open(path: &Path) -> Result<Self, NinetyNineError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Creates an in-memory `SQLite` database for testing.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the database cannot be created.
    pub fn in_memory() -> Result<Self, NinetyNineError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, NinetyNineError> {
        self.conn
            .lock()
            .map_err(|e| NinetyNineError::InvalidConfig {
                message: format!("mutex poisoned: {e}"),
            })
    }
}

fn migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    let current_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    for (i, migration) in schema::MIGRATIONS.iter().enumerate() {
        let version = i64::try_from(i).unwrap_or(0) + 1;
        if version > current_version {
            conn.execute_batch(migration)?;
            conn.pragma_update(None, "user_version", version)?;
        }
    }

    Ok(())
}

fn parse_timestamp(s: &str) -> DateTime<Utc> {
    super::parse_timestamp(s)
}

impl Storage for SqliteStorage {
    async fn store_session(&self, session: &RunSession) -> Result<(), NinetyNineError> {
        let finished = session.finished_at.map(|dt| dt.to_rfc3339());
        let guard = self.lock_conn()?;
        guard.execute(
            "INSERT INTO run_sessions (id, started_at, finished_at, test_count, flaky_count, commit_hash, branch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session.id.to_string(),
                session.started_at.to_rfc3339(),
                finished,
                session.test_count,
                session.flaky_count,
                session.commit_hash,
                session.branch,
            ],
        )?;
        drop(guard);
        Ok(())
    }

    async fn finish_session(
        &self,
        session_id: &Uuid,
        test_count: u32,
        flaky_count: u32,
    ) -> Result<(), NinetyNineError> {
        let guard = self.lock_conn()?;
        guard.execute(
            "UPDATE run_sessions SET finished_at = ?1, test_count = ?2, flaky_count = ?3 WHERE id = ?4",
            params![
                Utc::now().to_rfc3339(),
                test_count,
                flaky_count,
                session_id.to_string(),
            ],
        )?;
        drop(guard);
        Ok(())
    }

    async fn store_test_run(
        &self,
        run: &TestRun,
        session_id: &Uuid,
    ) -> Result<(), NinetyNineError> {
        let guard = self.lock_conn()?;
        guard.execute(
            "INSERT INTO test_runs (id, session_id, test_name, test_path, outcome, duration_ms,
             timestamp, commit_hash, branch, retry_count, error_message, stack_trace,
             env_os, env_rust_version, env_cpu_count, env_memory_gb, env_is_ci, env_ci_provider)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                run.id.to_string(),
                session_id.to_string(),
                run.test_name.as_ref(),
                run.test_path.to_string_lossy(),
                run.outcome.to_string(),
                duration_to_ms(run.duration),
                run.timestamp.to_rfc3339(),
                run.commit_hash,
                run.branch,
                run.retry_count,
                run.error_message,
                run.stack_trace,
                run.environment.os,
                run.environment.rust_version,
                run.environment.cpu_count,
                run.environment.memory_gb,
                run.environment.is_ci,
                run.environment.ci_provider,
            ],
        )?;
        drop(guard);
        Ok(())
    }

    async fn store_flakiness_score(&self, score: &FlakinessScore) -> Result<(), NinetyNineError> {
        let guard = self.lock_conn()?;
        guard.execute(
            "INSERT OR REPLACE INTO flakiness_scores
             (test_name, probability_flaky, confidence, pass_rate, fail_rate, total_runs,
              consecutive_failures, last_updated, alpha, beta, posterior_mean, posterior_variance,
              ci_lower, ci_upper)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                score.test_name.as_ref(),
                score.probability_flaky,
                score.confidence,
                score.pass_rate,
                score.fail_rate,
                score.total_runs,
                score.consecutive_failures,
                score.last_updated.to_rfc3339(),
                score.bayesian_params.alpha,
                score.bayesian_params.beta,
                score.bayesian_params.posterior_mean,
                score.bayesian_params.posterior_variance,
                score.bayesian_params.credible_interval_lower,
                score.bayesian_params.credible_interval_upper,
            ],
        )?;
        drop(guard);
        Ok(())
    }

    async fn get_test_runs(
        &self,
        test_name: &str,
        limit: u32,
    ) -> Result<Vec<TestRun>, NinetyNineError> {
        let runs = {
            let guard = self.lock_conn()?;
            let mut stmt = guard.prepare(
                "SELECT id, test_name, test_path, outcome, duration_ms, timestamp,
                        commit_hash, branch, retry_count, error_message, stack_trace,
                        env_os, env_rust_version, env_cpu_count, env_memory_gb, env_is_ci, env_ci_provider
                 FROM test_runs WHERE test_name = ?1
                 ORDER BY timestamp DESC LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![test_name, limit], |row| {
                Ok(extract_test_run_row(row))
            })?;

            let mut runs = Vec::new();
            for row_result in rows {
                runs.push(row_result??.into_test_run());
            }
            runs
        };
        Ok(runs)
    }

    async fn get_recent_sessions(&self, limit: u32) -> Result<Vec<RunSession>, NinetyNineError> {
        let sessions = {
            let guard = self.lock_conn()?;
            let mut stmt = guard.prepare(
                "SELECT id, started_at, finished_at, test_count, flaky_count, commit_hash, branch
                 FROM run_sessions ORDER BY started_at DESC LIMIT ?1",
            )?;

            let rows = stmt.query_map(params![limit], |row| {
                let id_str: String = row.get(0)?;
                let started_str: String = row.get(1)?;
                let finished_str: Option<String> = row.get(2)?;
                let test_count: u32 = row.get(3)?;
                let flaky_count: u32 = row.get(4)?;
                let commit_hash: String = row.get(5)?;
                let branch: String = row.get(6)?;

                Ok(RunSession {
                    id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    started_at: parse_timestamp(&started_str),
                    finished_at: finished_str.map(|s| parse_timestamp(&s)),
                    test_count,
                    flaky_count,
                    commit_hash,
                    branch,
                })
            })?;

            let mut sessions = Vec::new();
            for row_result in rows {
                sessions.push(row_result?);
            }
            sessions
        };
        Ok(sessions)
    }

    async fn get_all_scores(&self) -> Result<Vec<FlakinessScore>, NinetyNineError> {
        let scores = {
            let guard = self.lock_conn()?;
            let mut stmt = guard.prepare(
                "SELECT test_name, probability_flaky, confidence, pass_rate, fail_rate, total_runs,
                        consecutive_failures, last_updated, alpha, beta, posterior_mean,
                        posterior_variance, ci_lower, ci_upper
                 FROM flakiness_scores ORDER BY probability_flaky DESC",
            )?;

            let rows = stmt.query_map([], |row| Ok(extract_score_row(row)))?;

            let mut scores = Vec::new();
            for row_result in rows {
                scores.push(row_result??.into_score());
            }
            scores
        };
        Ok(scores)
    }

    async fn get_score(&self, test_name: &str) -> Result<Option<FlakinessScore>, NinetyNineError> {
        let result = {
            let guard = self.lock_conn()?;
            let mut stmt = guard.prepare(
                "SELECT test_name, probability_flaky, confidence, pass_rate, fail_rate, total_runs,
                        consecutive_failures, last_updated, alpha, beta, posterior_mean,
                        posterior_variance, ci_lower, ci_upper
                 FROM flakiness_scores WHERE test_name = ?1",
            )?;

            let mut rows = stmt.query_map(params![test_name], |row| Ok(extract_score_row(row)))?;

            match rows.next() {
                Some(row_result) => Some(row_result??.into_score()),
                None => None,
            }
        };
        Ok(result)
    }

    async fn quarantine_test(
        &self,
        test_name: &str,
        reason: &str,
        score: f64,
        auto: bool,
    ) -> Result<(), NinetyNineError> {
        let guard = self.lock_conn()?;
        guard.execute(
            "INSERT OR REPLACE INTO quarantine (test_name, quarantined_at, reason, flakiness_score, auto_quarantined)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                test_name,
                Utc::now().to_rfc3339(),
                reason,
                score,
                auto,
            ],
        )?;
        drop(guard);
        Ok(())
    }

    async fn unquarantine_test(&self, test_name: &str) -> Result<(), NinetyNineError> {
        let guard = self.lock_conn()?;
        guard.execute(
            "DELETE FROM quarantine WHERE test_name = ?1",
            params![test_name],
        )?;
        drop(guard);
        Ok(())
    }

    async fn get_quarantined_tests(&self) -> Result<Vec<QuarantineEntry>, NinetyNineError> {
        let entries = {
            let guard = self.lock_conn()?;
            let mut stmt = guard.prepare(
                "SELECT test_name, quarantined_at, reason, flakiness_score, auto_quarantined
                 FROM quarantine ORDER BY quarantined_at DESC",
            )?;

            let rows = stmt.query_map([], |row| {
                let test_name: String = row.get(0)?;
                let quarantined_str: String = row.get(1)?;
                let reason: String = row.get(2)?;
                let flakiness_score: f64 = row.get(3)?;
                let auto_quarantined: bool = row.get(4)?;

                Ok(QuarantineEntry {
                    test_name: TestName::from(test_name),
                    quarantined_at: parse_timestamp(&quarantined_str),
                    reason,
                    flakiness_score,
                    auto_quarantined,
                })
            })?;

            let mut entries = Vec::new();
            for row_result in rows {
                entries.push(row_result?);
            }
            entries
        };
        Ok(entries)
    }

    async fn is_quarantined(&self, test_name: &str) -> Result<bool, NinetyNineError> {
        let count: i64 = self.lock_conn()?.query_row(
            "SELECT COUNT(*) FROM quarantine WHERE test_name = ?1",
            params![test_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    async fn get_session_runs(&self, session_id: &Uuid) -> Result<Vec<TestRun>, NinetyNineError> {
        let runs = {
            let guard = self.lock_conn()?;
            let mut stmt = guard.prepare(
                "SELECT id, test_name, test_path, outcome, duration_ms, timestamp,
                        commit_hash, branch, retry_count, error_message, stack_trace,
                        env_os, env_rust_version, env_cpu_count, env_memory_gb, env_is_ci, env_ci_provider
                 FROM test_runs WHERE session_id = ?1
                 ORDER BY test_name ASC",
            )?;

            let rows = stmt.query_map(params![session_id.to_string()], |row| {
                Ok(extract_test_run_row(row))
            })?;

            let mut runs = Vec::new();
            for row_result in rows {
                runs.push(row_result??.into_test_run());
            }
            runs
        };
        Ok(runs)
    }

    async fn purge_older_than(&self, days: u32) -> Result<u64, NinetyNineError> {
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(days));
        let cutoff_str = cutoff.to_rfc3339();

        let guard = self.lock_conn()?;
        guard.execute(
            "DELETE FROM test_runs WHERE timestamp < ?1",
            params![cutoff_str],
        )?;

        Ok(guard.changes())
    }
}

fn extract_test_run_row(row: &rusqlite::Row<'_>) -> Result<RawTestRunRow, NinetyNineError> {
    Ok(RawTestRunRow {
        id: row.get(0)?,
        test_name: row.get(1)?,
        test_path: row.get(2)?,
        outcome: row.get(3)?,
        duration_ms: row.get(4)?,
        timestamp: row.get(5)?,
        commit_hash: row.get(6)?,
        branch: row.get(7)?,
        retry_count: row.get(8)?,
        error_message: row.get(9)?,
        stack_trace: row.get(10)?,
        env_os: row.get(11)?,
        env_rust_version: row.get(12)?,
        env_cpu_count: row.get(13)?,
        env_memory_gb: row.get(14)?,
        env_is_ci: row.get(15)?,
        env_ci_provider: row.get(16)?,
    })
}

fn extract_score_row(row: &rusqlite::Row<'_>) -> Result<RawScoreRow, NinetyNineError> {
    Ok(RawScoreRow {
        test_name: row.get(0)?,
        probability_flaky: row.get(1)?,
        confidence: row.get(2)?,
        pass_rate: row.get(3)?,
        fail_rate: row.get(4)?,
        total_runs: row.get(5)?,
        consecutive_failures: row.get(6)?,
        last_updated: row.get(7)?,
        alpha: row.get(8)?,
        beta: row.get(9)?,
        posterior_mean: row.get(10)?,
        posterior_variance: row.get(11)?,
        ci_lower: row.get(12)?,
        ci_upper: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_run_for_storage, test_score, test_session};
    use crate::types::TestOutcome;
    use proptest::prelude::*;
    use rstest::rstest;
    use std::time::Duration;

    fn make_test_run(name: &str, outcome: TestOutcome) -> TestRun {
        test_run_for_storage(name, outcome)
    }

    fn make_score(name: &str, probability: f64) -> FlakinessScore {
        test_score(name, probability)
    }

    fn setup() -> SqliteStorage {
        SqliteStorage::in_memory().unwrap()
    }

    #[tokio::test]
    async fn migration_sets_schema_version() {
        let storage = setup();
        let version: i64 = {
            let guard = storage.conn.lock().unwrap();
            guard
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .unwrap()
        };
        assert_eq!(
            version,
            i64::try_from(schema::MIGRATIONS.len()).unwrap_or(0)
        );
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let storage = setup();
        let result = {
            let guard = storage.conn.lock().unwrap();
            migrate(&guard)
        };
        assert!(result.is_ok());
    }

    #[rstest]
    #[case(TestOutcome::Passed)]
    #[case(TestOutcome::Failed)]
    #[case(TestOutcome::Timeout)]
    #[case(TestOutcome::Panic)]
    #[tokio::test]
    async fn store_and_retrieve_test_run(#[case] outcome: TestOutcome) {
        let storage = setup();
        let session = test_session("abc123", "main");
        storage.store_session(&session).await.unwrap();

        let run = make_test_run("tests::example", outcome);
        storage.store_test_run(&run, &session.id).await.unwrap();

        let retrieved = storage.get_test_runs("tests::example", 10).await.unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].test_name, "tests::example");
        assert_eq!(retrieved[0].outcome, outcome);
        assert_eq!(retrieved[0].retry_count, 0);
        assert_eq!(retrieved[0].duration, Duration::from_millis(42));
    }

    #[tokio::test]
    async fn score_upsert_keeps_latest() {
        let storage = setup();

        let score1 = make_score("tests::flaky", 0.1);
        storage.store_flakiness_score(&score1).await.unwrap();

        let score2 = make_score("tests::flaky", 0.5);
        storage.store_flakiness_score(&score2).await.unwrap();

        let retrieved = storage.get_score("tests::flaky").await.unwrap().unwrap();
        assert!((retrieved.probability_flaky - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn get_score_returns_none_for_unknown() {
        let storage = setup();
        let result = storage.get_score("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_all_scores_ordered_by_probability() {
        let storage = setup();
        storage
            .store_flakiness_score(&make_score("low", 0.01))
            .await
            .unwrap();
        storage
            .store_flakiness_score(&make_score("high", 0.9))
            .await
            .unwrap();
        storage
            .store_flakiness_score(&make_score("mid", 0.3))
            .await
            .unwrap();

        let scores = storage.get_all_scores().await.unwrap();
        assert_eq!(scores.len(), 3);
        assert_eq!(scores[0].test_name, "high");
        assert_eq!(scores[1].test_name, "mid");
        assert_eq!(scores[2].test_name, "low");
    }

    #[tokio::test]
    async fn quarantine_lifecycle() {
        let storage = setup();

        assert!(!storage.is_quarantined("tests::flaky").await.unwrap());

        storage
            .quarantine_test("tests::flaky", "too flaky", 0.5, false)
            .await
            .unwrap();
        assert!(storage.is_quarantined("tests::flaky").await.unwrap());

        let entries = storage.get_quarantined_tests().await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].test_name, "tests::flaky");
        assert_eq!(entries[0].reason, "too flaky");
        assert!(!entries[0].auto_quarantined);

        storage.unquarantine_test("tests::flaky").await.unwrap();
        assert!(!storage.is_quarantined("tests::flaky").await.unwrap());
        assert!(storage.get_quarantined_tests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn session_lifecycle() {
        let storage = setup();
        let session = test_session("def456", "feature-branch");
        storage.store_session(&session).await.unwrap();
        storage.finish_session(&session.id, 10, 3).await.unwrap();

        let sessions = storage.get_recent_sessions(5).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].test_count, 10);
        assert_eq!(sessions[0].flaky_count, 3);
        assert!(sessions[0].finished_at.is_some());
        assert_eq!(sessions[0].branch, "feature-branch");
    }

    #[tokio::test]
    async fn purge_removes_old_runs() {
        let storage = setup();
        let session = test_session("abc", "main");
        storage.store_session(&session).await.unwrap();

        let mut old_run = make_test_run("tests::old", TestOutcome::Passed);
        old_run.timestamp = Utc::now() - chrono::Duration::days(100);
        storage.store_test_run(&old_run, &session.id).await.unwrap();

        let recent_run = make_test_run("tests::new", TestOutcome::Passed);
        storage
            .store_test_run(&recent_run, &session.id)
            .await
            .unwrap();

        let purged = storage.purge_older_than(30).await.unwrap();
        assert_eq!(purged, 1);

        let remaining = storage.get_test_runs("tests::old", 10).await.unwrap();
        assert!(remaining.is_empty());

        let kept = storage.get_test_runs("tests::new", 10).await.unwrap();
        assert_eq!(kept.len(), 1);
    }

    #[tokio::test]
    async fn get_test_runs_respects_limit() {
        let storage = setup();
        let session = test_session("abc", "main");
        storage.store_session(&session).await.unwrap();

        for _ in 0..10 {
            let run = make_test_run("tests::many", TestOutcome::Passed);
            storage.store_test_run(&run, &session.id).await.unwrap();
        }

        let limited = storage.get_test_runs("tests::many", 3).await.unwrap();
        assert_eq!(limited.len(), 3);
    }

    fn arb_outcome() -> impl Strategy<Value = TestOutcome> {
        prop_oneof![
            Just(TestOutcome::Passed),
            Just(TestOutcome::Failed),
            Just(TestOutcome::Ignored),
            Just(TestOutcome::Timeout),
            Just(TestOutcome::Panic),
        ]
    }

    proptest! {
        #[test]
        fn store_retrieve_preserves_outcome(outcome in arb_outcome()) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let storage = setup();
            let session = test_session("abc", "main");
            rt.block_on(storage.store_session(&session)).unwrap();

            let run = make_test_run("tests::prop", outcome);
            rt.block_on(storage.store_test_run(&run, &session.id)).unwrap();

            let retrieved = rt.block_on(storage.get_test_runs("tests::prop", 1)).unwrap();
            prop_assert_eq!(retrieved.len(), 1);
            prop_assert_eq!(retrieved[0].outcome, outcome);
        }

        #[test]
        fn store_n_runs_retrieve_correct_count(n in 1u32..20) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let storage = setup();
            let session = test_session("abc", "main");
            rt.block_on(storage.store_session(&session)).unwrap();

            for _ in 0..n {
                let run = make_test_run("tests::count", TestOutcome::Passed);
                rt.block_on(storage.store_test_run(&run, &session.id)).unwrap();
            }

            let retrieved = rt.block_on(storage.get_test_runs("tests::count", 100)).unwrap();
            prop_assert_eq!(u32::try_from(retrieved.len()).unwrap_or(0), n);
        }
    }
}
