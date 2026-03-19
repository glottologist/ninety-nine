# Running Tests

The `test` subcommand is the core functionality -- it discovers, runs, and analyzes tests for flakiness.

## Basic Usage

```bash
# Run all tests 10 times each (default)
cargo ninety-nine test

# Filter to specific tests by name pattern
cargo ninety-nine test "my_module::tests"

# 50 iterations with 99% confidence threshold
cargo ninety-nine test -n 50 --confidence 0.99
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `<filter_expr>` | none | Filter expression -- a test name regex or [DSL expression](./filter-dsl.md) |
| `-n, --iterations` | 10 | Number of times to run each test |
| `--confidence` | 0.95 | Confidence threshold for classifying a test as flaky |

## Filter Expressions

The positional argument accepts either a plain regex pattern or a full [filter DSL](./filter-dsl.md) expression:

```bash
# Regex pattern matching test names
cargo ninety-nine test "my_module::tests"

# DSL: only tests marked flaky that are not quarantined
cargo ninety-nine test "flaky & !quarantined"

# DSL: tests in a specific package
cargo ninety-nine test "package(my_crate)"

# DSL: combine predicates
cargo ninety-nine test "test(network) & kind(test) & !quarantined"
```

See the [Filter DSL guide](./filter-dsl.md) for the full syntax reference.

## What Happens During a Run

1. **Binary discovery** -- runs `cargo test --no-run --message-format json-render-diagnostics` to compile and locate test binaries
2. **Test listing** -- executes each binary with `--list --format terse` to enumerate test cases
3. **Filtering** -- evaluates the filter expression (if provided) against each test's metadata
4. **Parallel execution** -- runs each matched test individually N times using `--exact --nocapture` flags
5. **Bayesian scoring** -- computes P(flaky) using Beta distribution with uniform prior
6. **Storage** -- writes results to SQLite database for trend tracking
7. **Reporting** -- displays results sorted by flakiness probability

## Understanding the Output

### Console Report

```
Flaky Test Detection Report

Test                                               Runs     Pass%   P(flaky)     Category
--------------------------------------------------------------------------------------------
tests::race_condition                                10     60.0%      33.3%     Frequent
tests::timing_dependent                              10     80.0%      16.7%     Moderate
tests::stable_test                                   10    100.0%       1.0%       Stable
```

### Flakiness Categories

| Category | P(flaky) Range | Meaning |
|----------|---------------|---------|
| Stable | < 1% | Reliably passes |
| Occasional | 1% - 5% | Rare failures, may be acceptable |
| Moderate | 5% - 15% | Notable flakiness, worth investigating |
| Frequent | 15% - 30% | Significant problem |
| Critical | > 30% | Severely broken, likely a bug |

## Post-Run Analysis

After the main report, the tool displays:

- **Detected Patterns** -- time-of-day or environmental correlations in failure data
- **Degrading Trends** -- tests whose failure rate has increased between recent and previous runs

## Auto-Quarantine

If `quarantine.auto_quarantine = true` in config, tests exceeding the configured thresholds are automatically quarantined after detection. See [Quarantine Management](./quarantine.md).

## Verbose Mode

Use `-v` for per-test progress output instead of the progress bar:

```bash
cargo ninety-nine test -v
```

## Summary Mode

Set `reporting.console.summary_only = true` in config to show only aggregate counts:

```
Summary: 42 tests, 3 flaky, 39 stable
```
