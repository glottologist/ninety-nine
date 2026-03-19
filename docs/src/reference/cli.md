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
| `-v, --verbose` | false | Enable verbose output |

---

## Commands

### test

Run tests and detect flakiness. Each discovered test is executed multiple times, scored with Bayesian inference, and results are stored for trend analysis.

```
cargo ninety-nine test [OPTIONS] [FILTER_EXPR]
```

| Argument/Option | Default | Description |
|-----------------|---------|-------------|
| `[FILTER_EXPR]` | none | Filter expression (DSL or test name pattern) |
| `-n, --iterations <N>` | from config (`min_runs`, default 10) | Number of times to run each test |
| `--confidence <FLOAT>` | from config (`confidence_threshold`, default 0.95) | Confidence threshold for flaky classification |

**Examples:**

```bash
# Run all tests 10 times each (defaults)
cargo ninety-nine test

# Run tests matching a substring
cargo ninety-nine test my_module

# Run with more iterations and stricter confidence
cargo ninety-nine test -n 25 --confidence 0.99

# Use filter DSL to run only flaky tests
cargo ninety-nine test "flaky"

# Combine filter predicates
cargo ninety-nine test "test(my_module) & !quarantined"
```

#### Filter DSL

The optional `FILTER_EXPR` argument accepts a domain-specific language for filtering tests. If the expression contains no DSL operators, it is treated as a plain test name substring filter.

**Predicates:**

| Predicate | Description |
|-----------|-------------|
| `test(pattern)` | Match test names by regex pattern |
| `package(name)` | Match tests in a package (substring match) |
| `binary(name)` | Match tests from a specific binary (substring match) |
| `kind(type)` | Match by binary kind: `lib`, `bin`, `test`, `example` |
| `flaky` | Match tests previously detected as flaky |
| `quarantined` | Match quarantined tests |
| `all` | Match all tests |
| `bare_word` | Treated as `test(bare_word)` regex pattern |

**Operators:**

| Operator | Meaning |
|----------|---------|
| `&` | AND -- both sides must match |
| `\|` | OR -- either side must match |
| `!` | NOT -- negate the following expression |
| `( )` | Grouping |

**Examples:**

```bash
# Tests matching a regex
cargo ninety-nine test "test(my_module::.*)"

# Flaky tests that are not quarantined
cargo ninety-nine test "flaky & !quarantined"

# Tests in a specific package or binary kind
cargo ninety-nine test "package(my_crate) | kind(test)"

# Complex expression with grouping
cargo ninety-nine test "(flaky | test(slow)) & !quarantined"
```

---

### init

Initialize a `.ninety-nine.toml` configuration file in the project root.

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
| `[FILTER]` | none | Filter by test name |
| `-n, --limit <N>` | 20 | Maximum sessions to show |

---

### export

Export flakiness data to a file.

```
cargo ninety-nine export <FORMAT> <PATH>
```

| Argument | Values | Description |
|----------|--------|-------------|
| `<FORMAT>` | `junit`, `html`, `csv`, `json` | Export format |
| `<PATH>` | file path | Output file path |

---

### quarantine

Manage test quarantine.

#### quarantine list

```
cargo ninety-nine quarantine list
```

Lists all quarantined tests with their quarantine date, reason, flakiness score, and whether they were auto-quarantined.

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

Generate a CI workflow file for flaky test detection.

```
cargo ninety-nine ci generate <PROVIDER> [PATH]
```

| Argument | Values | Description |
|----------|--------|-------------|
| `<PROVIDER>` | `github`, `gitlab` | CI provider |
| `[PATH]` | file path | Output file path (default: stdout) |
