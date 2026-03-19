# Configuration Reference

All configuration lives in `.ninety-nine.toml` at the project root. Every field has a default value -- you only need to specify overrides.

## `[detection]`

Controls how tests are discovered and analyzed.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `min_runs` | u32 | `10` | Minimum number of runs per test |
| `confidence_threshold` | f64 | `0.95` | Confidence level required to classify as flaky |
| `window_size` | u32 | `100` | Number of recent runs used for trend analysis |
| `detection_methods` | list | `["Bayesian"]` | Enabled detection methods |
| `auto_detect` | bool | `true` | Automatically detect and run |
| `parallel_runs` | u32 | `3` | Number of concurrent test executions |

### Detection Methods

Only one detection method is currently implemented:

- `Bayesian` -- Beta distribution posterior inference using conjugate priors

### `[detection.duration_regression]`

Optional configuration for detecting tests whose execution time has significantly increased.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | -- | Enable duration regression detection |
| `min_history_runs` | u32 | -- | Minimum historical runs required before checking |
| `threshold` | enum | -- | How to determine a regression (see below) |

Duration regression is disabled by default (the entire section is `None`). To enable it, provide the full section:

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

## `[retry]`

Controls retry behavior when tests fail.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `unit_test_retries` | u32 | `2` | Number of retries for failed tests |
| `backoff_strategy` | enum | `Exponential` | Backoff strategy between retries |
| `max_retry_time_secs` | u64 | `300` | Maximum total time for retries (seconds) |

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
| `max_quarantine_days` | u32 | `30` | Maximum days a test stays quarantined |

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
| `retention_days` | u32 | `90` | Days to keep test run data |

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

## `[ci]`

CI integration settings.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `provider` | enum | none | CI provider for workflow generation |

Supported providers: `GitHub`, `GitLab`, `Jenkins`, `CircleCI`, `AzureDevOps`, `Buildkite`.

```toml
[ci]
provider = "GitHub"
```

## Full Example

```toml
[detection]
min_runs = 20
confidence_threshold = 0.99
window_size = 50
detection_methods = ["Bayesian"]
auto_detect = true
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
max_quarantine_days = 14

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

[ci]
provider = "GitHub"
```
