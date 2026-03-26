# Interactive TUI

`cargo ninety-nine` includes a terminal user interface for browsing flakiness scores and session history. The TUI launches automatically when running `status`, `history`, or `test` in a terminal. Pass `--non-interactive` (or `-N`) to disable it.

## Scores View

The scores view is the main screen after running `test`, or when running `status` without a test name argument. It uses bordered panels, a scrollbar, and colour-coded categories.

```
┌ Flaky Test Report ───────────────────────────────────────────┐
│ cargo ninety-nine | 42/42 tests shown                        │
└──────────────────────────────────────────────────────────────┘
Filter: All | Sort: P(flaky) (desc)
┌ Tests ──────────────────────────────────────────────────────┐^
│ Test                           Runs  Pass%  P(flaky) Cat.   ││
│                                                              ││
│ tests::network::retry_timeout    20  75.0%    0.250  Freq   ││
│ tests::db::concurrent_writes     20  85.0%    0.150  Mod    ││
│ tests::parser::edge_cases        20  95.0%    0.050  Occ    │█
│ tests::math::addition            20 100.0%    0.010  Stab   ││
│                                                              ││
└──────────────────────────────────────────────────────────────┘v
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

Press `s` to cycle through sort fields. Press `r` to toggle ascending/descending. The current sort field and direction are shown in the orange filter bar below the header.

### Filtering

Press `f` to cycle through category filters. When a filter is active, only tests in that category are shown. The count in the header updates to reflect the filtered set.

### Scrollbar

A vertical scrollbar appears on the right edge of the content panel. The scrollbar tracks the current selection position within the full list. The table viewport automatically follows the selected row, so scrolling through large lists works without manual page management.

## Detail View

Press Enter on a test to open its detail overlay with a cyan border, showing:

- **Score summary** -- category, P(flaky), confidence, pass/fail rates, total runs
- **Bayesian parameters** -- alpha, beta, posterior mean, credible interval
- **Trend** -- direction (Improving/Stable/Degrading) with score delta
- **Failure patterns** -- correlated patterns with correlation percentage
- **Recent runs** -- last 10 runs with outcome, duration, and timestamp

Press Enter, `q`, or Esc to return to the scores list.

## History View

The history view shows past detection sessions with the same bordered-panel layout:

```
┌ Session History ─────────────────────────────────────────────┐
│ cargo ninety-nine | 13 sessions                              │
└──────────────────────────────────────────────────────────────┘
┌ Sessions ───────────────────────────────────────────────────┐^
│ Date              Tests  Flaky  Branch           Commit     ││
│                                                              ││
│ 2026-03-23 14:39   1803      0  jason/add_tests  92d65c74   │█
│ 2026-03-23 11:34   1803      0  jason/add_tests  92d65c74   ││
│ 2026-03-18 18:39    105      0  main             5ee51231   ││
└──────────────────────────────────────────────────────────────┘v
j/k:nav  Enter:detail  q:quit
```

Navigate with `j`/`k` or arrow keys. Press Enter to view the test runs from that session.

### Session Detail

Pressing Enter on a session opens a scrollable overlay with the same filter, sort, and reverse controls available in the scores view:

```
┌────────── 2026-03-26 18:38 | jason/add_tests | 47dc1602 ──────────┐
│ 9580/9580 tests | 9580 passed | 0 failed                           │
│ Filter: All | Sort: Test (asc)                                      │
│ Test                           Outcome  Duration  Retries          ^│
│                                                                    ││
│ add_slots_test                 PASS     51ms      0                ││
│ address::tests::parse_case_1  PASS     50ms      0                █│
│ address::tests::parse_case_2  PASS     50ms      0                ││
│ address::tests::roundtrip     PASS     50ms      0                ││
│                                                                    ││
│ j/k:nav  s:sort  r:reverse  f:filter  Enter/q/Esc:back            v│
└────────────────────────────────────────────────────────────────────-┘
```

- **Title bar** -- session date, branch, and commit hash
- **Summary line** -- filtered/total tests, passed count, failed count
- **Filter bar** -- current outcome filter and sort field with direction (same orange style as the scores view)
- **Test table** -- test name, outcome (colour-coded PASS/FAIL/TIME/PANC/SKIP), duration, retry count
- **Scrollbar** -- right edge of the test table with `^`/`v` markers, tracks the selected row

#### Keybindings

| Key | Action |
|-----|--------|
| `j` / Down arrow | Move selection down |
| `k` / Up arrow | Move selection up |
| `s` | Cycle sort field (Test, Outcome, Duration, Retries) |
| `r` | Reverse sort order |
| `f` | Cycle outcome filter (All, Pass, Fail, Timeout, Panic, Ignored) |
| Enter / `q` / Esc | Return to session list |
| Ctrl+C | Quit |

#### Sorting

Press `s` to cycle through sort fields: Test name, Outcome, Duration, Retries. Press `r` to toggle ascending/descending. The current sort state is shown in the orange filter bar.

#### Filtering

Press `f` to cycle through outcome filters. When active, only runs with that outcome are shown. The summary line updates to show filtered/total counts.

## Disabling the TUI

For CI pipelines, scripts, or piped output, use `--non-interactive`:

```bash
cargo ninety-nine status --non-interactive
cargo ninety-nine history --non-interactive
cargo ninety-nine test -n 10 --non-interactive
```

This produces the same text output as previous versions.

## Visual Style

The TUI follows a panel-based layout inspired by tools like tOwl:

- **Header panel** -- bordered with cyan, contains the tool name and summary statistics
- **Filter bar** -- orange text showing current filter and sort state
- **Content panel** -- bordered with dark grey, contains the data table with a title
- **Scrollbar** -- right edge of content panel with `^`/`v` end markers
- **Footer** -- keybinding hints with bold key names
- **Category colours** -- Stable (green), Occasional (yellow), Moderate (red), Frequent (bold red), Critical (white on red)

## Terminal Requirements

The TUI uses the alternate screen buffer and raw mode via crossterm. It works in any terminal emulator that supports ANSI escape sequences. On Unix, SIGTERM and SIGHUP trigger graceful shutdown. On all platforms, the terminal state is restored on exit, including after panics.

Minimum terminal size: 60 columns by 10 rows.
