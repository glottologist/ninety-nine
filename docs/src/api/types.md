# Types Reference

This chapter documents the core types used throughout `cargo-ninety-nine`.

## Test Execution

### `TestRun`

A single test execution record with full metadata.

```rust
pub struct TestRun {
    pub id: Uuid,
    pub test_name: TestName,
    pub test_path: PathBuf,
    pub outcome: TestOutcome,
    pub duration: Duration,
    pub timestamp: DateTime<Utc>,
    pub commit_hash: String,
    pub branch: String,
    pub environment: TestEnvironment,
    pub retry_count: u32,
    pub error_message: Option<String>,
    pub stack_trace: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Unique identifier for this run |
| `test_name` | `TestName` | Type-safe test name |
| `test_path` | `PathBuf` | Path to the test binary |
| `outcome` | `TestOutcome` | Result of the execution |
| `duration` | `Duration` | Wall-clock execution time |
| `timestamp` | `DateTime<Utc>` | When the run occurred |
| `commit_hash` | `String` | Git commit at time of run |
| `branch` | `String` | Git branch at time of run |
| `environment` | `TestEnvironment` | Execution environment details |
| `retry_count` | `u32` | Number of retries before this result |
| `error_message` | `Option<String>` | Failure message, if any |
| `stack_trace` | `Option<String>` | Stack trace on panic/failure |

### `TestOutcome`

Classification of a test execution result.

```rust
pub enum TestOutcome {
    Passed,
    Failed,
    Ignored,
    Timeout,
    Panic,
}
```

| Variant | Description |
|---------|-------------|
| `Passed` | Test completed successfully (exit code 0) |
| `Failed` | Test assertion failed (non-zero exit, no panic) |
| `Ignored` | Test marked with `#[ignore]` |
| `Timeout` | Test exceeded the configured timeout |
| `Panic` | Test panicked (detected via `panicked at` in output) |

Implements `Display` and `FromStr` for serialization. Display values: `"passed"`, `"failed"`, `"ignored"`, `"timeout"`, `"panic"`.

### `TestEnvironment`

Captures the environment where tests execute, used for pattern correlation.

```rust
pub struct TestEnvironment {
    pub os: String,
    pub rust_version: String,
    pub cpu_count: u32,
    pub memory_gb: f64,
    pub is_ci: bool,
    pub ci_provider: Option<String>,
}
```

Auto-detected at runtime from the host system. The `is_ci` field is inferred from environment variables (`GITHUB_ACTIONS`, `GITLAB_CI`, `JENKINS_URL`, `CIRCLECI`, `TF_BUILD`, `BUILDKITE`).

### `TestName`

Newtype wrapper providing type-safe test names. Prevents accidental confusion with branch names, commit hashes, or other string fields.

```rust
pub struct TestName(String);
```

**Conversions:**
- `From<String>`, `From<&str>` — construct from strings
- `Deref<Target = str>` — borrow as `&str`
- `AsRef<str>` — reference conversion
- `Display` — format for output
- `PartialEq<str>`, `PartialEq<&str>` — compare with string values

**Methods:**
- `into_inner(self) -> String` — consume and return the inner string

## Flakiness Detection

### `FlakinessScore`

The primary output of the Bayesian detection engine. Contains the computed probability that a test is flaky along with statistical parameters.

```rust
pub struct FlakinessScore {
    pub test_name: TestName,
    pub probability_flaky: f64,
    pub confidence: f64,
    pub pass_rate: f64,
    pub fail_rate: f64,
    pub total_runs: u64,
    pub consecutive_failures: u32,
    pub last_updated: DateTime<Utc>,
    pub bayesian_params: BayesianParams,
}
```

| Field | Type | Range | Description |
|-------|------|-------|-------------|
| `probability_flaky` | `f64` | [0.0, 1.0] | Posterior mean P(failure) |
| `confidence` | `f64` | [0.0, 1.0] | 1 - credible interval width (higher = more certain) |
| `pass_rate` | `f64` | [0.0, 1.0] | Fraction of runs that passed |
| `fail_rate` | `f64` | [0.0, 1.0] | Fraction of runs that failed (= 1 - pass_rate) |
| `total_runs` | `u64` | — | Number of non-ignored executions |
| `consecutive_failures` | `u32` | — | Trailing failure streak count |

