# Architecture

## Module Overview

```
cargo-ninety-nine
src/
  main.rs              Entry point, command dispatch
  lib.rs               Module re-exports
  env.rs               Git info, environment, CI provider detection
  orchestrator.rs      Test execution pipeline, session lifecycle, auto-quarantine
  error.rs             NinetyNineError enum (thiserror)
  analysis/
    mod.rs             Shared failure_rate() helper
    duration.rs        Duration regression detection
    pattern.rs         Failure pattern detection (time-of-day, environmental)
    trend.rs           Trend calculation (improving/stable/degrading)
  ci/
    mod.rs             CI re-exports
    workflow.rs        GitHub Actions / GitLab CI YAML generation
  cli/
    mod.rs             CLI argument definitions (clap derive)
    output.rs          Console/JSON output formatting
    export.rs          JUnit XML, HTML, CSV, JSON export
  config/
    mod.rs             Config loading from TOML
    model.rs           Config struct definitions with defaults
  detector/
    mod.rs             Detector re-exports
    bayesian.rs        Bayesian flakiness scoring
  filter/
    mod.rs             compile_filter(), build_eval_context()
    ast.rs             FilterExpr and Predicate AST nodes
    lexer.rs           Tokenizer for filter DSL
    parser.rs          Recursive descent parser
    eval.rs            Evaluator matching FilterExpr against TestMetadata
  runner/
    mod.rs             NativeRunner, RunnerBackend, execute_iterations()
    binary.rs          Test binary discovery via cargo_metadata
    listing.rs         Test case enumeration from binaries
    executor.rs        Per-test execution with timeout/retry
    detection.rs       Runner availability check
  storage/
    mod.rs             Storage trait (async), open_storage() factory
    backend.rs         StorageBackend enum with dispatch! macro
    mapping.rs         Shared row-to-domain-type conversion (RawTestRunRow, RawScoreRow)
    sqlite.rs          SQLite implementation (rusqlite, WAL mode)
    postgres.rs        PostgreSQL implementation (deadpool-postgres)
    schema.rs          SQL migration definitions
  tui/
    mod.rs             TUI entry points, event loop, terminal guard, signal handlers
    app.rs             Application state (ScoresApp, HistoryApp, Cursor, SortField)
    input.rs           Key event mapping to actions
    render.rs          Ratatui widget rendering (scores table, detail overlay, history)
  types/
    mod.rs             Type re-exports
    test_run.rs        TestRun, TestOutcome, TestEnvironment
    test_name.rs       TestName newtype
    flakiness.rs       FlakinessScore, BayesianParams, FlakinessCategory
    trend.rs           TrendDirection, TrendSummary
    session.rs         RunSession, ActiveSession, QuarantineEntry
    analysis.rs        FailurePattern, PatternType
```

## Test Execution Flow

```
CLI (clap)
    |
    v
Config Loader ---------> .ninety-nine.toml
    |
    v
Filter DSL (optional)
  lexer --> parser --> FilterExpr AST
    |
    v
Runner Backend (NativeRunner)
  +-------------------+
  | Binary Discovery  |  cargo test --no-run --message-format json
  |   (cargo_metadata)|
  +---------+---------+
            |
            v
  +-------------------+
  | Test Listing      |  binary --list --format terse
  |   (parallel via   |  (semaphore-bounded concurrency)
  |    tokio spawn)   |
  +---------+---------+
            |
            v
  +-------------------+
  | Filter Evaluation |  FilterExpr evaluated against TestMetadata
  |   (if DSL given)  |  (loads flaky/quarantined sets from storage)
  +---------+---------+
            |
            v
  +-------------------+
  | Executor          |  binary --exact test_name --nocapture
  |   (N iterations,  |  (parallel via spawn_blocking + Semaphore)
  |    timeout, retry) |
  +-------------------+
            |
            | Vec<TestRun>
            v
  +-------------------+
  | Bayesian Detector |  Beta(alpha, beta) posterior
  |   + Analysis      |  --> FlakinessScore
  |   + Duration      |  --> DurationRegression (optional)
  +-------------------+
            |
      +-----+-----+-------+
      v           v       v
  Storage      Reporter  TUI
  (SQLite/     (console/  (ratatui,
   Postgres)    JSON/      interactive
                export)    scores/history)
```

## Filter DSL Pipeline

The filter system has three stages:

1. **Lexer** (`filter/lexer.rs`) -- tokenizes the input string into `Token` values: `LParen`, `RParen`, `And` (`&`), `Or` (`|`), `Not` (`!`), and `Ident(String)`.

