# CLI Reference

## Synopsis

```
cargo ninety-nine [OPTIONS] <COMMAND>
```

## Global Options

| Option | Default | Description |
|--------|---------|-------------|
| `--project-dir <PATH>` | `.` | Project root directory |
| `--output <FORMAT>` | `console` | Output format: `console` or `json` |
| `-v, --verbose` | false | Verbose output |

---

## Commands

### detect

Detect flaky tests by running them multiple times.

```
cargo ninety-nine detect [OPTIONS] [FILTER]
```

| Argument/Option | Default | Description |
|-----------------|---------|-------------|
| `[FILTER]` | none | Test name substring filter |
| `-n, --iterations <N>` | 10 | Number of times to run each test |
| `--confidence <FLOAT>` | 0.95 | Confidence threshold for flaky classification |

**Example:**
```bash
cargo ninety-nine detect "my_module" -n 25 --confidence 0.99
```

---

### init

Initialize a configuration file.

```
cargo ninety-nine init [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--force` | Overwrite existing config file |

---

### status

Show flakiness status for tests.

```
cargo ninety-nine status [TEST_NAME]
```

| Argument | Description |
|----------|-------------|
| `[TEST_NAME]` | Show detailed status for a specific test. Omit to show all scores. |

When a test name is provided, shows: category, P(flaky), pass rate, total runs, consecutive failures, credible interval, trend direction, failure patterns, and recent run history.

---

### history

Show detection session history.

```
cargo ninety-nine history [OPTIONS] [FILTER]
```

| Argument/Option | Default | Description |
|-----------------|---------|-------------|
| `[FILTER]` | none | Filter by branch name or commit hash |
| `-n, --limit <N>` | 20 | Maximum sessions to show |

---

### export

Export flakiness data to a file.

```
cargo ninety-nine export <FORMAT> <PATH>
```

| Argument | Values | Description |
|----------|--------|-------------|
| `<FORMAT>` | `junit`, `html`, `csv` | Export format |
| `<PATH>` | file path | Output file path |

---

### quarantine

Manage test quarantine.

#### quarantine list

```
cargo ninety-nine quarantine list
```

#### quarantine add

```
cargo ninety-nine quarantine add <TEST_NAME> [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--reason <TEXT>` | `"manually quarantined"` | Reason for quarantine |

#### quarantine remove

```
cargo ninety-nine quarantine remove <TEST_NAME>
```

---

### ci

CI integration helpers.

#### ci generate

```
cargo ninety-nine ci generate <PROVIDER> [PATH]
```

| Argument | Values | Description |
|----------|--------|-------------|
| `<PROVIDER>` | `github`, `gitlab` | CI provider |
| `[PATH]` | file path | Output file path (default: stdout) |
