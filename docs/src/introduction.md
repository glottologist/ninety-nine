# cargo ninety-nine

<div align="center">
  <img src="../media/99-medium-res.png" alt="cargo ninety-nine logo" width="200"/>
</div>

**A cargo plugin for finding and tracking flaky tests in Rust projects using Bayesian inference.**

Flaky tests — tests that sometimes pass and sometimes fail without code changes — erode trust in your test suite, waste CI time, and mask real regressions. `cargo ninety-nine` runs each test multiple times, applies Bayesian statistical analysis to compute a flakiness probability, and tracks results over time in a local SQLite database.

## Key Features

- **Bayesian flakiness detection** — computes posterior probability of flakiness using Beta distribution, not just pass/fail ratios
- **Pattern analysis** — detects time-of-day and environmental (CI vs local) failure patterns
- **Trend tracking** — monitors whether tests are improving, stable, or degrading over time
- **Quarantine management** — manually or automatically quarantine flaky tests that exceed thresholds
- **Multiple export formats** — JUnit XML, HTML reports, and CSV for integration with other tools
- **CI workflow generation** — generates ready-to-use GitHub Actions and GitLab CI configurations
- **Persistent storage** — SQLite database with WAL mode for concurrent access, automatic data retention

## Quick Example

```bash
# Initialize configuration
cargo ninety-nine init

# Run flaky test detection (10 iterations per test)
cargo ninety-nine detect -n 10

# Check a specific test's history
cargo ninety-nine status tests::my_flaky_test

# Export results as JUnit XML
cargo ninety-nine export junit results.xml
```

## How It Works

1. **Discover** — compiles test binaries and lists all test cases
2. **Execute** — runs each test N times with configurable concurrency and timeouts
3. **Analyze** — applies Bayesian inference to compute P(flaky) with credible intervals
4. **Report** — displays results with category labels (Stable, Occasional, Moderate, Frequent, Critical)
5. **Store** — persists scores and run history for trend analysis across sessions
