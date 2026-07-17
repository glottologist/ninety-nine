use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub detection: DetectionConfig,
    pub retry: RetryConfig,
    pub quarantine: QuarantineConfig,
    pub storage: StorageConfig,
    pub reporting: ReportingConfig,
    pub diagnose: DiagnoseConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DiagnoseConfig {
    pub stress_runs: u32,
    pub isolation_runs: u32,
    /// Zero means use available parallelism at resolve time.
    pub stress_threads: u32,
    pub stress_timeout_secs: u64,
    pub record: bool,
    pub record_dir: PathBuf,
    pub record_attempts: u32,
}

impl Default for DiagnoseConfig {
    fn default() -> Self {
        Self {
            stress_runs: 3,
            isolation_runs: 10,
            stress_threads: 0,
            stress_timeout_secs: 300,
            record: false,
            record_dir: PathBuf::from(".ninety-nine/recordings"),
            record_attempts: 10,
        }
    }
}

impl DiagnoseConfig {
    #[must_use]
    pub fn effective_stress_threads(&self) -> usize {
        if self.stress_threads == 0 {
            std::thread::available_parallelism()
                .map(std::num::NonZeroUsize::get)
                .unwrap_or(4)
        } else {
            usize::try_from(self.stress_threads).unwrap_or(4)
        }
    }

    /// Validates diagnose configuration.
    ///
    /// # Errors
    ///
    /// Returns `InvalidConfig` when required counts are zero.
    pub fn validate(&self) -> Result<(), crate::error::NinetyNineError> {
        if self.stress_runs == 0 {
            return Err(crate::error::NinetyNineError::InvalidConfig {
                message: "diagnose.stress_runs must be >= 1".into(),
            });
        }
        if self.isolation_runs == 0 {
            return Err(crate::error::NinetyNineError::InvalidConfig {
                message: "diagnose.isolation_runs must be >= 1".into(),
            });
        }
        if self.record_attempts == 0 {
            return Err(crate::error::NinetyNineError::InvalidConfig {
                message: "diagnose.record_attempts must be >= 1".into(),
            });
        }
        if self.stress_timeout_secs == 0 {
            return Err(crate::error::NinetyNineError::InvalidConfig {
                message: "diagnose.stress_timeout_secs must be >= 1".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DetectionConfig {
    pub min_runs: u32,
    pub confidence_threshold: f64,
    pub window_size: u32,
    pub parallel_runs: u32,
    pub duration_regression: Option<DurationRegressionConfig>,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            min_runs: 10,
            confidence_threshold: 0.95,
            window_size: 100,
            parallel_runs: 3,
            duration_regression: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetryConfig {
    pub unit_test_retries: u32,
    pub backoff_strategy: BackoffStrategy,
    pub max_retry_time_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            unit_test_retries: 2,
            backoff_strategy: BackoffStrategy::Exponential {
                base_ms: 100,
                factor: 2.0,
                max_ms: 5000,
            },
            max_retry_time_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    None,
    Linear {
        delay_ms: u64,
    },
    Exponential {
        base_ms: u64,
        factor: f64,
        max_ms: u64,
    },
    Fibonacci {
        start_ms: u64,
        max_ms: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QuarantineConfig {
    pub enabled: bool,
    pub auto_quarantine: bool,
    pub threshold: QuarantineThreshold,
}

impl Default for QuarantineConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_quarantine: false,
            threshold: QuarantineThreshold::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QuarantineThreshold {
    pub consecutive_failures: u32,
    pub failure_rate: f64,
    pub flakiness_score: f64,
}

impl Default for QuarantineThreshold {
    fn default() -> Self {
        Self {
            consecutive_failures: 3,
            failure_rate: 0.20,
            flakiness_score: 0.15,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageBackendType {
    Sqlite,
    Postgres,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub database_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub connection_string: String,
    pub pool_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub backend: StorageBackendType,
    pub retention_days: u32,
    pub sqlite: Option<SqliteConfig>,
    pub postgres: Option<PostgresConfig>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackendType::Sqlite,
            retention_days: 90,
            sqlite: Some(SqliteConfig {
                database_path: default_database_path(),
            }),
            postgres: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportingConfig {
    pub console: ConsoleOutputConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ConsoleOutputConfig {
    pub summary_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurationRegressionConfig {
    pub enabled: bool,
    pub min_history_runs: u32,
    pub threshold: DurationThreshold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DurationThreshold {
    Multiplier(f64),
    StdDev(f64),
}

fn default_database_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ninety-nine")
        .join("ninety-nine.db")
}
