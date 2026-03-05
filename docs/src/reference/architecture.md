# Architecture

## Module Overview

```
cargo-ninety-nine
├── src/
│   ├── main.rs          # Entry point, command dispatch, orchestration
│   ├── lib.rs           # Module re-exports
│   ├── error.rs         # NinetyNineError enum (thiserror)
│   ├── cli/
│   │   ├── mod.rs       # CLI argument definitions (clap derive)
│   │   ├── output.rs    # Console/JSON output formatting
│   │   └── export.rs    # JUnit XML, HTML, CSV export
│   ├── config/
│   │   ├── mod.rs       # Config loading from TOML
│   │   └── model.rs     # Config struct definitions with defaults
│   ├── runner/
│   │   ├── mod.rs       # NativeRunner, RunnerBackend dispatch
│   │   ├── binary.rs    # Test binary discovery via cargo_metadata
│   │   ├── listing.rs   # Test case enumeration from binaries
│   │   ├── executor.rs  # Per-test execution with timeout/retry
│   │   └── detection.rs # Runner availability check (nextest/cargo)
│   ├── detector/
│   │   ├── mod.rs       # Detector re-exports
│   │   └── bayesian.rs  # Bayesian flakiness scoring
│   ├── analysis/
│   │   ├── mod.rs       # Analysis re-exports
│   │   ├── pattern.rs   # Failure pattern detection
│   │   └── trend.rs     # Trend calculation (improving/stable/degrading)
│   ├── storage/
│   │   ├── mod.rs       # Storage trait definition
│   │   ├── sqlite.rs    # SQLite implementation (rusqlite)
│   │   └── schema.rs    # SQL migration definitions
│   ├── types/
│   │   ├── mod.rs       # Type re-exports
│   │   ├── test_run.rs  # TestRun, TestOutcome, TestEnvironment
│   │   ├── flakiness.rs # FlakinessScore, BayesianParams, FlakinessCategory
│   │   ├── trend.rs     # TrendDirection, TrendSummary
│   │   ├── session.rs   # RunSession, QuarantineEntry
│   │   └── analysis.rs  # FailurePattern, PatternType
│   └── ci/
│       ├── mod.rs       # CI re-exports
│       └── workflow.rs  # GitHub Actions / GitLab CI YAML generation
```

## Data Flow

```
┌─────────────────┐
│   CLI (clap)    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐     ┌───────────────────┐
│   Config Loader │────▶│ .ninety-nine.toml  │
└────────┬────────┘     └───────────────────┘
         │
         ▼
┌─────────────────┐
│  Runner Backend │
│  ┌────────────┐ │
│  │ Binary     │ │  cargo test --no-run --message-format json
│  │ Discovery  │ │
│  └─────┬──────┘ │
│        ▼        │
│  ┌────────────┐ │  binary --list --format terse
│  │ Test       │ │
│  │ Listing    │ │
│  └─────┬──────┘ │
│        ▼        │
│  ┌────────────┐ │  binary --exact test_name --nocapture
│  │ Executor   │ │  (N iterations, timeout, retry)
│  └────────────┘ │
└────────┬────────┘
         │ Vec<TestRun>
         ▼
┌─────────────────┐
│ Bayesian        │  Beta(alpha, beta) posterior
│ Detector        │  → FlakinessScore
└────────┬────────┘
         │
    ┌────┴────┐
    ▼         ▼
┌────────┐ ┌──────────┐
│ SQLite │ │ Reporter │
│Storage │ │ (output/ │
│        │ │  export) │
└────────┘ └──────────┘
```

## Key Design Decisions

### Native Test Runner

Instead of wrapping `cargo test` or `cargo nextest` as a subprocess for the full run, the tool uses a **three-layer native pipeline**:

1. **Binary discovery** — uses `cargo_metadata` to parse `cargo test --no-run` JSON output, extracting test binary paths
2. **Test listing** — executes each binary with `--list --format terse` to get individual test names
3. **Per-test execution** — runs each test individually via `duct`, using `--exact` for isolation

This gives precise per-test timing, retry control, and outcome classification without parsing human-readable test output.

### Subprocess Management

Test execution uses `duct` for subprocess control with:
- **Poll-based timeout** — `try_wait()` in a loop with a 50ms polling interval, `kill()` on deadline
- **Output capture** — stdout and stderr captured for failure analysis
- **Unchecked mode** — non-zero exit codes are handled as test outcomes, not errors

### Async Runtime

The tool uses `tokio` for:
- Parallel test listing across multiple test binaries (semaphore-controlled)
- `spawn_blocking` for CPU-bound test execution
- The main event loop via `#[tokio::main]`

### Storage

SQLite with WAL mode provides:
- Concurrent reads during detection
- Schema versioning via `PRAGMA user_version`
- Automatic directory creation for the database path
- Configurable retention with `purge_older_than()`

### Error Handling

All errors flow through `NinetyNineError` (thiserror), with variants for each subsystem. The `?` operator propagates errors to the top-level `main()` handler, which prints the error and exits with code 1.
