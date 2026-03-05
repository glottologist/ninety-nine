# Detecting Flaky Tests

The `detect` subcommand is the core functionality — it discovers, runs, and analyzes tests.

## Basic Usage

```bash
# Run all tests 10 times each (default)
cargo ninety-nine detect

# Filter to specific tests
cargo ninety-nine detect "my_module::tests"

# 50 iterations with 99% confidence threshold
cargo ninety-nine detect -n 50 --confidence 0.99
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `<filter>` | none | Test name filter pattern (substring match) |
| `-n, --iterations` | 10 | Number of times to run each test |
| `--confidence` | 0.95 | Confidence threshold for classifying a test as flaky |

## What Happens During Detection

1. **Binary discovery** — runs `cargo test --no-run --message-format json-render-diagnostics` to compile and locate test binaries
2. **Test listing** — executes each binary with `--list --format terse` to enumerate test cases
3. **Parallel execution** — runs each test individually N times using `--exact --nocapture` flags
4. **Bayesian scoring** — computes P(flaky) using Beta distribution with uniform prior
5. **Storage** — writes results to SQLite database for trend tracking
6. **Reporting** — displays results sorted by flakiness probability

## Understanding the Output

### Console Report

```
Flaky Test Detection Report

Test                                               Runs     Pass%   P(flaky)     Category
--------------------------------------------------------------------------------------------
tests::timing_dependent                              10     80.0%      16.7%     Moderate
tests::race_condition                                 10     60.0%      33.3%     Frequent
tests::stable_test                                    10    100.0%       1.0%       Stable
```

### Flakiness Categories

| Category | P(flaky) Range | Meaning |
|----------|---------------|---------|
| Stable | < 1% | Reliably passes |
| Occasional | 1% - 5% | Rare failures, may be acceptable |
| Moderate | 5% - 15% | Notable flakiness, worth investigating |
| Frequent | 15% - 30% | Significant problem |
| Critical | > 30% | Severely broken, likely a bug |

## Post-Detection Analysis

After the main report, the tool displays:

- **Detected Patterns** — time-of-day or environmental correlations in failure data
- **Degrading Trends** — tests whose failure rate has increased between recent and previous runs

## Auto-Quarantine

If `quarantine.auto_quarantine = true` in config, tests exceeding the configured thresholds are automatically quarantined after detection. See [Quarantine Management](./quarantine.md).

## Verbose Mode

Use `-v` for per-test progress output instead of the progress bar:

```bash
cargo ninety-nine detect -v
```

## Summary Mode

Set `reporting.console.summary_only = true` in config to show only aggregate counts:

```
Summary: 42 tests, 3 flaky, 39 stable
```
