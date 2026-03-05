# Configuration Reference

All configuration lives in `.ninety-nine.toml` at the project root. Every field has a default value — you only need to specify overrides.

## `[detection]`

Controls how tests are discovered and analyzed.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `min_runs` | u32 | 10 | Minimum number of runs per test |
| `confidence_threshold` | f64 | 0.95 | Confidence level required to classify as flaky |
| `window_size` | u32 | 100 | Number of recent runs used for trend analysis |
| `detection_methods` | list | `["Bayesian", "FrequencyBased"]` | Enabled detection methods |
| `auto_detect` | bool | true | Automatically detect and run |
| `parallel_runs` | u32 | 3 | Number of concurrent test executions |

### Detection Methods

- `Bayesian` — Beta distribution posterior inference
- `FrequencyBased` — Simple pass/fail ratio analysis
- `PatternMatching` — Time-of-day and environmental pattern detection

## `[retry]`

Controls retry behavior when tests fail.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `unit_test_retries` | u32 | 2 | Retry count for unit tests |
| `integration_test_retries` | u32 | 3 | Retry count for integration tests |
| `e2e_test_retries` | u32 | 1 | Retry count for end-to-end tests |
| `backoff_strategy` | enum | `Exponential` | Backoff between retries |
| `max_retry_time_secs` | u64 | 300 | Maximum time for a single test execution (timeout) |
| `fail_fast` | bool | false | Stop on first failure |

### Backoff Strategies

```toml
# No delay between retries
[retry]
backoff_strategy = "None"

# Fixed delay
[retry.backoff_strategy]
Linear = { delay_ms = 200 }

# Exponential backoff (default)
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
| `enabled` | bool | true | Enable quarantine functionality |
| `auto_quarantine` | bool | false | Automatically quarantine tests exceeding thresholds |
| `max_quarantine_days` | u32 | 30 | Maximum days a test stays quarantined |

### `[quarantine.threshold]`

Thresholds for auto-quarantine. A test must be classified as flaky AND exceed at least one threshold.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `consecutive_failures` | u32 | 3 | Consecutive trailing failures |
| `failure_rate` | f64 | 0.20 | Overall failure rate |
| `flakiness_score` | f64 | 0.15 | Bayesian P(flaky) |

## `[storage]`

Controls data persistence.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `database_path` | path | `$XDG_DATA_HOME/ninety-nine/ninety-nine.db` | SQLite database path |
| `retention_days` | u32 | 90 | Days to keep test run data |

## `[reporting]`

Controls output behavior.

### `[reporting.console]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `verbose` | bool | false | Verbose console output |
| `color` | bool | true | Enable colored output |
| `progress_bar` | bool | true | Show progress bar during detection |
| `summary_only` | bool | false | Show only summary counts instead of full table |

## `[ci]`

CI integration settings.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `provider` | enum | none | CI provider (GitHub, GitLab, Jenkins, CircleCI, AzureDevOps, Buildkite) |
| `fail_on_flaky` | bool | false | Exit with error if flaky tests detected |

## `[advanced]`

Advanced execution settings.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `isolation_mode` | enum | `None` | Test isolation (`None` or `Process`) |

### `[advanced.resource_limits]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_memory_mb` | u64 | none | Maximum memory per test (optional) |
| `max_cpu_percent` | f64 | none | Maximum CPU usage (optional) |
| `max_threads` | u32 | none | Maximum threads per test (optional) |

## Example Full Configuration

```toml
[detection]
min_runs = 20
confidence_threshold = 0.99
window_size = 50
detection_methods = ["Bayesian", "PatternMatching"]
auto_detect = true
parallel_runs = 4

[retry]
unit_test_retries = 3
integration_test_retries = 2
e2e_test_retries = 1
max_retry_time_secs = 120
fail_fast = false

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
database_path = ".ninety-nine/data.db"
retention_days = 60

[reporting.console]
verbose = false
color = true
progress_bar = true
summary_only = false

[ci]
fail_on_flaky = true

[advanced]
isolation_mode = "Process"

[advanced.resource_limits]
max_memory_mb = 512
max_threads = 4
```
