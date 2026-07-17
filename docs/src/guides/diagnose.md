# Multi-phase Diagnose

`cargo ninety-nine diagnose` finds flaky tests by stressing each test binary under multi-threaded load, then re-running only stress-failing candidates in **serial isolation**.

## Phases

1. **Stress** — for each binary that contains a selected test, run the full binary N times with `--test-threads` set (no `--exact`). Failures are parsed from libtest output and intersected with the selected set.
2. **Isolation** — each candidate is run alone (`--exact`) N times, **serially** (concurrency 1, retries 0).
3. **Classify** — each candidate becomes one of:
   - **Contention** — failed under stress, always passed alone
   - **Intrinsic** — failed sometimes alone
   - **Broken** — never passed alone
4. **Record (optional)** — with `--record`, Intrinsic failures may be re-run under [rr](https://rr-project.org/) when available (Linux soft dependency).

## Contention meaning (V1)

V1 stress is **intra-binary**: each test binary is exercised multi-threaded as a whole. Cross-binary workspace races are out of scope until a later runner rewrite.

## Usage

```bash
# Default config ([diagnose] in .ninety-nine.toml)
cargo ninety-nine diagnose

# Override run counts
cargo ninety-nine diagnose --stress-runs 5 --isolation-runs 20

# Filter + optional rr recording
cargo ninety-nine diagnose "pkg::" --record --record-dir .ninety-nine/recordings

# JSON for CI
cargo ninety-nine diagnose -N --output json
```

## Configuration

```toml
[diagnose]
stress_runs = 3
isolation_runs = 10
stress_threads = 0          # 0 = host parallelism
stress_timeout_secs = 300
record = false
record_dir = ".ninety-nine/recordings"
record_attempts = 10
```

## Identity

Diagnostic rows store package + binary + test name. Bayesian scores and `test_runs` still use the short test name so existing filters and the scores TUI keep working.

## Soft CI exit

`diagnose` exits 0 after a successful run even when flaky classes are found. Fail the pipeline in a wrapper if you need a hard gate.
