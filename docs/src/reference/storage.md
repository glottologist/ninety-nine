# Storage

## Database

`cargo ninety-nine` stores all data in a SQLite database using WAL (Write-Ahead Logging) mode for concurrent access.

### Default Location

```
$XDG_DATA_HOME/ninety-nine/ninety-nine.db
```

On Linux this is typically `~/.local/share/ninety-nine/ninety-nine.db`.

Configure a custom path:

```toml
[storage]
database_path = "/custom/path/ninety-nine.db"
```

The parent directory is created automatically if it doesn't exist.

## Schema

The database has four tables:

### `run_sessions`

Tracks each detection run.

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT (UUID) | Session identifier |
| `started_at` | TEXT (RFC 3339) | When detection started |
| `finished_at` | TEXT (RFC 3339) | When detection finished |
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
| `duration_ms` | INTEGER | Execution time in milliseconds |
| `timestamp` | TEXT (RFC 3339) | When this run occurred |
| `commit_hash` | TEXT | Git commit |
| `branch` | TEXT | Git branch |
| `retry_count` | INTEGER | Number of retries used |
| `error_message` | TEXT | Stderr/stdout on failure |
| `stack_trace` | TEXT | Stack trace if available |
| `env_*` | Various | Environment metadata (OS, Rust version, CPU count, memory, CI) |

Indexed on `test_name`, `timestamp`, and `session_id`.

### `flakiness_scores`

Latest computed flakiness scores (upserted on test_name).

| Column | Type | Description |
|--------|------|-------------|
| `test_name` | TEXT (PK) | Fully qualified test name |
| `probability_flaky` | REAL | Bayesian P(flaky) |
| `confidence` | REAL | 1 - credible interval width |
| `pass_rate` | REAL | Passes / total |
| `fail_rate` | REAL | Failures / total |
| `total_runs` | INTEGER | Number of runs |
| `consecutive_failures` | INTEGER | Trailing failures |
| `last_updated` | TEXT (RFC 3339) | Last computation time |
| `alpha`, `beta` | REAL | Beta distribution parameters |
| `posterior_mean`, `posterior_variance` | REAL | Posterior statistics |
| `ci_lower`, `ci_upper` | REAL | 95% credible interval bounds |

### `quarantine`

Quarantined test records.

| Column | Type | Description |
|--------|------|-------------|
| `test_name` | TEXT (PK) | Fully qualified test name |
| `quarantined_at` | TEXT (RFC 3339) | When quarantined |
| `reason` | TEXT | Reason for quarantine |
| `flakiness_score` | REAL | P(flaky) at time of quarantine |
| `auto_quarantined` | INTEGER (bool) | Whether auto-quarantined |

Indexed on `quarantined_at`.

## Migrations

Schema migrations use SQLite's `PRAGMA user_version`. Each migration increments the version. Migrations are idempotent — running the tool against an already-migrated database is safe.

## Data Retention

Old test runs are automatically purged based on the `retention_days` config (default: 90 days). Purging happens after each `detect` session. Only the `test_runs` table is purged — `flakiness_scores` and `quarantine` entries persist.

```toml
[storage]
retention_days = 30
```

## Storage Trait

The `Storage` trait defines the interface for data access:

```rust
pub trait Storage {
    fn store_session(&self, session: &RunSession) -> Result<(), NinetyNineError>;
    fn finish_session(&self, session_id: &Uuid, test_count: u32, flaky_count: u32) -> Result<(), NinetyNineError>;
    fn store_test_run(&self, run: &TestRun, session_id: &Uuid) -> Result<(), NinetyNineError>;
    fn store_flakiness_score(&self, score: &FlakinessScore) -> Result<(), NinetyNineError>;
    fn get_test_runs(&self, test_name: &str, limit: u32) -> Result<Vec<TestRun>, NinetyNineError>;
    fn get_recent_sessions(&self, limit: u32) -> Result<Vec<RunSession>, NinetyNineError>;
    fn get_all_scores(&self) -> Result<Vec<FlakinessScore>, NinetyNineError>;
    fn get_score(&self, test_name: &str) -> Result<Option<FlakinessScore>, NinetyNineError>;
    fn quarantine_test(&self, test_name: &str, reason: &str, score: f64, auto: bool) -> Result<(), NinetyNineError>;
    fn unquarantine_test(&self, test_name: &str) -> Result<(), NinetyNineError>;
    fn get_quarantined_tests(&self) -> Result<Vec<QuarantineEntry>, NinetyNineError>;
    fn is_quarantined(&self, test_name: &str) -> Result<bool, NinetyNineError>;
    fn purge_older_than(&self, days: u32) -> Result<u64, NinetyNineError>;
}
```

The `SqliteStorage` struct is the sole implementation. The trait exists to enable alternative backends in the future.
