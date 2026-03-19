# Storage

## Storage Trait

The `Storage` trait defines the async interface for all data persistence. Both backends implement all 13 methods:

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

The `StorageBackend` enum wraps both backends and dispatches calls via a `dispatch!` macro:

```rust
pub enum StorageBackend {
    Sqlite(SqliteStorage),
    Postgres(PostgresStorage),
}
```

The `open_storage()` factory function reads the config to initialize the correct backend.

## SQLite Backend (Default)

SQLite is the default backend, using `rusqlite` with the bundled SQLite library. No external database is required.

### Default Location

```
$XDG_DATA_HOME/ninety-nine/ninety-nine.db
```

On Linux this is typically `~/.local/share/ninety-nine/ninety-nine.db`. The parent directory is created automatically if it does not exist.

### Features

- **WAL mode** -- enabled on open for concurrent read access during detection
- **Foreign keys** -- enforced via `PRAGMA foreign_keys=ON`
- **Thread safety** -- `Mutex<Connection>` guards all access; async trait methods run synchronously within async signatures
- **Bundled SQLite** -- no system SQLite dependency required

### Configuration

```toml
[storage]
backend = "Sqlite"
retention_days = 90

[storage.sqlite]
database_path = "/custom/path/ninety-nine.db"
```

If `storage.sqlite` is omitted, the default path is used.

### Migrations

Schema migrations use SQLite's `PRAGMA user_version`. Each migration increments the version. Migrations are idempotent -- running the tool against an already-migrated database is safe.

## PostgreSQL Backend

PostgreSQL support uses `deadpool-postgres` for connection pooling with `tokio-postgres` for async queries.

### Features

- **Connection pooling** -- configurable pool size via `deadpool-postgres`
- **Pool timeouts** -- 30s wait, 10s create, 5s recycle
- **Native async** -- all queries use async `tokio-postgres` directly
- **Schema migrations** -- tracked via a `schema_migrations` table with version and timestamp

### Configuration

```toml
[storage]
backend = "Postgres"
retention_days = 90

[storage.postgres]
connection_string = "postgresql://user:password@localhost:5432/ninety_nine"
pool_size = 8
```

> **Note**: Selecting `backend = "Postgres"` without providing `[storage.postgres]` will result in a configuration error at startup.

### Migrations

PostgreSQL migrations use a `schema_migrations` table instead of pragmas. The migration system tracks applied versions and only runs new migrations. The schema is identical to SQLite in structure.

## Schema

The database has four tables, identical in structure across both backends.

### `run_sessions`

Tracks each detection run session.

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT (UUID) | Session identifier |
| `started_at` | TEXT/TIMESTAMPTZ | When the session started |
| `finished_at` | TEXT/TIMESTAMPTZ | When the session finished (nullable) |
| `test_count` | INTEGER | Total tests analyzed |
| `flaky_count` | INTEGER | Tests classified as flaky |
| `commit_hash` | TEXT | Git commit at time of run |
| `branch` | TEXT | Git branch at time of run |

### `test_runs`

Individual test execution results.

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT (UUID) | Run identifier |
| `session_id` | TEXT (FK) | Parent session |
| `test_name` | TEXT | Fully qualified test name |
| `test_path` | TEXT | Binary path |
| `outcome` | TEXT | passed, failed, timeout, panic, ignored |
| `duration_ms` | INTEGER/BIGINT | Execution time in milliseconds |
| `timestamp` | TEXT/TIMESTAMPTZ | When this run occurred |
| `commit_hash` | TEXT | Git commit |
| `branch` | TEXT | Git branch |
| `retry_count` | INTEGER | Number of retries used |
| `error_message` | TEXT | Stderr/stdout on failure (nullable) |
| `stack_trace` | TEXT | Stack trace if available (nullable) |
| `env_os` | TEXT | Operating system |
| `env_rust_version` | TEXT | Rust toolchain version |
| `env_cpu_count` | INTEGER | CPU core count |
| `env_memory_gb` | REAL/DOUBLE PRECISION | System memory in GB |
| `env_is_ci` | INTEGER/BOOLEAN | Whether running in CI |
| `env_ci_provider` | TEXT | CI provider name (nullable) |

Indexed on `test_name`, `timestamp`, and `session_id`.

### `flakiness_scores`

Latest computed flakiness scores, upserted on `test_name`.

| Column | Type | Description |
|--------|------|-------------|
| `test_name` | TEXT (PK) | Fully qualified test name |
| `probability_flaky` | REAL | Bayesian P(flaky) |
| `confidence` | REAL | 1 - credible interval width |
| `pass_rate` | REAL | Passes / total |
| `fail_rate` | REAL | Failures / total |
| `total_runs` | INTEGER | Number of runs |
| `consecutive_failures` | INTEGER | Trailing failures |
| `last_updated` | TEXT/TIMESTAMPTZ | Last computation time |
| `alpha` | REAL | Beta distribution alpha parameter |
| `beta` | REAL | Beta distribution beta parameter |
| `posterior_mean` | REAL | Posterior mean |
| `posterior_variance` | REAL | Posterior variance |
| `ci_lower` | REAL | 95% credible interval lower bound |
| `ci_upper` | REAL | 95% credible interval upper bound |

### `quarantine`

Quarantined test records.

| Column | Type | Description |
|--------|------|-------------|
| `test_name` | TEXT (PK) | Fully qualified test name |
| `quarantined_at` | TEXT/TIMESTAMPTZ | When quarantined |
| `reason` | TEXT | Reason for quarantine |
| `flakiness_score` | REAL | P(flaky) at time of quarantine |
| `auto_quarantined` | INTEGER/BOOLEAN | Whether auto-quarantined |

Indexed on `quarantined_at`.

## Data Retention

Old test runs are automatically purged based on the `retention_days` config (default: 90 days). Purging happens after each `test` session. Only the `test_runs` table is purged -- `flakiness_scores` and `quarantine` entries persist.

```toml
[storage]
retention_days = 30
```
