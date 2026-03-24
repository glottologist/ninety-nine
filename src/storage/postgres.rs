use chrono::{DateTime, Utc};
use deadpool_postgres::{Config as PgConfig, Pool, Runtime};
use tokio_postgres::NoTls;
use uuid::Uuid;

use crate::error::NinetyNineError;
use crate::storage::mapping::{RawScoreRow, RawTestRunRow};
use crate::storage::{Storage, duration_to_ms};
use crate::types::{FlakinessScore, QuarantineEntry, RunSession, TestName, TestRun};

pub struct PostgresStorage {
    pool: Pool,
}

impl PostgresStorage {
    /// Connects to a `PostgreSQL` database and runs migrations.
    ///
    /// # Errors
    ///
    /// Returns `PostgresPool` if the pool cannot be created,
    /// or `PostgresStorage` if the migration fails.
    pub async fn connect(connection_string: &str, pool_size: u32) -> Result<Self, NinetyNineError> {
        let mut cfg = PgConfig::new();
        cfg.url = Some(connection_string.to_string());
        let mut pool_cfg =
            deadpool_postgres::PoolConfig::new(usize::try_from(pool_size).unwrap_or(8));
        pool_cfg.timeouts = deadpool_postgres::Timeouts {
            wait: Some(std::time::Duration::from_secs(30)),
            create: Some(std::time::Duration::from_secs(10)),
            recycle: Some(std::time::Duration::from_secs(5)),
        };
        cfg.pool = Some(pool_cfg);

        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).map_err(|e| {
            NinetyNineError::PostgresPool {
                message: format!("failed to create pool: {e}"),
            }
        })?;

        let client = pool
            .get()
            .await
            .map_err(|e| NinetyNineError::PostgresPool {
                message: format!("failed to get connection: {e}"),
            })?;

        Self::migrate(&client).await?;

        Ok(Self { pool })
    }

    async fn migrate(client: &tokio_postgres::Client) -> Result<(), NinetyNineError> {
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                )",
            )
            .await?;

        let row = client
            .query_one(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                &[],
            )
            .await?;
        let current_version: i32 = row.get(0);

        let migrations = [Self::migration_v1()];

        for (i, sql) in migrations.iter().enumerate() {
            let version = i32::try_from(i).unwrap_or(0) + 1;
            if version > current_version {
                client.batch_execute(sql).await?;
                client
                    .execute(
                        "INSERT INTO schema_migrations (version) VALUES ($1)",
                        &[&version],
                    )
                    .await?;
            }
        }

        Ok(())
    }

    const fn migration_v1() -> &'static str {
        "CREATE TABLE IF NOT EXISTS run_sessions (
            id TEXT PRIMARY KEY,
            started_at TIMESTAMPTZ NOT NULL,
            finished_at TIMESTAMPTZ,
            test_count INTEGER NOT NULL DEFAULT 0,
            flaky_count INTEGER NOT NULL DEFAULT 0,
            commit_hash TEXT NOT NULL DEFAULT '',
            branch TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS test_runs (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES run_sessions(id),
            test_name TEXT NOT NULL,
            test_path TEXT NOT NULL,
            outcome TEXT NOT NULL,
            duration_ms BIGINT NOT NULL,
            timestamp TIMESTAMPTZ NOT NULL,
            commit_hash TEXT NOT NULL DEFAULT '',
            branch TEXT NOT NULL DEFAULT '',
            retry_count INTEGER NOT NULL DEFAULT 0,
            error_message TEXT,
            stack_trace TEXT,
            env_os TEXT NOT NULL DEFAULT '',
            env_rust_version TEXT NOT NULL DEFAULT '',
            env_cpu_count INTEGER NOT NULL DEFAULT 0,
            env_memory_gb DOUBLE PRECISION NOT NULL DEFAULT 0,
            env_is_ci BOOLEAN NOT NULL DEFAULT FALSE,
            env_ci_provider TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_test_runs_name ON test_runs(test_name);
        CREATE INDEX IF NOT EXISTS idx_test_runs_session ON test_runs(session_id);

        CREATE TABLE IF NOT EXISTS flakiness_scores (
            test_name TEXT PRIMARY KEY,
            probability_flaky DOUBLE PRECISION NOT NULL,
            confidence DOUBLE PRECISION NOT NULL,
            pass_rate DOUBLE PRECISION NOT NULL,
            fail_rate DOUBLE PRECISION NOT NULL,
            total_runs BIGINT NOT NULL,
            consecutive_failures INTEGER NOT NULL DEFAULT 0,
            last_updated TIMESTAMPTZ NOT NULL,
            alpha DOUBLE PRECISION NOT NULL DEFAULT 1,
            beta DOUBLE PRECISION NOT NULL DEFAULT 1,
            posterior_mean DOUBLE PRECISION NOT NULL DEFAULT 0.5,
            posterior_variance DOUBLE PRECISION NOT NULL DEFAULT 0.25,
            ci_lower DOUBLE PRECISION NOT NULL DEFAULT 0,
            ci_upper DOUBLE PRECISION NOT NULL DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS quarantine (
            test_name TEXT PRIMARY KEY,
            quarantined_at TIMESTAMPTZ NOT NULL,
            reason TEXT NOT NULL DEFAULT '',
            flakiness_score DOUBLE PRECISION NOT NULL DEFAULT 0,
            auto_quarantined BOOLEAN NOT NULL DEFAULT FALSE
        );"
    }

    async fn get_client(&self) -> Result<deadpool_postgres::Object, NinetyNineError> {
        self.pool
            .get()
            .await
            .map_err(|e| NinetyNineError::PostgresPool {
                message: format!("pool error: {e}"),
            })
    }
}

