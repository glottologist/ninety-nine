use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum NinetyNineError {
    #[error("config file not found: {path}")]
    ConfigNotFound { path: PathBuf },

    #[error("failed to parse config: {source}")]
    ConfigParse {
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to read config file {path}: {source}")]
    ConfigIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("no test runner available: install cargo-nextest or use cargo test")]
    NoRunnerAvailable,

    #[error("runner execution failed: {message}")]
    RunnerExecution { message: String },

    #[error("invalid configuration: {message}")]
    InvalidConfig { message: String },

    #[error("binary discovery failed: {message}")]
    BinaryDiscovery { message: String },

    #[error("test listing failed for {binary}: {message}")]
    TestListing { binary: PathBuf, message: String },

    #[error("test not found: {name}")]
    TestNotFound { name: String },

    #[error("filter parse error: {message}")]
    FilterParse { message: String },

    #[error("io error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("json serialization error: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },

    #[error("storage error: {source}")]
    Storage {
        #[from]
        source: rusqlite::Error,
    },

    #[error("postgres storage error: {source}")]
    PostgresStorage {
        #[from]
        source: tokio_postgres::Error,
    },

    #[error("postgres pool error: {message}")]
    PostgresPool { message: String },
}
