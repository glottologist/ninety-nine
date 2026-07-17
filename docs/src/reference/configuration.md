# Configuration Reference

All configuration lives in `.ninety-nine.toml` at the project root. Every field has a default value -- you only need to specify overrides.

## `[detection]`

Controls how tests are discovered and analyzed.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `min_runs` | u32 | `10` | Minimum number of runs per test |
| `confidence_threshold` | f64 | `0.95` | Confidence level required to classify as flaky |
| `window_size` | u32 | `100` | Number of recent runs used for trend analysis |
| `parallel_runs` | u32 | `3` | Number of concurrent test executions |

Detection uses Beta distribution posterior inference with conjugate priors. A test is only ever classified as flaky once at least one failure has actually been observed; long histories of clean passes always classify as stable.

### `[detection.duration_regression]`

Optional configuration for detecting tests whose execution time has significantly increased.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | -- | Enable duration regression detection |
| `min_history_runs` | u32 | -- | Minimum historical runs required before checking |
| `threshold` | enum | -- | How to determine a regression (see below) |

When the section is omitted entirely, the check runs with its long-standing defaults: a 10-run history requirement and a threshold of 2 standard deviations. Provide the section to tune those values, or set `enabled = false` to switch the check off:

```toml
[detection.duration_regression]
enabled = true
min_history_runs = 10

# Option A: flag when latest duration exceeds N standard deviations above the mean
[detection.duration_regression.threshold]
StdDev = 2.0

# Option B: flag when latest duration exceeds mean * multiplier
# [detection.duration_regression.threshold]
# Multiplier = 3.0
```

**Threshold variants:**

| Variant | Description |
|---------|-------------|
| `StdDev(f64)` | Flag when the latest duration is more than N standard deviations above the historical mean |
| `Multiplier(f64)` | Flag when the latest duration exceeds the historical mean multiplied by this factor |

## `[diagnose]`

Controls the multi-phase `diagnose` command (stress / isolation / optional rr).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `stress_runs` | u32 | `3` | Full-binary multi-threaded suite iterations |
| `isolation_runs` | u32 | `10` | Serial isolation iterations per candidate |
| `stress_threads` | u32 | `0` | libtest threads (0 = host parallelism) |
| `stress_timeout_secs` | u64 | `300` | Timeout per stress binary iteration |
| `record` | bool | `false` | Attempt rr recording for Intrinsic failures |
| `record_dir` | path | `.ninety-nine/recordings` | rr output directory |
| `record_attempts` | u32 | `10` | Max rr attempts per Intrinsic candidate |

## `[retry]`

Controls retry behavior when tests fail.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `unit_test_retries` | u32 | `2` | Number of retries for failed tests |
| `backoff_strategy` | enum | `Exponential` | Backoff strategy between retries |
| `max_retry_time_secs` | u64 | `300` | Time limit for a single test execution (seconds) |

Every attempt is recorded as its own run, so a test that fails and then passes on retry contributes both outcomes to its flakiness score — recovery on retry is itself flaky evidence, and raising the retry count therefore gathers more evidence per iteration rather than hiding failures.

### Backoff Strategies

```toml
# No delay between retries
[retry]
backoff_strategy = "None"

# Fixed delay
[retry.backoff_strategy]
Linear = { delay_ms = 200 }

# Exponential backoff (default: base_ms=100, factor=2.0, max_ms=5000)
[retry.backoff_strategy]
Exponential = { base_ms = 100, factor = 2.0, max_ms = 5000 }

# Fibonacci sequence delays
[retry.backoff_strategy]
Fibonacci = { start_ms = 100, max_ms = 5000 }
```

## `[quarantine]`

Controls test quarantine behavior.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `true` | Enable quarantine functionality |
| `auto_quarantine` | bool | `false` | Automatically quarantine tests exceeding thresholds |

### `[quarantine.threshold]`

Thresholds for auto-quarantine. A test must be classified as flaky AND exceed at least one threshold.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `consecutive_failures` | u32 | `3` | Consecutive trailing failures |
| `failure_rate` | f64 | `0.20` | Overall failure rate |
| `flakiness_score` | f64 | `0.15` | Bayesian P(flaky) |

## `[storage]`

Controls data persistence. Two backends are supported: SQLite (default) and PostgreSQL.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `backend` | enum | `"Sqlite"` | Storage backend: `Sqlite` or `Postgres` |
| `retention_days` | u32 | `90` | Days to keep test run data; sessions left without any runs by the purge are removed with them |

### `[storage.sqlite]`

SQLite-specific settings. Only used when `backend = "Sqlite"`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `database_path` | path | `$XDG_DATA_HOME/ninety-nine/ninety-nine.db` | Path to the SQLite database file |

```toml
[storage]
backend = "Sqlite"
retention_days = 90

[storage.sqlite]
database_path = ".ninety-nine/data.db"
```

### `[storage.postgres]`

PostgreSQL-specific settings. Required when `backend = "Postgres"`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `connection_string` | string | -- | PostgreSQL connection URL |
| `pool_size` | u32 | -- | Connection pool size |

```toml
[storage]
backend = "Postgres"
retention_days = 90

[storage.postgres]
connection_string = "postgresql://user:password@localhost:5432/ninety_nine"
pool_size = 8
```

> **Warning**: Setting `backend = "Postgres"` without providing `[storage.postgres]` will cause a startup error.

## `[reporting]`

Controls output behavior.

### `[reporting.console]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `summary_only` | bool | `false` | Show only summary counts instead of full table |

## Full Example

```toml
[detection]
min_runs = 20
confidence_threshold = 0.99
window_size = 50
parallel_runs = 4

[detection.duration_regression]
enabled = true
min_history_runs = 10

[detection.duration_regression.threshold]
StdDev = 2.5

[retry]
unit_test_retries = 3
max_retry_time_secs = 120

[retry.backoff_strategy]
Exponential = { base_ms = 200, factor = 2.0, max_ms = 10000 }

[quarantine]
enabled = true
auto_quarantine = true

[quarantine.threshold]
consecutive_failures = 5
failure_rate = 0.15
flakiness_score = 0.10

[storage]
backend = "Sqlite"
retention_days = 60

[storage.sqlite]
database_path = ".ninety-nine/data.db"

[reporting.console]
summary_only = false
```
