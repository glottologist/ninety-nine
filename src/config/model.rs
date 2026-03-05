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
    pub advanced: AdvancedConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionConfig {
    pub min_runs: u32,
    pub confidence_threshold: f64,
    pub window_size: u32,
    pub detection_methods: Vec<DetectionMethod>,
    pub auto_detect: bool,
    pub parallel_runs: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DetectionMethod {
    Bayesian,
    FrequencyBased,
    PatternMatching,
}

impl std::fmt::Display for DetectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bayesian => write!(f, "bayesian"),
            Self::FrequencyBased => write!(f, "frequency-based"),
            Self::PatternMatching => write!(f, "pattern-matching"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub unit_test_retries: u32,
    pub integration_test_retries: u32,
    pub e2e_test_retries: u32,
    pub backoff_strategy: BackoffStrategy,
    pub max_retry_time_secs: u64,
    pub fail_fast: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub database_path: PathBuf,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportingConfig {
    pub console: ConsoleOutputConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleOutputConfig {
    pub verbose: bool,
    pub color: bool,
    pub progress_bar: bool,
    pub summary_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiConfig {
    pub provider: Option<CiProvider>,
    pub fail_on_flaky: bool,
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
pub struct AdvancedConfig {
    pub isolation_mode: IsolationMode,
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IsolationMode {
    None,
    Process,
}

impl std::fmt::Display for IsolationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Process => write!(f, "process"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: Option<u64>,
    pub max_cpu_percent: Option<f64>,
    pub max_threads: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            detection: DetectionConfig {
                min_runs: 10,
                confidence_threshold: 0.95,
                window_size: 100,
                detection_methods: vec![DetectionMethod::Bayesian, DetectionMethod::FrequencyBased],
                auto_detect: true,
                parallel_runs: 3,
            },
            retry: RetryConfig {
                unit_test_retries: 2,
                integration_test_retries: 3,
                e2e_test_retries: 1,
                backoff_strategy: BackoffStrategy::Exponential {
                    base_ms: 100,
                    factor: 2.0,
                    max_ms: 5000,
                },
                max_retry_time_secs: 300,
                fail_fast: false,
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
                database_path: default_database_path(),
                retention_days: 90,
            },
            reporting: ReportingConfig {
                console: ConsoleOutputConfig {
                    verbose: false,
                    color: true,
                    progress_bar: true,
                    summary_only: false,
                },
            },
            ci: CiConfig {
                provider: None,
                fail_on_flaky: false,
            },
            advanced: AdvancedConfig {
                isolation_mode: IsolationMode::None,
                resource_limits: ResourceLimits {
                    max_memory_mb: None,
                    max_cpu_percent: None,
                    max_threads: None,
                },
            },
        }
    }
}

fn default_database_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ninety-nine")
        .join("ninety-nine.db")
}
