# Filter DSL

The filter DSL lets you select which tests to run using an expressive query language. Filters are passed as the positional argument to the `test` command.

```bash
cargo ninety-nine test "<filter expression>"
```

## Syntax Overview

A filter expression is built from **predicates** combined with **boolean operators**.

### Bare Words

A bare word (any identifier that is not a keyword or function call) is treated as a regex pattern matched against the test name:

```bash
# Matches any test whose name contains "network"
cargo ninety-nine test "network"

# Regex: matches test names starting with "tests::db_"
cargo ninety-nine test "tests::db_"
```

### Function Predicates

Function predicates filter tests by metadata fields. The argument is always a single identifier.

| Function | Matches |
|----------|---------|
| `test(pattern)` | Test name matches the regex `pattern` |
| `package(name)` | Test belongs to a package whose name contains `name` |
| `binary(name)` | Test belongs to a binary whose name contains `name` |
| `kind(k)` | Binary kind is `k` -- one of `lib`, `bin`, `test`, `example` |

```bash
# Tests in the "my_crate" package
cargo ninety-nine test "package(my_crate)"

# Tests from test binaries only (excludes doctests, examples, etc.)
cargo ninety-nine test "kind(test)"

# Tests whose name matches a regex
cargo ninety-nine test "test(db_.*insert)"
```

### Boolean Keywords

These keywords evaluate based on stored flakiness data:

| Keyword | Matches |
|---------|---------|
| `flaky` | Tests with P(flaky) > 1% at or above the confidence threshold |
| `quarantined` | Tests currently in the quarantine list |
| `all` | All tests (always true) |

```bash
# Only previously-detected flaky tests
cargo ninety-nine test "flaky"

# All quarantined tests
cargo ninety-nine test "quarantined"
```

> **Note**: The `flaky` and `quarantined` keywords rely on data from previous runs stored in the database. Run `test` at least once before using them.

## Operators

Combine predicates using boolean operators:

| Operator | Meaning | Example |
|----------|---------|---------|
| `&` | AND -- both sides must match | `flaky & kind(test)` |
| `\|` | OR -- either side must match | `package(a) \| package(b)` |
| `!` | NOT -- inverts the predicate | `!quarantined` |
| `( )` | Grouping -- controls evaluation order | `(flaky \| quarantined) & kind(test)` |

### Operator Precedence

From highest to lowest:

1. `!` (NOT) -- binds tightest
2. `&` (AND)
3. `|` (OR) -- binds loosest

Use parentheses to override the default precedence when needed.

**Example**: `flaky | quarantined & kind(test)` is parsed as `flaky | (quarantined & kind(test))` because `&` binds tighter than `|`. To apply the OR first, write `(flaky | quarantined) & kind(test)`.

## Examples

### Basic Filtering

```bash
# Run all tests (no filter)
cargo ninety-nine test

# Match test names by regex
cargo ninety-nine test "integration"

# Tests in a specific package
cargo ninety-nine test "package(my_lib)"
```

### Combining Predicates

```bash
# Flaky tests that are not quarantined
cargo ninety-nine test "flaky & !quarantined"

# Tests from either of two packages
cargo ninety-nine test "package(auth) | package(session)"

# Network tests in the integration test binary
cargo ninety-nine test "test(network) & binary(integration)"
```

### CI-Focused Patterns

```bash
# Re-run only known flaky tests with extra iterations
cargo ninety-nine test "flaky & !quarantined" -n 50

# Run test binaries only, skip examples and benches
cargo ninety-nine test "kind(test)"

# Quarantined tests only (verify if they are still flaky)
cargo ninety-nine test "quarantined" -n 30 --confidence 0.99

# Everything except quarantined tests
cargo ninety-nine test "!quarantined"
```

### Complex Expressions

```bash
# Flaky tests in test binaries from a specific package
cargo ninety-nine test "flaky & kind(test) & package(core)"

# Either flaky or quarantined, but only from lib targets
cargo ninety-nine test "(flaky | quarantined) & kind(lib)"
```

## Error Handling

Invalid filter expressions produce a clear error message:

```bash
$ cargo ninety-nine test "kind(invalid)"
# Error: unknown binary kind: invalid

$ cargo ninety-nine test "unknown_func(arg)"
# Error: unknown function: unknown_func
```

Valid `kind` values are: `lib`, `bin`, `test`, `example`.