### `BayesianParams`

Full Bayesian computation state stored for auditability.

```rust
pub struct BayesianParams {
    pub alpha: f64,
    pub beta: f64,
    pub posterior_mean: f64,
    pub posterior_variance: f64,
    pub credible_interval_lower: f64,
    pub credible_interval_upper: f64,
}
```

| Field | Description |
|-------|-------------|
| `alpha` | Beta distribution shape parameter (prior + failures) |
| `beta` | Beta distribution shape parameter (prior + passes) |
| `posterior_mean` | `alpha / (alpha + beta)` |
| `posterior_variance` | `(alpha * beta) / ((alpha + beta)^2 * (alpha + beta + 1))` |
| `credible_interval_lower` | 2.5th percentile of posterior Beta distribution |
| `credible_interval_upper` | 97.5th percentile of posterior Beta distribution |

### `FlakinessCategory`

Human-readable classification of flakiness severity.

```rust
pub enum FlakinessCategory {
    Stable,
    Occasional,
    Moderate,
    Frequent,
    Critical,
}
```

| Category | Score Range | Console Color |
|----------|-------------|---------------|
| Stable | < 0.01 | Green |
| Occasional | 0.01 – 0.05 | Yellow |
| Moderate | 0.05 – 0.15 | Orange |
| Frequent | 0.15 – 0.30 | Red |
| Critical | >= 0.30 | Dark Red |

**Methods:**
- `from_score(score: f64) -> Self` — classify a probability value
- `label(&self) -> &'static str` — human-readable label

## Sessions

### `ActiveSession`

Represents a running test session. Created via `start()`, consumed by `into_run_session()` to produce a storable `RunSession`. Ownership semantics prevent double-finish bugs.

```rust
pub struct ActiveSession { /* private fields */ }
```

**Methods:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `start` | `fn start(commit_hash: &str, branch: &str) -> Self` | Create a new session |
| `id` | `fn id(&self) -> &Uuid` | Get session UUID |
| `into_run_session` | `fn into_run_session(self) -> RunSession` | Convert to storable form (consuming) |
| `to_run_session` | `fn to_run_session(&self) -> RunSession` | Convert to storable form (borrowing) |

### `RunSession`

A session record suitable for storage. May represent a running or completed session.

```rust
pub struct RunSession {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub test_count: u32,
    pub flaky_count: u32,
    pub commit_hash: String,
    pub branch: String,
}
```

### `QuarantineEntry`

A quarantined test record.

```rust
pub struct QuarantineEntry {
    pub test_name: TestName,
    pub quarantined_at: DateTime<Utc>,
    pub reason: String,
    pub flakiness_score: f64,
    pub auto_quarantined: bool,
}
```

## Analysis

### `TrendDirection`

Direction of flakiness change over time.

```rust
pub enum TrendDirection {
    Improving,
    Stable,
    Degrading,
}
```

A delta exceeding 0.05 (5%) triggers `Improving` (decreased flakiness) or `Degrading` (increased flakiness). Otherwise `Stable`.

### `TrendSummary`

Trend analysis result comparing recent vs. historical flakiness.

```rust
pub struct TrendSummary {
    pub test_name: TestName,
    pub direction: TrendDirection,
    pub recent_score: f64,
    pub previous_score: f64,
    pub score_delta: f64,
    pub window_runs: u64,
}
```

### `FailurePattern`

A detected failure pattern with correlation strength.

```rust
pub struct FailurePattern {
    pub pattern_type: PatternType,
    pub occurrences: u32,
    pub correlation: f64,
    pub examples: Vec<String>,
}
```

### `PatternType`

Classification of detected failure patterns.

```rust
pub enum PatternType {
    TimeOfDay,
    Environmental,
    Random,
}
```

| Variant | Trigger | Description |
|---------|---------|-------------|
| `TimeOfDay` | Failure concentration > 3x expected in a specific hour | Failures cluster at a particular time |
| `Environmental` | CI vs. local failure rate difference > 15% | Environment-specific failures |
| `Random` | No pattern detected | Failures appear randomly distributed |
