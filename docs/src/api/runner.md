# Runner API

The runner module handles test discovery, execution, and result collection.

## Test Discovery

### `discover_test_binaries`

```rust
pub fn discover_test_binaries(
    project_root: &Path,
) -> Result<Vec<TestBinary>, NinetyNineError>
```

Discovers all test binaries in a Cargo project by running `cargo test --no-run --message-format json-render-diagnostics` and parsing the output.

**Returns:** A list of `TestBinary` structs, each containing the binary path, package name, and kind.

**Errors:** Returns `BinaryDiscovery` if cargo fails or output cannot be parsed.

### `list_tests_parallel`

```rust
pub async fn list_tests_parallel(
    binaries: &[TestBinary],
    concurrency: usize,
) -> Result<Vec<TestCase>, NinetyNineError>
```

Lists all tests across multiple binaries concurrently. Each binary is invoked with `--list --format terse` and output lines ending with `: test` or `: benchmark` are parsed.

Uses `tokio::sync::Semaphore` for concurrency control.

**Errors:** Returns `TestListing` if a binary fails to produce test listings.

### `detect_available_runner`

```rust
pub fn detect_available_runner() -> Option<AvailableRunner>
```

Checks if a test runner is available on the system.

```rust
pub enum AvailableRunner {
    Nextest,
    CargoTest,
}
```

Prefers `cargo-nextest` if installed, falls back to `cargo test`.

## Test Execution

### `Executor`

```rust
pub struct Executor<'a> {
    config: &'a ExecutionConfig,
}
```

Runs individual test cases with retry support.

**Constructor:**

```rust
pub fn new(config: &'a ExecutionConfig) -> Self
```

#### `run_single`

```rust
pub fn run_single(
    &self,
    test_case: &TestCase,
) -> Result<TestResult, NinetyNineError>
```

Executes a single test case with retries. Spawns the test binary with the `--exact` flag targeting the specific test.

**Retry behavior:**
- Retries up to `config.retries` times on failure
- Stops immediately on first pass
- Applies `config.retry_delay` between attempts

**Timeout:** Uses polling-based detection (50ms intervals). Kills the process when the deadline is exceeded, returning `TestOutcome::Timeout`.

**Outcome classification:**
| Condition | Outcome |
|-----------|---------|
| Exit code 0 | `Passed` |
| `panicked at` in stderr/stdout | `Panic` |
| Deadline exceeded | `Timeout` |
| Other non-zero exit | `Failed` |

### `ExecutionConfig`

```rust
pub struct ExecutionConfig {
    pub concurrency: usize,
    pub timeout: Duration,
    pub retries: u32,
    pub retry_delay: Duration,
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `concurrency` | — | Maximum parallel test binary invocations |
| `timeout` | 300s | Per-test execution timeout |
| `retries` | 0 | Number of retry attempts on failure |
| `retry_delay` | 100ms | Delay between retry attempts |

### `TestResult`

```rust
pub struct TestResult {
    pub test_case: TestCase,
    pub outcome: TestOutcome,
    pub duration: Duration,
    pub stdout: String,
    pub stderr: String,
    pub attempt: u32,
}
```

## Test Case Types

### `TestCase`

```rust
pub struct TestCase {
    pub name: TestName,
    pub binary_path: PathBuf,
    pub binary_name: String,
    pub package_name: String,
    pub binary_kind: BinaryKind,
    pub kind: TestKind,
}
```

### `TestKind`

```rust
pub enum TestKind {
    Test,
    Benchmark,
}
```

### `BinaryKind`

```rust
pub enum BinaryKind {
    Lib,
    Bin,
    Test,
    Example,
}
```

Derived from Cargo metadata target kinds.

### `TestBinary`

```rust
pub struct TestBinary {
    pub path: PathBuf,
    pub package_name: String,
    pub binary_name: String,
    pub kind: BinaryKind,
}
```

## High-Level Runner

### `NativeRunner`

```rust
pub struct NativeRunner { /* private fields */ }
```

**Constructor:**

```rust
pub fn new(project_root: &Path, config: ExecutionConfig) -> Self
```

**Methods:**

| Method | Description |
|--------|-------------|
| `discover_tests(&self, filter: &str)` | Discovers test cases, optionally filtered by name substring |
| `run_test_sync(&self, test_case: &TestCase)` | Runs a single test case once |
| `run_test_repeatedly(&self, test_case, iterations, environment)` | Runs a test multiple times, returning `Vec<TestRun>` |

### `RunnerBackend`

```rust
pub enum RunnerBackend {
    Native(NativeRunner),
}
```

Extensible enum wrapping runner implementations. Currently supports native Cargo test execution.

**Methods:** `native()`, `execution_config()`, `discover_tests()`, `run_test_repeatedly()` — all delegate to the inner `NativeRunner`.

## Standalone Function

### `execute_iterations`

```rust
pub fn execute_iterations(
    test_case: &TestCase,
    iterations: u32,
    config: &ExecutionConfig,
    environment: &TestEnvironment,
) -> Result<Vec<TestRun>, NinetyNineError>
```

Convenience function that runs a test for N iterations and converts results to `TestRun` records. Used by the main command handler.
