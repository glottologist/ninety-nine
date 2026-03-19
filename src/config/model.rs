use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub detection: DetectionConfig,
    pub retry: RetryConfig,
    pub quarantine: QuarantineConfig,
    pub storage: StorageConfig,
    pub reporting: ReportingConfig,
    pub ci: CiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionConfig {
    pub min_runs: u32,
    pub confidence_threshold: f64,
    pub window_size: u32,
    pub detection_methods: Vec<DetectionMethod>,
    pub auto_detect: bool,
    pub parallel_runs: u32,
    pub duration_regression: Option<DurationRegressionConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DetectionMethod {
    Bayesian,
}

impl std::fmt::Display for DetectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bayesian => write!(f, "bayesian"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub unit_test_retries: u32,
    pub backoff_strategy: BackoffStrategy,
    pub max_retry_time_secs: u64,
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
pub struct QuarantineConfig {
    pub enabled: bool,
    pub auto_quarantine: bool,
    pub threshold: QuarantineThreshold,
    pub max_quarantine_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineThreshold {
    pub consecutive_failures: u32,
    pub failure_rate: f64,
    pub flakiness_score: f64,
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
pub struct StorageConfig {
    pub backend: StorageBackendType,
    pub retention_days: u32,
    pub sqlite: Option<SqliteConfig>,
    pub postgres: Option<PostgresConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportingConfig {
    pub console: ConsoleOutputConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleOutputConfig {
    pub summary_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiConfig {
    pub provider: Option<CiProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CiProvider {
    GitHub,
    GitLab,
    Jenkins,
    CircleCI,
    AzureDevOps,
    Buildkite,
}

impl std::fmt::Display for CiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHub => write!(f, "GitHub Actions"),
            Self::GitLab => write!(f, "GitLab CI"),
            Self::Jenkins => write!(f, "Jenkins"),
            Self::CircleCI => write!(f, "CircleCI"),
            Self::AzureDevOps => write!(f, "Azure DevOps"),
            Self::Buildkite => write!(f, "Buildkite"),
        }
    }
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

impl Default for Config {
    fn default() -> Self {
        Self {
            detection: DetectionConfig {
                min_runs: 10,
                confidence_threshold: 0.95,
                window_size: 100,
                detection_methods: vec![DetectionMethod::Bayesian],
                auto_detect: true,
                parallel_runs: 3,
                duration_regression: None,
            },
            retry: RetryConfig {
                unit_test_retries: 2,
                backoff_strategy: BackoffStrategy::Exponential {
                    base_ms: 100,
                    factor: 2.0,
                    max_ms: 5000,
                },
                max_retry_time_secs: 300,
            },
            quarantine: QuarantineConfig {
                enabled: true,
                auto_quarantine: false,
                threshold: QuarantineThreshold {
                    consecutive_failures: 3,
                    failure_rate: 0.20,
                    flakiness_score: 0.15,
                },
                max_quarantine_days: 30,
            },
            storage: StorageConfig {
                backend: StorageBackendType::Sqlite,
                retention_days: 90,
                sqlite: Some(SqliteConfig {
                    database_path: default_database_path(),
                }),
                postgres: None,
            },
            reporting: ReportingConfig {
                console: ConsoleOutputConfig {
                    summary_only: false,
                },
            },
            ci: CiConfig { provider: None },
        }
    }
}

fn default_database_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ninety-nine")
        .join("ninety-nine.db")
}
