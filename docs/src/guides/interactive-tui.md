# Interactive TUI

`cargo ninety-nine` includes a terminal user interface for browsing flakiness scores and session history. The TUI launches automatically when running `status`, `history`, or `test` in a terminal. Pass `--non-interactive` (or `-N`) to disable it.

## Scores View

The scores view is the main screen after running `test`, or when running `status` without a test name argument.

```
cargo ninety-nine | 42/42 tests
Filter: All | Sort: P(flaky) (desc)
+--------------------------------+------+-------+---------+----------+----------+
| Test                           | Runs | Pass% | P(flaky)| Category |Confidence|
+--------------------------------+------+-------+---------+----------+----------+
| tests::network::retry_timeout  |   20 | 75.0% |   0.250 | Frequent |     0.89 |
| tests::db::concurrent_writes   |   20 | 85.0% |   0.150 | Moderate |     0.92 |
| tests::parser::edge_cases      |   20 | 95.0% |   0.050 | Occasional|    0.97 |
| tests::math::addition          |   20 |100.0% |   0.010 | Stable   |     0.99 |
+--------------------------------+------+-------+---------+----------+----------+
j/k:nav  s:sort  r:reverse  f:filter  Enter:detail  q:quit
```

### Keybindings

| Key | Action |
|-----|--------|
| `j` / Down arrow | Move selection down |
| `k` / Up arrow | Move selection up |
| `s` | Cycle sort field (Test, Runs, Pass%, P(flaky), Category) |
| `r` | Reverse sort order |
| `f` | Cycle category filter (All, Stable, Occasional, Moderate, Frequent, Critical) |
| Enter | Drill into selected test detail |
| `q` / Esc | Quit |
| Ctrl+C | Quit |

### Sorting

Press `s` to cycle through sort fields. Press `r` to toggle ascending/descending. The current sort field and direction are shown in the filter bar.

### Filtering

Press `f` to cycle through category filters. When a filter is active, only tests in that category are shown. The count in the header updates to reflect the filtered set.

## Detail View

Press Enter on a test to open its detail overlay, showing:

- **Score summary** -- category, P(flaky), confidence, pass/fail rates, total runs
- **Bayesian parameters** -- alpha, beta, posterior mean, credible interval
- **Trend** -- direction (Improving/Stable/Degrading) with score delta
- **Failure patterns** -- correlated patterns with correlation percentage
- **Recent runs** -- last 10 runs with outcome, duration, and timestamp

Press Enter, `q`, or Esc to return to the scores list.

## History View

The history view shows past detection sessions:

```
cargo ninety-nine | 5 sessions
+--------------------+------+------+-----------------+----------+
| Date               | Tests| Flaky| Branch          | Commit   |
+--------------------+------+------+-----------------+----------+
| 2026-03-23 14:30   |   42 |    3 | main            | 9e9bce5  |
| 2026-03-22 09:15   |   42 |    2 | feature/tui     | a9f0068  |
+--------------------+------+------+-----------------+----------+
j/k:nav  q:quit
```

Navigate with `j`/`k` or arrow keys. Press `q` or Esc to quit.

## Disabling the TUI

For CI pipelines, scripts, or piped output, use `--non-interactive`:

```bash
cargo ninety-nine status --non-interactive
cargo ninety-nine history --non-interactive
cargo ninety-nine test -n 10 --non-interactive
```

This produces the same text output as previous versions.

## Terminal Requirements

The TUI uses the alternate screen buffer and raw mode via crossterm. It works in any terminal emulator that supports ANSI escape sequences. On Unix, SIGTERM and SIGHUP trigger graceful shutdown. On all platforms, the terminal state is restored on exit, including after panics.

Minimum terminal size: 60 columns by 10 rows.
