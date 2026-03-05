# Analysis & Patterns

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

If failures exist but no specific pattern is detected, a Random pattern is reported. This is the fallback — it means failures occur but don't correlate with time or environment.

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

- `cargo ninety-nine status <test_name>` — shows trend direction and delta
- After `detect` — degrading trends are highlighted in the post-detection analysis
