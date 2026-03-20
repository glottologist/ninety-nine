# Best Practices

Guidance for getting the most out of `cargo-ninety-nine` in real-world workflows.

## Iteration Strategy

### Choosing `min_runs`

The number of iterations directly impacts detection accuracy:

| Iterations | Use Case | Confidence |
|-----------|----------|------------|
| 5–10 | Quick smoke check | Low — high false positive rate |
| 10–20 | Daily development | Moderate — good for known-flaky tests |
| 20–50 | Pre-merge gate | High — reliable classification |
| 50–100 | Baseline establishment | Very high — suitable for initial assessment |

> **Note:** The Bayesian detector uses a uniform Beta(1,1) prior. With fewer than 10 runs, the prior dominates the posterior and scores are unreliable. The default `min_runs = 10` is a minimum for meaningful results.

### Building a Baseline

When first adopting `cargo-ninety-nine`, establish a baseline:

```bash
# Run with higher iterations to build confidence
cargo ninety-nine test -n 50

# Review the full status
cargo ninety-nine status
```

Subsequent runs build on the historical data, so daily runs with fewer iterations (10–20) are sufficient once the baseline exists.

## Quarantine Strategy

### When to Quarantine

Quarantine a test when:
- It blocks CI pipelines with intermittent failures
- It has a `Moderate` or higher flakiness category (>= 0.05)
- It has 3+ consecutive failures with no code changes

Do **not** quarantine a test when:
- It consistently fails — that is a real bug, not flakiness
- It just started failing after a recent change — investigate the change first
- The flakiness score is `Occasional` (<0.05) — monitor it instead

### Auto-Quarantine Thresholds

If enabling auto-quarantine, tune the thresholds to avoid over-quarantining:

```toml
[quarantine]
auto_quarantine = true

[quarantine.threshold]
consecutive_failures = 5    # stricter than default 3
failure_rate = 0.30         # stricter than default 0.20
flakiness_score = 0.20      # stricter than default 0.15
```

Start strict and relax gradually based on your team's tolerance.

### Quarantine Review Cadence

Set `max_quarantine_days` to force periodic review:

```toml
[quarantine]
max_quarantine_days = 14  # review every 2 weeks
```

During review:
1. `cargo ninety-nine quarantine list` — see all quarantined tests
2. Re-run quarantined tests: `cargo ninety-nine test "quarantined()"`
3. Remove fixed tests: `cargo ninety-nine quarantine remove <test_name>`

## CI Integration

### Recommended CI Workflow

Run `cargo-ninety-nine` as a separate CI job that does not block merges:

```yaml
# GitHub Actions example
flaky-detection:
  runs-on: ubuntu-latest
  if: github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - run: cargo install cargo-ninety-nine
    - run: cargo ninety-nine test -n 20
    - run: cargo ninety-nine export junit report.xml
    - uses: actions/upload-artifact@v4
      with:
        name: flaky-report
        path: report.xml
```

> **Warning:** Avoid running flaky detection on every push. It adds significant CI time and produces noise. Schedule it nightly or weekly.

### Shared Storage in CI

For teams wanting to accumulate results across CI runs, use PostgreSQL:

```toml
[storage]
backend = "Postgres"

[storage.postgres]
connection_string = "host=db.internal dbname=ninety_nine user=ci password=${NN_PG_PASSWORD}"
pool_size = 4
```

Pass the password via environment variable in CI secrets.

## Performance Tuning

### Parallel Execution

The `parallel_runs` setting controls how many tests execute concurrently:

```toml
[detection]
parallel_runs = 4  # default: 3
```

**Guidelines:**
- Local development: set to number of CPU cores / 2
- CI (shared runners): set to 1–2 to avoid resource contention
- Dedicated CI machines: set to CPU count

### Reducing Test Discovery Time

Test discovery runs `cargo test --no-run`, which may trigger a full build. To speed this up:
- Keep your build cache warm (`target/` directory)
- Use incremental compilation
- Consider running detection on a subset: `cargo ninety-nine test "package(critical_module)"`

### Database Performance

**SQLite** (default):
- Excellent for single-machine use
- WAL mode is enabled automatically for concurrent reads
- Keep the database on a local SSD, not a network filesystem

**PostgreSQL**:
- Better for shared/team use and large datasets
- Tune `pool_size` based on concurrent CI jobs
- Use connection pooling (PgBouncer) for high-concurrency environments

## Interpreting Results

### Understanding Bayesian Scores

| Score | Meaning | Action |
|-------|---------|--------|
| < 0.01 | Stable — no flakiness detected | No action needed |
| 0.01 – 0.05 | Occasional — rare failures | Monitor, investigate if trending up |
| 0.05 – 0.15 | Moderate — noticeable flakiness | Investigate root cause, consider quarantine |
| 0.15 – 0.30 | Frequent — regular failures | Fix or quarantine immediately |
| >= 0.30 | Critical — failing more than passing | Urgent fix required |

### Acting on Patterns

| Pattern | Common Causes | Remediation |
|---------|--------------|-------------|
| Time-of-day | Scheduled jobs competing for resources, time-sensitive assertions | Remove time dependencies, use `faketime` in tests |
| Environmental | Missing dependencies in CI, different OS behavior | Ensure CI mirrors local dev environment, use containers |
| Random | Race conditions, shared mutable state, non-deterministic ordering | Add synchronization, use deterministic seeds, isolate test state |

### Confidence and Sample Size

Low confidence means the credible interval is wide — the true flakiness probability could be much higher or lower than the point estimate. Before acting on a score:

- **High confidence (>0.95):** Act on the score directly
- **Moderate confidence (0.80–0.95):** Run more iterations to confirm
- **Low confidence (<0.80):** Too few runs to draw conclusions, need more data

## Migrating from SQLite to PostgreSQL

1. Export current data:
   ```bash
   cargo ninety-nine export json current-data.json
   ```

2. Update configuration:
   ```toml
   [storage]
   backend = "Postgres"

   [storage.postgres]
   connection_string = "host=localhost dbname=ninety_nine"
   pool_size = 4
   ```

3. Run a fresh detection pass to populate the new database:
   ```bash
   cargo ninety-nine test -n 20
   ```

> **Note:** There is no automatic migration tool. Historical run data is not transferred — only flakiness scores and quarantine state can be preserved via JSON export. The new database will build fresh statistics from subsequent runs.

## Team Workflows

### Flaky Test Triage

Establish a regular triage process:

1. **Weekly:** Review `cargo ninety-nine status` output
2. **Per-sprint:** Assign `Moderate`+ flaky tests to developers
3. **Per-release:** Clear all `Frequent`/`Critical` tests before release

### Filter Expressions for Triage

```bash
# Show only flaky tests
cargo ninety-nine test "flaky()"

# Focus on a specific package
cargo ninety-nine test "package(api) & flaky()"

# Exclude already-quarantined tests
cargo ninety-nine test "flaky() & !quarantined()"
```

### Using JSON Output for Automation

```bash
# Export status as JSON for dashboards
cargo ninety-nine --output json status > flaky-status.json

# Parse with jq
cat flaky-status.json | jq '.[] | select(.probability_flaky > 0.1)'
```
