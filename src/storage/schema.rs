pub const MIGRATIONS: &[&str] = &[MIGRATION_001];

const MIGRATION_001: &str = r"
CREATE TABLE IF NOT EXISTS run_sessions (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    test_count INTEGER NOT NULL DEFAULT 0,
    flaky_count INTEGER NOT NULL DEFAULT 0,
    commit_hash TEXT NOT NULL,
    branch TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS test_runs (
    id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES run_sessions(id),
    test_name TEXT NOT NULL,
    test_path TEXT NOT NULL,
    outcome TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    timestamp TEXT NOT NULL,
    commit_hash TEXT NOT NULL,
    branch TEXT NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    stack_trace TEXT,
    env_os TEXT NOT NULL,
    env_rust_version TEXT NOT NULL,
    env_cpu_count INTEGER NOT NULL,
    env_memory_gb REAL NOT NULL,
    env_is_ci INTEGER NOT NULL DEFAULT 0,
    env_ci_provider TEXT
);

CREATE INDEX IF NOT EXISTS idx_test_runs_name ON test_runs(test_name);
CREATE INDEX IF NOT EXISTS idx_test_runs_timestamp ON test_runs(timestamp);
CREATE INDEX IF NOT EXISTS idx_test_runs_session ON test_runs(session_id);

CREATE TABLE IF NOT EXISTS flakiness_scores (
    test_name TEXT PRIMARY KEY,
    probability_flaky REAL NOT NULL,
    confidence REAL NOT NULL,
    pass_rate REAL NOT NULL,
    fail_rate REAL NOT NULL,
    total_runs INTEGER NOT NULL,
    consecutive_failures INTEGER NOT NULL,
    last_updated TEXT NOT NULL,
    alpha REAL NOT NULL,
    beta REAL NOT NULL,
    posterior_mean REAL NOT NULL,
    posterior_variance REAL NOT NULL,
    ci_lower REAL NOT NULL,
    ci_upper REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS quarantine (
    test_name TEXT PRIMARY KEY,
    quarantined_at TEXT NOT NULL,
    reason TEXT NOT NULL,
    flakiness_score REAL NOT NULL,
    auto_quarantined INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_quarantine_date ON quarantine(quarantined_at);
";
