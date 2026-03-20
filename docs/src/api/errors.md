# Error Types

All fallible operations in `cargo-ninety-nine` return `Result<T, NinetyNineError>`.

## `NinetyNineError`

A comprehensive error enum covering all failure modes, derived with `thiserror`.

```rust
pub enum NinetyNineError {
    ConfigNotFound { path: PathBuf },
    ConfigParse { source: toml::de::Error },
    ConfigIo { path: PathBuf, source: std::io::Error },
    NoRunnerAvailable,
    RunnerExecution { message: String },
    InvalidConfig { message: String },
    BinaryDiscovery { message: String },
    TestListing { binary: PathBuf, message: String },
    TestNotFound { name: String },
    FilterParse { message: String },
    Io { source: std::io::Error },
    Json { source: serde_json::Error },
    Storage { source: rusqlite::Error },
    PostgresStorage { source: tokio_postgres::Error },
    PostgresPool { message: String },
}
```

## Error Categories

### Configuration Errors

| Variant | Display Message | Cause |
|---------|----------------|-------|
| `ConfigNotFound` | `config file not found: {path}` | `.ninety-nine.toml` does not exist at the expected path |
| `ConfigParse` | `failed to parse config: {source}` | TOML syntax error or invalid field value |
| `ConfigIo` | `failed to read config file {path}: {source}` | File exists but cannot be read (permissions, etc.) |
| `InvalidConfig` | `invalid configuration: {message}` | Logical configuration error (e.g., Postgres backend without connection config) |

### Runner Errors

| Variant | Display Message | Cause |
|---------|----------------|-------|
| `NoRunnerAvailable` | `no test runner available: install cargo-nextest or use cargo test` | Neither cargo-nextest nor cargo test found |
| `RunnerExecution` | `runner execution failed: {message}` | Test binary failed to spawn or produced unexpected output |
| `BinaryDiscovery` | `binary discovery failed: {message}` | `cargo test --no-run` failed or produced unparseable output |
| `TestListing` | `test listing failed for {binary}: {message}` | Test binary `--list` invocation failed |
| `TestNotFound` | `test not found: {name}` | Requested test does not exist in the project |

### Filter Errors

| Variant | Display Message | Cause |
|---------|----------------|-------|
| `FilterParse` | `filter parse error: {message}` | Invalid filter DSL syntax |

### Storage Errors

| Variant | Display Message | Cause |
|---------|----------------|-------|
| `Storage` | `storage error: {source}` | SQLite operation failed (auto-converted from `rusqlite::Error`) |
| `PostgresStorage` | `postgres storage error: {source}` | PostgreSQL operation failed (auto-converted from `tokio_postgres::Error`) |
| `PostgresPool` | `postgres pool error: {message}` | Connection pool exhausted or configuration error |

### I/O and Serialization Errors

| Variant | Display Message | Cause |
|---------|----------------|-------|
| `Io` | `io error: {source}` | Generic I/O failure (auto-converted from `std::io::Error`) |
| `Json` | `json serialization error: {source}` | JSON serialization/deserialization failure (auto-converted from `serde_json::Error`) |

## Automatic Conversions

The following `From` implementations allow `?` propagation:

| Source Type | Target Variant |
|-------------|---------------|
| `std::io::Error` | `Io` |
| `serde_json::Error` | `Json` |
| `rusqlite::Error` | `Storage` |
| `tokio_postgres::Error` | `PostgresStorage` |

## Error Handling in Practice

All errors are displayed to stderr in the main entry point and cause the process to exit with code 1:

```rust
// Simplified from main.rs
match run(args).await {
    Ok(()) => {}
    Err(e) => {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
```

When `--verbose` is enabled, the tracing subscriber is set to `debug` level, providing additional context before errors surface.
