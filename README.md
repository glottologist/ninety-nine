<p align="center">
  <img src="docs/media/99-medium-res.png" alt="cargo ninety-nine logo" width="200"/>
</p>

<h1 align="center">cargo ninety-nine</h1>

<p align="center">
  <strong>A cargo plugin for finding and tracking flaky tests in Rust projects using Bayesian inference.</strong>
</p>

<p align="center">
  <a href="https://crates.io/crates/cargo-ninety-nine"><img src="https://img.shields.io/crates/v/cargo-ninety-nine.svg" alt="crates.io"/></a>
  <a href="https://docs.rs/cargo-ninety-nine"><img src="https://docs.rs/cargo-ninety-nine/badge.svg" alt="docs.rs"/></a>
  <a href="https://github.com/glottologist/ninety-nine/blob/main/LICENSE"><img src="https://img.shields.io/crates/l/cargo-ninety-nine.svg" alt="license"/></a>
</p>

---

Flaky tests -- tests that sometimes pass and sometimes fail without code changes -- erode trust in your test suite, waste CI time, and mask real regressions. `cargo ninety-nine` runs each test multiple times, applies Bayesian statistical analysis to compute a flakiness probability, and tracks results over time.

## Features

- **Bayesian flakiness detection** -- computes posterior probability of flakiness using Beta distribution, not just pass/fail ratios
- **Interactive TUI** -- terminal interface with scrollable tables, category filtering, sort cycling, and detail drill-down
- **Filter DSL** -- expressive query language to target tests by name, package, binary, kind, or flakiness status
- **Pattern analysis** -- detects time-of-day and environmental (CI vs local) failure patterns
- **Trend tracking** -- monitors whether tests are improving, stable, or degrading over time
- **Duration regression detection** -- flags tests whose execution time has significantly increased
- **Quarantine management** -- manually or automatically quarantine flaky tests that exceed thresholds
- **Multiple export formats** -- JUnit XML, HTML reports, CSV, and JSON
- **CI workflow generation** -- ready-to-use GitHub Actions and GitLab CI configurations
- **Dual storage backends** -- SQLite (default, zero-config) or PostgreSQL for shared environments

## Quick Start

```bash
# Install
cargo install cargo-ninety-nine

# Initialize configuration
cargo ninety-nine init

# Run flaky test detection (10 iterations per test)
cargo ninety-nine test -n 10

# Run only flaky, non-quarantined tests
cargo ninety-nine test "flaky & !quarantined"

# Check a specific test's history
cargo ninety-nine status tests::my_flaky_test

# Browse all scores interactively
cargo ninety-nine status

# View session history
cargo ninety-nine history

# Export results as JSON
cargo ninety-nine export json results.json
```

## How It Works

1. **Discover** -- compiles test binaries and lists all test cases
2. **Filter** -- applies filter expressions to select which tests to run
3. **Execute** -- runs each test N times with configurable concurrency and timeouts
4. **Analyse** -- applies Bayesian inference to compute P(flaky) with credible intervals
5. **Report** -- displays results interactively or as text, with category labels (Stable, Occasional, Moderate, Frequent, Critical)
6. **Store** -- persists scores and run history for trend analysis across sessions

## Commands

| Command | Purpose |
|---------|---------|
| `test` | Run tests repeatedly and compute flakiness scores |
| `status` | Browse current flakiness scores (interactive TUI) |
| `history` | Browse past detection sessions (interactive TUI) |
| `init` | Create a default `.ninety-nine.toml` configuration file |
| `export` | Export results to JUnit XML, HTML, CSV, or JSON |
| `quarantine` | Manage test quarantine (list, add, remove) |
| `ci` | Generate CI workflow files |

Pass `--non-interactive` (`-N`) to disable the TUI for CI pipelines or scripted use.

## Configuration

`cargo ninety-nine init` creates a `.ninety-nine.toml` in your project root. Key settings:

```toml
[detection]
min_runs = 10                  # iterations per test
confidence_threshold = 0.95    # Bayesian confidence required
parallel_runs = 3              # concurrent test executions

[quarantine]
enabled = true
auto_quarantine = false        # set true to auto-quarantine flaky tests

[storage]
backend = "Sqlite"             # or "Postgres"
retention_days = 90
```

See the [full documentation](https://glottologist.github.io/ninety-nine/) for all configuration options.

## Documentation

Full documentation is available at **[glottologist.github.io/ninety-nine](https://glottologist.github.io/ninety-nine/)**, covering:

- [Getting started](https://glottologist.github.io/ninety-nine/getting-started/installation.html)
- [Interactive TUI guide](https://glottologist.github.io/ninety-nine/guides/interactive-tui.html)
- [Filter DSL reference](https://glottologist.github.io/ninety-nine/guides/filter-dsl.html)
- [API reference](https://glottologist.github.io/ninety-nine/api/types.html)
- [Architecture](https://glottologist.github.io/ninety-nine/reference/architecture.html)

## Requirements

- Rust 1.85+
- `cargo test` or `cargo-nextest` (auto-detected)

## License

MIT
