use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::error::NinetyNineError;
use crate::storage::Storage;
use crate::storage::schema;
use crate::types::{
    BayesianParams, FlakinessScore, QuarantineEntry, RunSession, TestEnvironment, TestOutcome,
    TestRun,
};

pub struct SqliteStorage {
    conn: Connection,
}

impl SqliteStorage {
    pub fn open(path: &Path) -> Result<Self, NinetyNineError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        migrate(&conn)?;
        Ok(Self { conn })
    }

    pub fn in_memory() -> Result<Self, NinetyNineError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        migrate(&conn)?;
        Ok(Self { conn })
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
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn duration_to_ms(d: Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

fn ms_to_duration(ms: i64) -> Duration {
    Duration::from_millis(u64::try_from(ms).unwrap_or(0))
}

impl Storage for SqliteStorage {
    fn store_session(&self, session: &RunSession) -> Result<(), NinetyNineError> {
        let finished = session.finished_at.map(|dt| dt.to_rfc3339());
        self.conn.execute(
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
        Ok(())
    }

    fn finish_session(
        &self,
        session_id: &Uuid,
        test_count: u32,
        flaky_count: u32,
    ) -> Result<(), NinetyNineError> {
        self.conn.execute(
            "UPDATE run_sessions SET finished_at = ?1, test_count = ?2, flaky_count = ?3 WHERE id = ?4",
            params![
                Utc::now().to_rfc3339(),
                test_count,
                flaky_count,
                session_id.to_string(),
            ],
        )?;
        Ok(())
    }

    fn store_test_run(&self, run: &TestRun, session_id: &Uuid) -> Result<(), NinetyNineError> {
        self.conn.execute(
            "INSERT INTO test_runs (id, session_id, test_name, test_path, outcome, duration_ms,
             timestamp, commit_hash, branch, retry_count, error_message, stack_trace,
             env_os, env_rust_version, env_cpu_count, env_memory_gb, env_is_ci, env_ci_provider)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                run.id.to_string(),
                session_id.to_string(),
                run.test_name,
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
        Ok(())
    }

    fn store_flakiness_score(&self, score: &FlakinessScore) -> Result<(), NinetyNineError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO flakiness_scores
             (test_name, probability_flaky, confidence, pass_rate, fail_rate, total_runs,
              consecutive_failures, last_updated, alpha, beta, posterior_mean, posterior_variance,
              ci_lower, ci_upper)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                score.test_name,
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
        Ok(())
    }

    fn get_test_runs(&self, test_name: &str, limit: u32) -> Result<Vec<TestRun>, NinetyNineError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, test_name, test_path, outcome, duration_ms, timestamp,
                    commit_hash, branch, retry_count, error_message, stack_trace,
                    env_os, env_rust_version, env_cpu_count, env_memory_gb, env_is_ci, env_ci_provider
             FROM test_runs WHERE test_name = ?1
             ORDER BY timestamp DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![test_name, limit], |row| Ok(read_test_run_row(row)))?;

        let mut runs = Vec::new();
        for row_result in rows {
            runs.push(row_result??);
        }
        Ok(runs)
    }

    fn get_recent_sessions(&self, limit: u32) -> Result<Vec<RunSession>, NinetyNineError> {
        let mut stmt = self.conn.prepare(
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
        Ok(sessions)
    }

    fn get_all_scores(&self) -> Result<Vec<FlakinessScore>, NinetyNineError> {
        let mut stmt = self.conn.prepare(
            "SELECT test_name, probability_flaky, confidence, pass_rate, fail_rate, total_runs,
                    consecutive_failures, last_updated, alpha, beta, posterior_mean,
                    posterior_variance, ci_lower, ci_upper
             FROM flakiness_scores ORDER BY probability_flaky DESC",
        )?;

        let rows = stmt.query_map([], |row| Ok(read_score_row(row)))?;

        let mut scores = Vec::new();
        for row_result in rows {
            scores.push(row_result??);
        }
        Ok(scores)
    }

    fn get_score(&self, test_name: &str) -> Result<Option<FlakinessScore>, NinetyNineError> {
        let mut stmt = self.conn.prepare(
            "SELECT test_name, probability_flaky, confidence, pass_rate, fail_rate, total_runs,
                    consecutive_failures, last_updated, alpha, beta, posterior_mean,
                    posterior_variance, ci_lower, ci_upper
             FROM flakiness_scores WHERE test_name = ?1",
        )?;

        let mut rows = stmt.query_map(params![test_name], |row| Ok(read_score_row(row)))?;

        match rows.next() {
            Some(row_result) => Ok(Some(row_result??)),
            None => Ok(None),
        }
    }

    fn quarantine_test(
        &self,
        test_name: &str,
        reason: &str,
        score: f64,
        auto: bool,
    ) -> Result<(), NinetyNineError> {
        self.conn.execute(
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
        Ok(())
    }

    fn unquarantine_test(&self, test_name: &str) -> Result<(), NinetyNineError> {
        self.conn.execute(
            "DELETE FROM quarantine WHERE test_name = ?1",
            params![test_name],
        )?;
        Ok(())
    }

    fn get_quarantined_tests(&self) -> Result<Vec<QuarantineEntry>, NinetyNineError> {
        let mut stmt = self.conn.prepare(
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
                test_name,
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
        Ok(entries)
    }

    fn is_quarantined(&self, test_name: &str) -> Result<bool, NinetyNineError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM quarantine WHERE test_name = ?1",
            params![test_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn purge_older_than(&self, days: u32) -> Result<u64, NinetyNineError> {
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(days));
        let cutoff_str = cutoff.to_rfc3339();

        self.conn.execute(
            "DELETE FROM test_runs WHERE timestamp < ?1",
            params![cutoff_str],
        )?;

        Ok(self.conn.changes())
    }
}

fn read_test_run_row(row: &rusqlite::Row<'_>) -> Result<TestRun, NinetyNineError> {
    let id_str: String = row.get(0)?;
    let test_name: String = row.get(1)?;
    let test_path_str: String = row.get(2)?;
    let outcome_str: String = row.get(3)?;
    let duration_ms: i64 = row.get(4)?;
    let timestamp_str: String = row.get(5)?;
    let commit_hash: String = row.get(6)?;
    let branch: String = row.get(7)?;
    let retry_count: u32 = row.get(8)?;
    let error_message: Option<String> = row.get(9)?;
    let stack_trace: Option<String> = row.get(10)?;
    let env_os: String = row.get(11)?;
    let env_rust_version: String = row.get(12)?;
    let env_cpu_count: u32 = row.get(13)?;
    let env_memory_gb: f64 = row.get(14)?;
    let env_is_ci: bool = row.get(15)?;
    let env_ci_provider: Option<String> = row.get(16)?;

    Ok(TestRun {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
        test_name,
        test_path: PathBuf::from(test_path_str),
        outcome: TestOutcome::from_str(&outcome_str).unwrap_or(TestOutcome::Failed),
        duration: ms_to_duration(duration_ms),
        timestamp: parse_timestamp(&timestamp_str),
        commit_hash,
        branch,
        environment: TestEnvironment {
            os: env_os,
            rust_version: env_rust_version,
            cpu_count: env_cpu_count,
            memory_gb: env_memory_gb,
            is_ci: env_is_ci,
            ci_provider: env_ci_provider,
        },
        retry_count,
        error_message,
        stack_trace,
    })
}

fn read_score_row(row: &rusqlite::Row<'_>) -> Result<FlakinessScore, NinetyNineError> {
    let test_name: String = row.get(0)?;
    let probability_flaky: f64 = row.get(1)?;
    let confidence: f64 = row.get(2)?;
    let pass_rate: f64 = row.get(3)?;
    let fail_rate: f64 = row.get(4)?;
    let total_runs: u64 = row.get(5)?;
    let consecutive_failures: u32 = row.get(6)?;
    let last_updated_str: String = row.get(7)?;
    let alpha: f64 = row.get(8)?;
    let beta: f64 = row.get(9)?;
    let posterior_mean: f64 = row.get(10)?;
    let posterior_variance: f64 = row.get(11)?;
    let ci_lower: f64 = row.get(12)?;
    let ci_upper: f64 = row.get(13)?;

    Ok(FlakinessScore {
        test_name,
        probability_flaky,
        confidence,
        pass_rate,
        fail_rate,
        total_runs,
        consecutive_failures,
        last_updated: parse_timestamp(&last_updated_str),
        bayesian_params: BayesianParams {
            alpha,
            beta,
            posterior_mean,
            posterior_variance,
            credible_interval_lower: ci_lower,
            credible_interval_upper: ci_upper,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn make_environment() -> TestEnvironment {
        TestEnvironment {
            os: "linux".to_string(),
            rust_version: "1.85.0".to_string(),
            cpu_count: 8,
            memory_gb: 16.0,
            is_ci: false,
            ci_provider: None,
        }
    }

    fn make_test_run(name: &str, outcome: TestOutcome) -> TestRun {
        TestRun {
            id: Uuid::new_v4(),
            test_name: name.to_string(),
            test_path: PathBuf::from("/tmp/test-binary"),
            outcome,
            duration: Duration::from_millis(42),
            timestamp: Utc::now(),
            commit_hash: "abc123".to_string(),
            branch: "main".to_string(),
            environment: make_environment(),
            retry_count: 0,
            error_message: None,
            stack_trace: None,
        }
    }

    fn make_score(name: &str, probability: f64) -> FlakinessScore {
        FlakinessScore {
            test_name: name.to_string(),
            probability_flaky: probability,
            confidence: 0.95,
            pass_rate: 1.0 - probability,
            fail_rate: probability,
            total_runs: 100,
            consecutive_failures: 0,
            last_updated: Utc::now(),
            bayesian_params: BayesianParams {
                alpha: 2.0,
                beta: 98.0,
                posterior_mean: probability,
                posterior_variance: 0.001,
                credible_interval_lower: 0.01,
                credible_interval_upper: 0.05,
            },
        }
    }

    fn setup() -> SqliteStorage {
        SqliteStorage::in_memory().unwrap()
    }

    #[test]
    fn migration_sets_schema_version() {
        let storage = setup();
        let version: i64 = storage
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(
            version,
            i64::try_from(schema::MIGRATIONS.len()).unwrap_or(0)
        );
    }

    #[test]
    fn migration_is_idempotent() {
        let storage = setup();
        let result = migrate(&storage.conn);
        assert!(result.is_ok());
    }

    #[rstest]
    #[case(TestOutcome::Passed)]
    #[case(TestOutcome::Failed)]
    #[case(TestOutcome::Timeout)]
    #[case(TestOutcome::Panic)]
    fn store_and_retrieve_test_run(#[case] outcome: TestOutcome) {
        let storage = setup();
        let session = RunSession::start("abc123", "main");
        storage.store_session(&session).unwrap();

        let run = make_test_run("tests::example", outcome);
        storage.store_test_run(&run, &session.id).unwrap();

        let retrieved = storage.get_test_runs("tests::example", 10).unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].test_name, "tests::example");
        assert_eq!(retrieved[0].outcome, outcome);
        assert_eq!(retrieved[0].retry_count, 0);
        assert_eq!(retrieved[0].duration, Duration::from_millis(42));
    }

    #[test]
    fn score_upsert_keeps_latest() {
        let storage = setup();

        let score1 = make_score("tests::flaky", 0.1);
        storage.store_flakiness_score(&score1).unwrap();

        let score2 = make_score("tests::flaky", 0.5);
        storage.store_flakiness_score(&score2).unwrap();

        let retrieved = storage.get_score("tests::flaky").unwrap().unwrap();
        assert!((retrieved.probability_flaky - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn get_score_returns_none_for_unknown() {
        let storage = setup();
        let result = storage.get_score("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn get_all_scores_ordered_by_probability() {
        let storage = setup();
        storage
            .store_flakiness_score(&make_score("low", 0.01))
            .unwrap();
        storage
            .store_flakiness_score(&make_score("high", 0.9))
            .unwrap();
        storage
            .store_flakiness_score(&make_score("mid", 0.3))
            .unwrap();

        let scores = storage.get_all_scores().unwrap();
        assert_eq!(scores.len(), 3);
        assert_eq!(scores[0].test_name, "high");
        assert_eq!(scores[1].test_name, "mid");
        assert_eq!(scores[2].test_name, "low");
    }

    #[test]
    fn quarantine_lifecycle() {
        let storage = setup();

        assert!(!storage.is_quarantined("tests::flaky").unwrap());

        storage
            .quarantine_test("tests::flaky", "too flaky", 0.5, false)
            .unwrap();
        assert!(storage.is_quarantined("tests::flaky").unwrap());

        let entries = storage.get_quarantined_tests().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].test_name, "tests::flaky");
        assert_eq!(entries[0].reason, "too flaky");
        assert!(!entries[0].auto_quarantined);

        storage.unquarantine_test("tests::flaky").unwrap();
        assert!(!storage.is_quarantined("tests::flaky").unwrap());
        assert!(storage.get_quarantined_tests().unwrap().is_empty());
    }

    #[test]
    fn session_lifecycle() {
        let storage = setup();
        let session = RunSession::start("def456", "feature-branch");
        storage.store_session(&session).unwrap();
        storage.finish_session(&session.id, 10, 3).unwrap();

        let sessions = storage.get_recent_sessions(5).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].test_count, 10);
        assert_eq!(sessions[0].flaky_count, 3);
        assert!(sessions[0].finished_at.is_some());
        assert_eq!(sessions[0].branch, "feature-branch");
    }

    #[test]
    fn purge_removes_old_runs() {
        let storage = setup();
        let session = RunSession::start("abc", "main");
        storage.store_session(&session).unwrap();

        let mut old_run = make_test_run("tests::old", TestOutcome::Passed);
        old_run.timestamp = Utc::now() - chrono::Duration::days(100);
        storage.store_test_run(&old_run, &session.id).unwrap();

        let recent_run = make_test_run("tests::new", TestOutcome::Passed);
        storage.store_test_run(&recent_run, &session.id).unwrap();

        let purged = storage.purge_older_than(30).unwrap();
        assert_eq!(purged, 1);

        let remaining = storage.get_test_runs("tests::old", 10).unwrap();
        assert!(remaining.is_empty());

        let kept = storage.get_test_runs("tests::new", 10).unwrap();
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn get_test_runs_respects_limit() {
        let storage = setup();
        let session = RunSession::start("abc", "main");
        storage.store_session(&session).unwrap();

        for _ in 0..10 {
            let run = make_test_run("tests::many", TestOutcome::Passed);
            storage.store_test_run(&run, &session.id).unwrap();
        }

        let limited = storage.get_test_runs("tests::many", 3).unwrap();
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
            let storage = setup();
            let session = RunSession::start("abc", "main");
            storage.store_session(&session).unwrap();

            let run = make_test_run("tests::prop", outcome);
            storage.store_test_run(&run, &session.id).unwrap();

            let retrieved = storage.get_test_runs("tests::prop", 1).unwrap();
            prop_assert_eq!(retrieved.len(), 1);
            prop_assert_eq!(retrieved[0].outcome, outcome);
        }

        #[test]
        fn store_n_runs_retrieve_correct_count(n in 1u32..20) {
            let storage = setup();
            let session = RunSession::start("abc", "main");
            storage.store_session(&session).unwrap();

            for _ in 0..n {
                let run = make_test_run("tests::count", TestOutcome::Passed);
                storage.store_test_run(&run, &session.id).unwrap();
            }

            let retrieved = storage.get_test_runs("tests::count", 100).unwrap();
            prop_assert_eq!(u32::try_from(retrieved.len()).unwrap_or(0), n);
        }
    }
}
