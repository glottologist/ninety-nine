# Analysis and Patterns

## Pattern Detection

After running tests, `cargo ninety-nine` analyzes failure data to identify patterns that might explain why tests are flaky.

### Pattern Types

| Pattern | Detection Method |
|---------|-----------------|
| **TimeOfDay** | Failures concentrated at a specific hour (3x expected rate) |
| **Environmental** | Failure rate differs > 15% between CI and local environments |
| **Random** | Failures present but no discernible pattern (fallback) |

### Time-of-Day Detection

Requires at least 5 failures. Counts failures by hour (0-23 UTC) and computes:

```
concentration = max_hour_count / expected_per_hour
```

If `concentration >= 3.0`, a TimeOfDay pattern is reported with:
- `correlation`: `min(concentration - 1.0, 1.0)`
- `examples`: the peak hour identified

This pattern suggests timing-dependent failures (e.g., midnight log rotation, scheduled background jobs).

### Environmental Detection

Requires at least 3 CI runs and 3 local runs. Compares failure rates between environments:

```
diff = |ci_fail_rate - local_fail_rate|
```

If `diff >= 0.15`, an Environmental pattern is reported, identifying which environment has the higher failure rate. This pattern suggests resource-dependent failures (e.g., CPU count, memory limits, file system speed).

### Random Pattern

If failures exist but no specific pattern is detected, a Random pattern is reported. This is the fallback -- it means failures occur but do not correlate with time or environment.

## Trend Analysis

Trend analysis compares recent failure rates to previous failure rates within a sliding window.

### How It Works

1. Filter runs for a specific test name
2. Take up to `window_size` most recent runs (configurable, default: 100)
3. Split into two halves: **recent** (first half) and **previous** (second half)
4. Compute failure rate for each half
5. Classify the delta:

| Delta | Direction |
|-------|-----------|
| > 5% | Degrading |
| < -5% | Improving |
| within +/- 5% | Stable |

### Requirements

At least 4 runs are required for trend calculation. Fewer runs return `None`.

### Where Trends Are Shown

- `cargo ninety-nine status <test_name>` -- shows trend direction and delta
- After `test` -- degrading trends are highlighted in the post-detection analysis

## Duration Regression Analysis

Duration regression detection identifies tests whose execution time has significantly increased compared to their historical average. This helps catch performance regressions early, even when tests still pass.

### How It Works

1. Collect all historical run durations for a test (most recent first)
2. Require at least `min_history_runs` data points (configured in `[detection.duration_regression]`)
3. Compute the **mean** and **sample standard deviation** of all durations
4. Compare the **latest** duration against the historical distribution:

```
deviation_factor = (latest_duration - mean) / standard_deviation
```

5. If `deviation_factor > threshold`, a regression is reported

### Configuration

Duration regression detection is disabled by default. Enable it in `.ninety-nine.toml`:

```toml
[detection.duration_regression]
enabled = true
min_history_runs = 10

# Flag when latest run exceeds 2.5 standard deviations above the mean
[detection.duration_regression.threshold]
StdDev = 2.5
```

Two threshold variants are available:

| Variant | Description | Example |
|---------|-------------|---------|
| `StdDev(f64)` | Number of standard deviations above the mean | `StdDev = 2.0` flags tests running >2 sigma slower |
| `Multiplier(f64)` | Multiple of the historical mean | `Multiplier = 3.0` flags tests taking >3x their average |

### Output

When a regression is detected, the output includes:

| Field | Description |
|-------|-------------|
| `test_name` | Fully qualified name of the affected test |
| `current_ms` | Duration of the latest run in milliseconds |
| `mean_ms` | Historical mean duration in milliseconds |
| `std_dev_ms` | Standard deviation of historical durations |
| `deviation_factor` | How many standard deviations above the mean |

### Edge Cases

- If the standard deviation is effectively zero (all durations identical), no regression is reported regardless of the latest duration
- If fewer than `min_history_runs` runs exist, no regression check is performed
- The latest run (index 0) is included in the mean/stddev calculation along with historical runs