fn parse_timestamp(s: &str) -> DateTime<Utc> {
    super::parse_timestamp(s)
}

impl Storage for PostgresStorage {
    async fn store_session(&self, session: &RunSession) -> Result<(), NinetyNineError> {
        let client = self.get_client().await?;

        client
                .execute(
                    "INSERT INTO run_sessions (id, started_at, finished_at, test_count, flaky_count, commit_hash, branch)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)
                     ON CONFLICT (id) DO UPDATE SET
                        started_at = EXCLUDED.started_at,
                        finished_at = EXCLUDED.finished_at,
                        test_count = EXCLUDED.test_count,
                        flaky_count = EXCLUDED.flaky_count,
                        commit_hash = EXCLUDED.commit_hash,
                        branch = EXCLUDED.branch",
                    &[
                        &session.id.to_string(),
                        &session.started_at.to_rfc3339(),
                        &session.finished_at.map(|dt| dt.to_rfc3339()),
                        &i32::try_from(session.test_count).unwrap_or(i32::MAX),
                        &i32::try_from(session.flaky_count).unwrap_or(i32::MAX),
                        &session.commit_hash,
                        &session.branch,
                    ],
                )
                .await?;

        Ok(())
    }

    async fn finish_session(
        &self,
        session_id: &Uuid,
        test_count: u32,
        flaky_count: u32,
    ) -> Result<(), NinetyNineError> {
        let client = self.get_client().await?;

        client
                .execute(
                    "UPDATE run_sessions SET finished_at = $1, test_count = $2, flaky_count = $3 WHERE id = $4",
                    &[
                        &Utc::now().to_rfc3339(),
                        &i32::try_from(test_count).unwrap_or(i32::MAX),
                        &i32::try_from(flaky_count).unwrap_or(i32::MAX),
                        &session_id.to_string(),
                    ],
                )
                .await?;

        Ok(())
    }

    async fn store_test_run(
        &self,
        run: &TestRun,
        session_id: &Uuid,
    ) -> Result<(), NinetyNineError> {
        let client = self.get_client().await?;

        client
                .execute(
                    "INSERT INTO test_runs (id, session_id, test_name, test_path, outcome, duration_ms,
                     timestamp, commit_hash, branch, retry_count, error_message, stack_trace,
                     env_os, env_rust_version, env_cpu_count, env_memory_gb, env_is_ci, env_ci_provider)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
                    &[
                        &run.id.to_string(),
                        &session_id.to_string(),
                        &run.test_name.as_ref(),
                        &run.test_path.to_string_lossy().into_owned(),
                        &run.outcome.to_string(),
                        &duration_to_ms(run.duration),
                        &run.timestamp.to_rfc3339(),
                        &run.commit_hash,
                        &run.branch,
                        &i32::try_from(run.retry_count).unwrap_or(i32::MAX),
                        &run.error_message,
                        &run.stack_trace,
                        &run.environment.os,
                        &run.environment.rust_version,
                        &i32::try_from(run.environment.cpu_count).unwrap_or(i32::MAX),
                        &run.environment.memory_gb,
                        &run.environment.is_ci,
                        &run.environment.ci_provider,
                    ],
                )
                .await?;

