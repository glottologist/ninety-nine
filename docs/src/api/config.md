# Configuration API

The configuration module handles loading, parsing, and serializing the `.ninety-nine.toml` configuration file.

## Loading

### `load_config`

```rust
pub fn load_config(project_root: &Path) -> Result<Config, NinetyNineError>
```

Searches for `.ninety-nine.toml` in the given project root directory. Returns `Config::default()` if the file does not exist.

**Errors:** Returns `ConfigParse` if the file exists but contains invalid TOML, or `ConfigIo` if the file cannot be read.

### `default_config_toml`

```rust
pub fn default_config_toml() -> Result<String, NinetyNineError>
```

Serializes `Config::default()` to pretty-printed TOML. Used by the `init` command.

### `backoff_base_delay`

```rust
pub fn backoff_base_delay(strategy: &BackoffStrategy) -> Duration
```

Extracts the initial delay from a backoff strategy.

| Strategy | Base Delay |
|----------|-----------|
| `None` | 0ms |
| `Linear { delay_ms }` | `delay_ms` |
| `Exponential { base_ms, .. }` | `base_ms` |
| `Fibonacci { start_ms, .. }` | `start_ms` |

## Config Model

### `Config`

Top-level configuration struct. All fields have sensible defaults.

```rust
pub struct Config {
    pub detection: DetectionConfig,
    pub retry: RetryConfig,
    pub quarantine: QuarantineConfig,
    pub storage: StorageConfig,
    pub reporting: ReportingConfig,
    pub ci: CiConfig,
}
```

### `DetectionConfig`

Controls flakiness detection behavior.

```rust
pub struct DetectionConfig {
    pub min_runs: u32,
    pub confidence_threshold: f64,
    pub window_size: u32,
    pub detection_methods: Vec<DetectionMethod>,
    pub auto_detect: bool,
    pub parallel_runs: u32,
    pub duration_regression: Option<DurationRegressionConfig>,
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `min_runs` | 10 | Minimum iterations per test |
| `confidence_threshold` | 0.95 | Statistical confidence required to classify as flaky |
| `window_size` | 100 | Maximum historical runs to consider |
| `detection_methods` | `[Bayesian]` | Detection algorithms to use |
| `auto_detect` | true | Automatically run detection after tests |
| `parallel_runs` | 3 | Number of concurrent test executions |
| `duration_regression` | None | Optional duration regression detection config |

### `RetryConfig`

Controls test retry behavior on failure.

```rust
pub struct RetryConfig {
    pub unit_test_retries: u32,
    pub backoff_strategy: BackoffStrategy,
    pub max_retry_time_secs: u64,
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `unit_test_retries` | 2 | Maximum retries per failing test |
| `backoff_strategy` | Exponential(100ms, 2.0x, 5000ms) | Delay strategy between retries |
| `max_retry_time_secs` | 300 | Hard timeout across all retries |

### `BackoffStrategy`

```rust
pub enum BackoffStrategy {
    None,
    Linear { delay_ms: u64 },
    Exponential { base_ms: u64, factor: f64, max_ms: u64 },
    Fibonacci { start_ms: u64, max_ms: u64 },
}
```

### `QuarantineConfig`

Controls automatic and manual test quarantine.

```rust
pub struct QuarantineConfig {
    pub enabled: bool,
    pub auto_quarantine: bool,
    pub threshold: QuarantineThreshold,
    pub max_quarantine_days: u32,
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | true | Enable quarantine system |
| `auto_quarantine` | false | Automatically quarantine tests exceeding thresholds |
| `max_quarantine_days` | 30 | Days before quarantined tests are reviewed |

### `QuarantineThreshold`

```rust
pub struct QuarantineThreshold {
    pub consecutive_failures: u32,
    pub failure_rate: f64,
    pub flakiness_score: f64,
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `consecutive_failures` | 3 | Consecutive failures before quarantine |
| `failure_rate` | 0.20 | Failure rate threshold (20%) |
| `flakiness_score` | 0.15 | Bayesian score threshold |

### `StorageConfig`

```rust
pub struct StorageConfig {
    pub backend: StorageBackendType,
    pub retention_days: u32,
    pub sqlite: Option<SqliteConfig>,
    pub postgres: Option<PostgresConfig>,
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `backend` | `Sqlite` | Storage backend to use |
| `retention_days` | 90 | Days to retain test run data |

### `StorageBackendType`

```rust
pub enum StorageBackendType {
    Sqlite,
    Postgres,
}
```

### `SqliteConfig`

```rust
pub struct SqliteConfig {
    pub database_path: PathBuf,
}
```

Default path: `$XDG_DATA_HOME/ninety-nine/ninety-nine.db`

### `PostgresConfig`

```rust
pub struct PostgresConfig {
    pub connection_string: String,
    pub pool_size: u32,
}
```

### `ReportingConfig`

```rust
pub struct ReportingConfig {
    pub console: ConsoleOutputConfig,
}

pub struct ConsoleOutputConfig {
    pub summary_only: bool,  // default: false
}
```

### `CiConfig`

```rust
pub struct CiConfig {
    pub provider: Option<CiProvider>,
}
```

### `CiProvider`

```rust
pub enum CiProvider {
    GitHub,
    GitLab,
    Jenkins,
    CircleCI,
    AzureDevOps,
    Buildkite,
}
```

### `DurationRegressionConfig`

```rust
pub struct DurationRegressionConfig {
    pub enabled: bool,
    pub min_history_runs: u32,
    pub threshold: DurationThreshold,
}

pub enum DurationThreshold {
    Multiplier(f64),
    StdDev(f64),
}
```

See the [Configuration Reference](../reference/configuration.md) for TOML examples.
