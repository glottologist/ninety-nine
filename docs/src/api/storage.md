# Storage API

The storage module provides an async trait abstraction over test result persistence, with SQLite and PostgreSQL backends.

## `Storage` Trait

```rust
pub trait Storage: Send + Sync {
    async fn store_session(&self, session: &RunSession) -> Result<(), NinetyNineError>;
    async fn finish_session(&self, session_id: &Uuid, test_count: u32, flaky_count: u32) -> Result<(), NinetyNineError>;
    async fn store_test_run(&self, run: &TestRun, session_id: &Uuid) -> Result<(), NinetyNineError>;
    async fn store_flakiness_score(&self, score: &FlakinessScore) -> Result<(), NinetyNineError>;
    async fn get_test_runs(&self, test_name: &str, limit: u32) -> Result<Vec<TestRun>, NinetyNineError>;
    async fn get_recent_sessions(&self, limit: u32) -> Result<Vec<RunSession>, NinetyNineError>;
    async fn get_all_scores(&self) -> Result<Vec<FlakinessScore>, NinetyNineError>;
    async fn get_score(&self, test_name: &str) -> Result<Option<FlakinessScore>, NinetyNineError>;
    async fn quarantine_test(&self, test_name: &str, reason: &str, score: f64, auto: bool) -> Result<(), NinetyNineError>;
    async fn unquarantine_test(&self, test_name: &str) -> Result<(), NinetyNineError>;
    async fn get_quarantined_tests(&self) -> Result<Vec<QuarantineEntry>, NinetyNineError>;
    async fn is_quarantined(&self, test_name: &str) -> Result<bool, NinetyNineError>;
    async fn purge_older_than(&self, days: u32) -> Result<u64, NinetyNineError>;
}
```

All methods are async to support both synchronous (SQLite via `spawn_blocking`) and natively async (PostgreSQL) backends.

### Method Reference

| Method | Description |
|--------|-------------|
| `store_session` | Persists a new run session |
| `finish_session` | Marks a session as complete with final test/flaky counts |
| `store_test_run` | Stores a single test execution result |
| `store_flakiness_score` | Upserts a computed flakiness score (INSERT OR REPLACE) |
| `get_test_runs` | Retrieves runs for a test, most recent first |
| `get_recent_sessions` | Lists recent sessions, most recent first |
| `get_all_scores` | Returns all scores ordered by `probability_flaky` descending |
| `get_score` | Returns the score for a specific test, or `None` |
| `quarantine_test` | Adds a test to quarantine with reason and score |
| `unquarantine_test` | Removes a test from quarantine |
| `get_quarantined_tests` | Lists all quarantined tests |
| `is_quarantined` | Checks if a specific test is quarantined |
| `purge_older_than` | Deletes test runs older than N days, returns count deleted |

## `StorageBackend` Enum

```rust
pub enum StorageBackend {
    Sqlite(SqliteStorage),
    Postgres(PostgresStorage),
}
```

Delegates all `Storage` trait methods to the underlying backend.

## Factory Function

### `open_storage`

```rust
pub async fn open_storage(config: &Config) -> Result<StorageBackend, NinetyNineError>
```

Opens the configured storage backend:

- **`StorageBackendType::Sqlite`** — Opens (or creates) a SQLite database at the configured path, defaulting to `$XDG_DATA_HOME/ninety-nine/ninety-nine.db`
- **`StorageBackendType::Postgres`** — Connects to PostgreSQL using the configured connection string and pool size

**Errors:** Returns `InvalidConfig` if Postgres is selected but no `[storage.postgres]` configuration is provided.

## SQLite Backend

### `SqliteStorage`

```rust
pub struct SqliteStorage { /* private fields */ }
```

**Constructor:**

```rust
pub fn open(db_path: &Path) -> Result<Self, NinetyNineError>
```

Opens the database, creates parent directories if needed, enables WAL mode for concurrent reads/writes, and runs schema migrations.

**Features:**
- WAL (Write-Ahead Logging) mode for better concurrent access
- Automatic schema creation on first open
- Bundled SQLite via `rusqlite` (no system dependency required)

**Default location:** `$XDG_DATA_HOME/ninety-nine/ninety-nine.db`
- Linux: `~/.local/share/ninety-nine/ninety-nine.db`
- macOS: `~/Library/Application Support/ninety-nine/ninety-nine.db`

## PostgreSQL Backend

### `PostgresStorage`

```rust
pub struct PostgresStorage { /* private fields */ }
```

**Constructor:**

```rust
pub async fn connect(
    connection_string: &str,
    pool_size: u32,
) -> Result<Self, NinetyNineError>
```

Connects to PostgreSQL and initializes the schema. Uses `deadpool-postgres` for connection pooling.

| Parameter | Type | Description |
|-----------|------|-------------|
| `connection_string` | `&str` | PostgreSQL connection URL |
| `pool_size` | `u32` | Maximum number of pooled connections |

**Features:**
- Connection pooling via `deadpool-postgres`
- Automatic schema creation on first connect
- Natively async operations (no `spawn_blocking` needed)

## Database Schema

Both backends share the same logical schema with four tables:

```sql
-- Test execution sessions
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    test_count INTEGER NOT NULL DEFAULT 0,
    flaky_count INTEGER NOT NULL DEFAULT 0,
    commit_hash TEXT NOT NULL,
    branch TEXT NOT NULL
);

-- Individual test run results
CREATE TABLE test_runs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    test_name TEXT NOT NULL,
    test_path TEXT NOT NULL,
    outcome TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    timestamp TEXT NOT NULL,
    commit_hash TEXT NOT NULL,
    branch TEXT NOT NULL,
    environment TEXT NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    stack_trace TEXT
);

-- Computed flakiness scores (upserted)
CREATE TABLE flakiness_scores (
    test_name TEXT PRIMARY KEY,
    probability_flaky REAL NOT NULL,
    confidence REAL NOT NULL,
    pass_rate REAL NOT NULL,
    fail_rate REAL NOT NULL,
    total_runs INTEGER NOT NULL,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    last_updated TEXT NOT NULL,
    bayesian_params TEXT NOT NULL
);

-- Quarantine entries
CREATE TABLE quarantine (
    test_name TEXT PRIMARY KEY,
    quarantined_at TEXT NOT NULL,
    reason TEXT NOT NULL,
    flakiness_score REAL NOT NULL DEFAULT 0.0,
    auto_quarantined INTEGER NOT NULL DEFAULT 0
);
```

## Utility Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `parse_timestamp` | `fn(s: &str) -> DateTime<Utc>` | Parses RFC 3339 timestamps, falls back to `Utc::now()` |
| `duration_to_ms` | `fn(d: Duration) -> i64` | Converts `Duration` to milliseconds for storage |
| `ms_to_duration` | `fn(ms: i64) -> Duration` | Converts milliseconds back to `Duration` |

See the [Storage Reference](../reference/storage.md) for configuration and migration details.
