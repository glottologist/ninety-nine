pub mod model;

pub use model::Config;

use std::path::Path;
use std::time::Duration;

use crate::error::NinetyNineError;

const CONFIG_FILE_NAME: &str = ".ninety-nine.toml";

/// Loads configuration from project root.
///
/// # Errors
///
/// Returns `ConfigIo` if the config file exists but cannot be read,
/// or `ConfigParse` if the file contents are not valid TOML.
pub fn load_config(project_root: &Path) -> Result<Config, NinetyNineError> {
    let config_path = project_root.join(CONFIG_FILE_NAME);

    if !config_path.exists() {
        return Ok(Config::default());
    }

    let contents = std::fs::read_to_string(&config_path).map_err(|source| {
        NinetyNineError::ConfigIo {
            path: config_path.clone(), // clone: needed after move into closure
            source,
        }
    })?;

    let config: Config =
        toml::from_str(&contents).map_err(|source| NinetyNineError::ConfigParse { source })?;

    Ok(config)
}

/// Serializes the default config to TOML.
///
/// # Errors
///
/// Returns `InvalidConfig` if serialization fails.
pub fn default_config_toml() -> Result<String, NinetyNineError> {
    let config = Config::default();
    toml::to_string_pretty(&config).map_err(|e| NinetyNineError::InvalidConfig {
        message: e.to_string(),
    })
}

/// Extracts the base delay from a backoff strategy.
#[must_use]
pub const fn backoff_base_delay(strategy: &model::BackoffStrategy) -> Duration {
    match strategy {
        model::BackoffStrategy::None => Duration::ZERO,
        model::BackoffStrategy::Linear { delay_ms } => Duration::from_millis(*delay_ms),
        model::BackoffStrategy::Exponential { base_ms, .. } => Duration::from_millis(*base_ms),
        model::BackoffStrategy::Fibonacci { start_ms, .. } => Duration::from_millis(*start_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[test]
    fn load_config_missing_file_returns_default() {
        let config = load_config(Path::new("/tmp/nonexistent-dir-12345")).unwrap();
        assert_eq!(config.detection.min_runs, 10);
    }

    #[test]
    fn load_config_accepts_partial_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CONFIG_FILE_NAME),
            "[detection]\nmin_runs = 20\n\n[storage]\nretention_days = 30\n",
        )
        .unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.detection.min_runs, 20);
        assert_eq!(config.storage.retention_days, 30);
        assert!((config.detection.confidence_threshold - 0.95).abs() < f64::EPSILON);
        assert_eq!(config.retry.unit_test_retries, 2);
        assert_eq!(config.storage.backend, model::StorageBackendType::Sqlite);
    }

    #[test]
    fn load_config_malformed_toml_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE_NAME), "{{invalid toml!").unwrap();
        let result = load_config(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn default_config_toml_roundtrips() {
        let toml_str = default_config_toml().unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.detection.min_runs, 10);
        assert!((parsed.detection.confidence_threshold - 0.95).abs() < f64::EPSILON);
        assert_eq!(parsed.diagnose.stress_runs, 3);
        assert_eq!(parsed.diagnose.isolation_runs, 10);
        assert!(!parsed.diagnose.record);
    }

    #[test]
    fn diagnose_config_defaults() {
        let d = model::DiagnoseConfig::default();
        assert_eq!(d.stress_runs, 3);
        assert_eq!(d.isolation_runs, 10);
        assert_eq!(d.stress_threads, 0);
        assert!(!d.record);
        assert_eq!(d.record_attempts, 10);
        assert!(d.validate().is_ok());
    }

    #[test]
    fn diagnose_validate_rejects_zero_stress_runs() {
        let d = model::DiagnoseConfig {
            stress_runs: 0,
            ..model::DiagnoseConfig::default()
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn diagnose_validate_allows_zero_stress_threads() {
        let d = model::DiagnoseConfig {
            stress_threads: 0,
            ..model::DiagnoseConfig::default()
        };
        assert!(d.validate().is_ok());
        assert!(d.effective_stress_threads() >= 1);
    }

    #[test]
    fn load_config_diagnose_override() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CONFIG_FILE_NAME),
            "[diagnose]\nstress_runs = 7\n",
        )
        .unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.diagnose.stress_runs, 7);
        assert_eq!(config.diagnose.isolation_runs, 10);
    }

    #[rstest]
    #[case(model::BackoffStrategy::None, 0)]
    #[case(model::BackoffStrategy::Linear { delay_ms: 200 }, 200)]
    #[case(model::BackoffStrategy::Exponential { base_ms: 100, factor: 2.0, max_ms: 5000 }, 100)]
    #[case(model::BackoffStrategy::Fibonacci { start_ms: 50, max_ms: 1000 }, 50)]
    fn backoff_base_delay_extracts_correctly(
        #[case] strategy: model::BackoffStrategy,
        #[case] expected_ms: u64,
    ) {
        assert_eq!(
            backoff_base_delay(&strategy),
            Duration::from_millis(expected_ms)
        );
    }

    proptest! {
        #[test]
        fn config_roundtrip_preserves_values(
            min_runs in 1u32..1000,
            confidence in 0.5f64..1.0,
            retention in 1u32..365,
        ) {
            let mut config = Config::default();
            config.detection.min_runs = min_runs;
            config.detection.confidence_threshold = confidence;
            config.storage.retention_days = retention;

            let toml_str = toml::to_string_pretty(&config).unwrap();
            let parsed: Config = toml::from_str(&toml_str).unwrap();

            prop_assert_eq!(parsed.detection.min_runs, min_runs);
            prop_assert!((parsed.detection.confidence_threshold - confidence).abs() < f64::EPSILON);
            prop_assert_eq!(parsed.storage.retention_days, retention);
        }
    }
}