        Ok(())
    }

    async fn store_flakiness_score(&self, score: &FlakinessScore) -> Result<(), NinetyNineError> {
        let client = self.get_client().await?;

        client
                .execute(
                    "INSERT INTO flakiness_scores
                     (test_name, probability_flaky, confidence, pass_rate, fail_rate, total_runs,
                      consecutive_failures, last_updated, alpha, beta, posterior_mean, posterior_variance,
                      ci_lower, ci_upper)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                     ON CONFLICT (test_name) DO UPDATE SET
                        probability_flaky = EXCLUDED.probability_flaky,
                        confidence = EXCLUDED.confidence,
                        pass_rate = EXCLUDED.pass_rate,
                        fail_rate = EXCLUDED.fail_rate,
                        total_runs = EXCLUDED.total_runs,
                        consecutive_failures = EXCLUDED.consecutive_failures,
                        last_updated = EXCLUDED.last_updated,
                        alpha = EXCLUDED.alpha,
                        beta = EXCLUDED.beta,
                        posterior_mean = EXCLUDED.posterior_mean,
                        posterior_variance = EXCLUDED.posterior_variance,
                        ci_lower = EXCLUDED.ci_lower,
                        ci_upper = EXCLUDED.ci_upper",
                    &[
                        &score.test_name.as_ref(),
                        &score.probability_flaky,
                        &score.confidence,
                        &score.pass_rate,
                        &score.fail_rate,
                        &i64::try_from(score.total_runs).unwrap_or(i64::MAX),
                        &i32::try_from(score.consecutive_failures).unwrap_or(i32::MAX),
                        &score.last_updated.to_rfc3339(),
                        &score.bayesian_params.alpha,
                        &score.bayesian_params.beta,
                        &score.bayesian_params.posterior_mean,
                        &score.bayesian_params.posterior_variance,
                        &score.bayesian_params.credible_interval_lower,
                        &score.bayesian_params.credible_interval_upper,
                    ],
                )
                .await?;

        Ok(())
    }

    async fn get_test_runs(
        &self,
        test_name: &str,
        limit: u32,
    ) -> Result<Vec<TestRun>, NinetyNineError> {
        let client = self.get_client().await?;

        let rows = client
                .query(
                    "SELECT id, test_name, test_path, outcome, duration_ms, timestamp,
                            commit_hash, branch, retry_count, error_message, stack_trace,
                            env_os, env_rust_version, env_cpu_count, env_memory_gb, env_is_ci, env_ci_provider
                     FROM test_runs WHERE test_name = $1
                     ORDER BY timestamp DESC LIMIT $2",
                    &[&test_name, &i64::from(limit)],
                )
                .await?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in &rows {
            runs.push(extract_test_run(row).into_test_run());
        }
        Ok(runs)
    }

    async fn get_recent_sessions(&self, limit: u32) -> Result<Vec<RunSession>, NinetyNineError> {
        let client = self.get_client().await?;

        let rows = client
            .query(
                "SELECT id, started_at, finished_at, test_count, flaky_count, commit_hash, branch
                     FROM run_sessions ORDER BY started_at DESC LIMIT $1",
                &[&i64::from(limit)],
            )
            .await?;

        let mut sessions = Vec::with_capacity(rows.len());
        for row in &rows {
            let id_str: String = row.get(0);
            let started_str: String = row.get(1);
            let finished_str: Option<String> = row.get(2);
            let test_count: i32 = row.get(3);
            let flaky_count: i32 = row.get(4);
            let commit_hash: String = row.get(5);
            let branch: String = row.get(6);

            sessions.push(RunSession {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                started_at: parse_timestamp(&started_str),
                finished_at: finished_str.map(|s| parse_timestamp(&s)),
                test_count: u32::try_from(test_count).unwrap_or(0),
                flaky_count: u32::try_from(flaky_count).unwrap_or(0),
                commit_hash,
                branch,
            });
        }
        Ok(sessions)
    }

    async fn get_all_scores(&self) -> Result<Vec<FlakinessScore>, NinetyNineError> {
        let client = self.get_client().await?;

        let rows = client
            .query(
                "SELECT test_name, probability_flaky, confidence, pass_rate, fail_rate, total_runs,
                            consecutive_failures, last_updated, alpha, beta, posterior_mean,
                            posterior_variance, ci_lower, ci_upper
                     FROM flakiness_scores ORDER BY probability_flaky DESC",
                &[],
            )
            .await?;

        let mut scores = Vec::with_capacity(rows.len());
        for row in &rows {
            scores.push(extract_score(row).into_score());
        }
        Ok(scores)
    }

    async fn get_score(&self, test_name: &str) -> Result<Option<FlakinessScore>, NinetyNineError> {
        let client = self.get_client().await?;

        let rows = client
            .query(
                "SELECT test_name, probability_flaky, confidence, pass_rate, fail_rate, total_runs,
                            consecutive_failures, last_updated, alpha, beta, posterior_mean,
                            posterior_variance, ci_lower, ci_upper
                     FROM flakiness_scores WHERE test_name = $1",
                &[&test_name],
            )
            .await?;

        Ok(rows.first().map(|row| extract_score(row).into_score()))
    }

    async fn quarantine_test(
        &self,
        test_name: &str,
        reason: &str,
        score: f64,
        auto: bool,
    ) -> Result<(), NinetyNineError> {
        let client = self.get_client().await?;

        client
                .execute(
                    "INSERT INTO quarantine (test_name, quarantined_at, reason, flakiness_score, auto_quarantined)
                     VALUES ($1, $2, $3, $4, $5)
                     ON CONFLICT (test_name) DO UPDATE SET
                        quarantined_at = EXCLUDED.quarantined_at,
                        reason = EXCLUDED.reason,
                        flakiness_score = EXCLUDED.flakiness_score,
                        auto_quarantined = EXCLUDED.auto_quarantined",
                    &[
                        &test_name,
                        &Utc::now().to_rfc3339(),
                        &reason,
                        &score,
                        &auto,
                    ],
                )
                .await?;

        Ok(())
    }

    async fn unquarantine_test(&self, test_name: &str) -> Result<(), NinetyNineError> {
        let client = self.get_client().await?;

        client
            .execute("DELETE FROM quarantine WHERE test_name = $1", &[&test_name])
            .await?;

        Ok(())
    }

    async fn get_quarantined_tests(&self) -> Result<Vec<QuarantineEntry>, NinetyNineError> {
        let client = self.get_client().await?;

        let rows = client
            .query(
                "SELECT test_name, quarantined_at, reason, flakiness_score, auto_quarantined
                     FROM quarantine ORDER BY quarantined_at DESC",
                &[],
            )
            .await?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let test_name = TestName::from(row.get::<_, String>(0));
            let quarantined_str: String = row.get(1);
            let reason: String = row.get(2);
            let flakiness_score: f64 = row.get(3);
            let auto_quarantined: bool = row.get(4);

            entries.push(QuarantineEntry {
                test_name,
                quarantined_at: parse_timestamp(&quarantined_str),
                reason,
                flakiness_score,
                auto_quarantined,
            });
        }
        Ok(entries)
    }

    async fn is_quarantined(&self, test_name: &str) -> Result<bool, NinetyNineError> {
        let client = self.get_client().await?;

        let row = client
            .query_one(
                "SELECT COUNT(*) FROM quarantine WHERE test_name = $1",
                &[&test_name],
            )
            .await?;

        let count: i64 = row.get(0);
        Ok(count > 0)
    }

    async fn get_session_runs(&self, session_id: &Uuid) -> Result<Vec<TestRun>, NinetyNineError> {
        let client = self.get_client().await?;

        let rows = client
            .query(
                "SELECT id, test_name, test_path, outcome, duration_ms, timestamp,
                        commit_hash, branch, retry_count, error_message, stack_trace,
                        env_os, env_rust_version, env_cpu_count, env_memory_gb, env_is_ci, env_ci_provider
                 FROM test_runs WHERE session_id = $1
                 ORDER BY test_name ASC",
                &[&session_id.to_string()],
            )
            .await?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in &rows {
            runs.push(extract_test_run(row).into_test_run());
        }
        Ok(runs)
    }

    async fn purge_older_than(&self, days: u32) -> Result<u64, NinetyNineError> {
        let client = self.get_client().await?;

        let cutoff = Utc::now() - chrono::Duration::days(i64::from(days));
        let cutoff_str = cutoff.to_rfc3339();

        let count = client
            .execute("DELETE FROM test_runs WHERE timestamp < $1", &[&cutoff_str])
            .await?;

        Ok(count)
    }
}