2. **Parser** (`filter/parser.rs`) -- recursive descent parser that builds a `FilterExpr` AST. Supports binary operators (`&`, `|`), unary negation (`!`), parenthesized grouping, and function-call predicates like `test(pattern)`, `package(name)`, `binary(name)`, `kind(lib|bin|test|example)`. Bare identifiers are resolved as keywords (`flaky`, `quarantined`, `all`) or treated as test name regex patterns.

3. **Evaluator** (`filter/eval.rs`) -- evaluates a `FilterExpr` against a `TestMetadata` struct containing the test name, package name, binary name, and binary kind. An `EvalContext` holds pre-loaded sets of flaky and quarantined test names from storage.

## Storage Abstraction

The `Storage` trait defines 13 async methods for persisting and querying test data. Two backends implement it:

- **SqliteStorage** -- uses `rusqlite` with WAL mode and `Mutex<Connection>` for thread safety. Synchronous operations run inside async method signatures. Migrations use `PRAGMA user_version`.

- **PostgresStorage** -- uses `deadpool-postgres` for connection pooling with configurable pool size and timeouts. Migrations use a `schema_migrations` table.

The `StorageBackend` enum wraps both backends and dispatches calls via a `dispatch!` macro, keeping the orchestration layer backend-agnostic.

```rust
pub enum StorageBackend {
    Sqlite(SqliteStorage),
    Postgres(PostgresStorage),
}
```

The `open_storage()` factory function reads the config to determine which backend to initialize.

## Type System

### TestName

A newtype wrapper around `String` that prevents confusion with other string fields (branch names, commit hashes, error messages). Implements `Deref<Target=str>`, `AsRef<str>`, `Display`, `From<String>`, `From<&str>`, and `PartialEq<str>`. Used in `TestRun`, `FlakinessScore`, `QuarantineEntry`, `TrendSummary`, `DurationRegression`, and `TestCase`.

### ActiveSession

Type-state pattern for session lifecycle. `ActiveSession::start()` creates a running session. `into_run_session()` consumes it to produce a storable `RunSession`, preventing double-finish at the orchestration layer. `to_run_session()` creates a storable copy without consuming the active session.

### FlakinessScore and FlakinessCategory

`FlakinessScore` holds the Bayesian posterior parameters (alpha, beta, posterior mean/variance, credible interval) alongside aggregate statistics (pass rate, fail rate, consecutive failures). `FlakinessCategory` classifies scores into five levels:

| Score Range | Category |
|-------------|----------|
| < 0.01 | Stable |
| 0.01 - 0.05 | Occasional |
| 0.05 - 0.15 | Moderate |
| 0.15 - 0.30 | Frequent |
| >= 0.30 | Critical |

## Key Design Decisions

### Native Test Runner

Instead of wrapping `cargo test` as a subprocess for the full run, the tool uses a three-layer native pipeline:

1. **Binary discovery** -- uses `cargo_metadata` to parse `cargo test --no-run` JSON output, extracting test binary paths with package name and binary kind.
2. **Test listing** -- executes each binary with `--list --format terse` to enumerate individual test names. Runs in parallel across binaries using `tokio::spawn_blocking` with semaphore-bounded concurrency.
3. **Per-test execution** -- runs each test individually via `duct`, using `--exact` for isolation. Supports configurable timeout, retry count, and retry delay.

This gives precise per-test timing, retry control, and outcome classification without parsing human-readable test output.

### Parallel Execution

Test execution uses `tokio::spawn_blocking` with a `Semaphore` to bound concurrency. The semaphore limit comes from `detection.parallel_runs` in the config (default: 3). Each test iteration acquires a permit before spawning a blocking task.

### Subprocess Management

Test execution uses `duct` for subprocess control with:
- **Poll-based timeout** -- `try_wait()` in a loop with a 50ms polling interval, `kill()` on deadline
- **Output capture** -- stdout and stderr captured for failure analysis
- **Unchecked mode** -- non-zero exit codes are handled as test outcomes, not errors

### Storage Backend Dispatch

The `StorageBackend` enum uses a `dispatch!` macro to forward all 13 trait methods to the underlying backend. This avoids 130+ lines of boilerplate match arms while keeping the dispatch zero-cost.

### Error Handling

All errors flow through `NinetyNineError` (thiserror), with variants for each subsystem: config, storage, binary discovery, test listing, runner execution, filter parsing, Postgres pool, and I/O. The `?` operator propagates errors to the top-level handler.
