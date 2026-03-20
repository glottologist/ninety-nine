# Analysis API

The analysis module provides three capabilities: failure rate computation, trend detection, and failure pattern recognition.

## Failure Rate

### `failure_rate`

```rust
pub fn failure_rate(runs: &[&TestRun]) -> f64
```

Computes the fraction of runs that resulted in a failure outcome.

| Outcome | Counted as |
|---------|------------|
| `Passed` | Non-failure |
| `Ignored` | Non-failure |
| `Failed` | Failure |
| `Panic` | Failure |
| `Timeout` | Failure |

**Returns:** A value in `[0.0, 1.0]`. Returns `0.0` for empty input.

## Trend Detection

### `calculate_trend`

```rust
pub fn calculate_trend(
    test_name: &str,
    runs: &[TestRun],
    window: u32,
) -> Option<TrendSummary>
```

Analyzes the direction of flakiness change by comparing recent runs against historical runs.

| Parameter | Type | Description |
|-----------|------|-------------|
| `test_name` | `&str` | Name of the test being analyzed |
| `runs` | `&[TestRun]` | Runs to analyze (newest first not required) |
| `window` | `u32` | Maximum number of runs to consider |

**Algorithm:**

1. Takes up to `window` most recent runs
2. Requires a minimum of 4 runs; returns `None` otherwise
3. Splits runs at the midpoint into "recent" (first half) and "previous" (second half)
4. Computes failure rate for each half
5. Classifies direction based on the delta:
   - Delta < -0.05 (5% improvement): `Improving`
   - Delta > +0.05 (5% regression): `Degrading`
   - Otherwise: `Stable`

**Returns:** `Some(TrendSummary)` with direction, scores, and delta, or `None` if insufficient data.

## Duration Regression

### `detect_duration_regressions`

```rust
pub fn detect_duration_regressions(
    test_name: &str,
    runs: &[TestRun],
    min_history: usize,
    threshold_std_devs: f64,
) -> Option<DurationRegression>
```

Detects if the most recent test run is significantly slower than historical runs.

| Parameter | Type | Description |
|-----------|------|-------------|
| `test_name` | `&str` | Name of the test |
| `runs` | `&[TestRun]` | Historical runs (at least `min_history` required) |
| `min_history` | `usize` | Minimum number of historical runs required |
| `threshold_std_devs` | `f64` | Number of standard deviations above mean to trigger regression |

**Algorithm:**

1. Returns `None` if fewer than `min_history` runs
2. Computes mean and standard deviation of all run durations
3. Returns `None` if standard deviation is near zero (all durations constant)
4. Checks if the latest run duration exceeds `mean + (threshold × std_dev)`
5. Returns `DurationRegression` with deviation statistics

### `DurationRegression`

```rust
pub struct DurationRegression {
    pub test_name: String,
    pub current_ms: i64,
    pub mean_ms: i64,
    pub std_dev_ms: i64,
    pub deviation_factor: f64,
}
```

| Field | Description |
|-------|-------------|
| `current_ms` | Duration of the latest run in milliseconds |
| `mean_ms` | Historical mean duration in milliseconds |
| `std_dev_ms` | Historical standard deviation in milliseconds |
| `deviation_factor` | How many standard deviations above mean: `(current - mean) / std_dev` |

## Pattern Detection

### `detect_patterns`

```rust
pub fn detect_patterns(runs: &[TestRun]) -> Vec<FailurePattern>
```

Scans test runs for recurring failure patterns. Returns all detected patterns.

**Detection strategies:**

#### Time-of-Day Pattern

Bins all failures by hour (0–23). If any hour contains more than 3x the expected random concentration, a `TimeOfDay` pattern is reported.

- Correlation value: ratio of peak-hour failures to total failures
- Examples: formatted as `"Hour HH: N failures"` for the peak hour

#### Environmental Pattern

Compares failure rates between CI and local environments. If the difference exceeds 15%, an `Environmental` pattern is reported.

- Correlation value: absolute difference between CI and local failure rates
- Examples: formatted as `"CI failure rate: X%, local: Y%"`

#### Random Fallback

If no time-of-day or environmental pattern is detected, a `Random` pattern is returned with correlation `0.0`, indicating failures appear uniformly distributed.

## Usage Example

```rust
use cargo_ninety_nine::analysis::{calculate_trend, detect_patterns};
use cargo_ninety_nine::analysis::duration::detect_duration_regressions;

// Trend analysis
if let Some(trend) = calculate_trend("tests::my_test", &runs, 100) {
    println!("Trend: {} (delta: {:.2})", trend.direction, trend.score_delta);
}

// Duration regression
if let Some(reg) = detect_duration_regressions("tests::my_test", &runs, 5, 2.0) {
    println!("SLOW: {}ms vs mean {}ms ({:.1}x std_dev)",
        reg.current_ms, reg.mean_ms, reg.deviation_factor);
}

// Pattern detection
for pattern in detect_patterns(&runs) {
    println!("Pattern: {} (correlation: {:.2})", pattern.pattern_type, pattern.correlation);
}
```
