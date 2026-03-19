# cargo ninety-nine

<div align="center">
  <img src="../media/99-medium-res.png" alt="cargo ninety-nine logo" width="200"/>
</div>

**A cargo plugin for finding and tracking flaky tests in Rust projects using Bayesian inference.**

Flaky tests -- tests that sometimes pass and sometimes fail without code changes -- erode trust in your test suite, waste CI time, and mask real regressions. `cargo ninety-nine` runs each test multiple times, applies Bayesian statistical analysis to compute a flakiness probability, and tracks results over time in a local SQLite database.

## Key Features

- **Bayesian flakiness detection** -- computes posterior probability of flakiness using Beta distribution, not just pass/fail ratios
- **Filter DSL** -- expressive query language to target tests by name, package, binary, kind, or flakiness status
- **Pattern analysis** -- detects time-of-day and environmental (CI vs local) failure patterns
- **Trend tracking** -- monitors whether tests are improving, stable, or degrading over time
- **Quarantine management** -- manually or automatically quarantine flaky tests that exceed thresholds
- **Multiple export formats** -- JUnit XML, HTML reports, CSV, and JSON for integration with other tools
- **CI workflow generation** -- generates ready-to-use GitHub Actions and GitLab CI configurations
- **Persistent storage** -- SQLite database with WAL mode for concurrent access, automatic data retention

## Quick Example

```bash
# Initialize configuration
cargo ninety-nine init

# Run flaky test detection (10 iterations per test)
cargo ninety-nine test -n 10

# Run only flaky, non-quarantined tests
cargo ninety-nine test "flaky & !quarantined"

# Check a specific test's history
cargo ninety-nine status tests::my_flaky_test

# Export results as JSON
cargo ninety-nine export json results.json
```

## How It Works

1. **Discover** -- compiles test binaries and lists all test cases
2. **Filter** -- applies filter expressions to select which tests to run
3. **Execute** -- runs each test N times with configurable concurrency and timeouts
4. **Analyze** -- applies Bayesian inference to compute P(flaky) with credible intervals
5. **Report** -- displays results with category labels (Stable, Occasional, Moderate, Frequent, Critical)
6. **Store** -- persists scores and run history for trend analysis across sessions
