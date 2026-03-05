# Quarantine Management

Quarantine allows you to mark flaky tests for tracking and review. Quarantined tests are stored in the SQLite database.

## List Quarantined Tests

```bash
cargo ninety-nine quarantine list
```

Output shows test name, flakiness score, whether it was auto-quarantined, and when:

```
Quarantined Tests

Test                                                  Score       Auto Since
------------------------------------------------------------------------------------------------
tests::race_condition                                  33.3%        yes 2026-03-05 14:30:00
tests::timing_test                                     20.0%         no 2026-03-04 10:15:00
```

## Add a Test to Quarantine

```bash
cargo ninety-nine quarantine add "tests::flaky_test" --reason "depends on network timing"
```

The `--reason` flag defaults to `"manually quarantined"` if omitted.

## Remove a Test from Quarantine

```bash
cargo ninety-nine quarantine remove "tests::flaky_test"
```

## Auto-Quarantine

Enable automatic quarantine in `.ninety-nine.toml`:

```toml
[quarantine]
enabled = true
auto_quarantine = true

[quarantine.threshold]
consecutive_failures = 3
failure_rate = 0.20
flakiness_score = 0.15
```

When `auto_quarantine = true`, any test detected as flaky (by the Bayesian detector) that also exceeds **any** of the three thresholds is automatically quarantined after a `detect` run.

### Threshold Fields

| Field | Default | Description |
|-------|---------|-------------|
| `consecutive_failures` | 3 | Number of consecutive failures at the tail of the run |
| `failure_rate` | 0.20 | Overall failure rate (failures / total runs) |
| `flakiness_score` | 0.15 | Bayesian P(flaky) posterior mean |

A test is auto-quarantined if `is_flaky(score) AND (exceeds_score OR exceeds_failures OR exceeds_rate)`.

## JSON Output

```bash
cargo ninety-nine quarantine list --output json
```