fn extract_test_run(row: &tokio_postgres::Row) -> RawTestRunRow {
    RawTestRunRow {
        id: row.get(0),
        test_name: row.get(1),
        test_path: row.get(2),
        outcome: row.get(3),
        duration_ms: row.get(4),
        timestamp: row.get(5),
        commit_hash: row.get(6),
        branch: row.get(7),
        retry_count: u32::try_from(row.get::<_, i32>(8)).unwrap_or(0),
        error_message: row.get(9),
        stack_trace: row.get(10),
        env_os: row.get(11),
        env_rust_version: row.get(12),
        env_cpu_count: u32::try_from(row.get::<_, i32>(13)).unwrap_or(0),
        env_memory_gb: row.get(14),
        env_is_ci: row.get(15),
        env_ci_provider: row.get(16),
    }
}

fn extract_score(row: &tokio_postgres::Row) -> RawScoreRow {
    RawScoreRow {
        test_name: row.get(0),
        probability_flaky: row.get(1),
        confidence: row.get(2),
        pass_rate: row.get(3),
        fail_rate: row.get(4),
        total_runs: u64::try_from(row.get::<_, i64>(5)).unwrap_or(0),
        consecutive_failures: u32::try_from(row.get::<_, i32>(6)).unwrap_or(0),
        last_updated: row.get(7),
        alpha: row.get(8),
        beta: row.get(9),
        posterior_mean: row.get(10),
        posterior_variance: row.get(11),
        ci_lower: row.get(12),
        ci_upper: row.get(13),
    }
}
