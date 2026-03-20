# Troubleshooting

Common issues and their solutions when using `cargo-ninety-nine`.

## Installation Issues

### `no test runner available`

```
error: no test runner available: install cargo-nextest or use cargo test
```

**Cause:** Neither `cargo-nextest` nor `cargo test` was found on the system PATH.

**Solutions:**
1. Install cargo-nextest: `cargo install cargo-nextest`
2. Verify Rust toolchain is installed: `rustup show`
3. Ensure `~/.cargo/bin` is in your PATH

### `binary discovery failed`

```
error: binary discovery failed: ...
```

**Cause:** `cargo test --no-run` failed to compile or list test binaries.

**Solutions:**
1. Run `cargo test --no-run` manually to see the full compiler output
2. Fix any compilation errors in your project
3. Ensure you are running from the project root (or use `--project-dir`)

## Configuration Issues

### `failed to parse config`

```
error: failed to parse config: ...
```

**Cause:** The `.ninety-nine.toml` file contains invalid TOML syntax or unrecognized fields.

**Solutions:**
1. Validate your TOML syntax: check for unclosed quotes, missing brackets, or incorrect indentation
2. Re-generate a fresh config: `cargo ninety-nine init --force`
3. Compare against the default config shown in the [Configuration Reference](../reference/configuration.md)

### `postgres backend selected but no config`

```
error: invalid configuration: postgres backend selected but no [storage.postgres] config provided
```

**Cause:** `storage.backend` is set to `"Postgres"` but the `[storage.postgres]` section is missing.

**Solution:** Add the PostgreSQL configuration:

```toml
[storage]
backend = "Postgres"

[storage.postgres]
connection_string = "host=localhost dbname=ninety_nine user=postgres"
pool_size = 4
```

## Test Execution Issues

### Tests not discovered

**Symptom:** `cargo ninety-nine test` reports 0 tests found.

**Causes and solutions:**
1. **No test targets:** Ensure your project has `#[test]` functions or files in `tests/`
2. **Filter too restrictive:** Remove or widen your filter expression
3. **Wrong project directory:** Use `--project-dir /path/to/project`
4. **Benchmark-only binaries:** Only `#[test]` functions are discovered, not benchmarks

### Test timeouts

**Symptom:** Tests that normally pass are reported as `Timeout`.

**Causes and solutions:**
1. **Default timeout too low:** The default is 300 seconds. For long-running tests, increase it in config:
   ```toml
   [detection]
   # Timeout is controlled via the execution config
   ```
2. **CI resource constraints:** CI environments often have fewer resources. Check `memory_gb` and `cpu_count` in the environment report
3. **Test contention:** Reduce `parallel_runs` to lower resource contention:
   ```toml
   [detection]
   parallel_runs = 1
   ```

### All tests show as flaky

**Symptom:** Every test gets a non-trivial flakiness score.

**Causes and solutions:**
1. **Too few iterations:** With only a few runs, the Bayesian prior has outsized influence. Increase `min_runs`:
   ```toml
   [detection]
   min_runs = 20
   ```
2. **Confidence threshold too low:** Raise the threshold to require stronger evidence:
   ```toml
   [detection]
   confidence_threshold = 0.99
   ```
3. **Systemic failures:** If tests are failing due to environment issues (missing database, network), fix the root cause rather than tuning thresholds

## Storage Issues

### SQLite database locked

**Symptom:** `storage error: database is locked`

**Causes and solutions:**
1. **Concurrent access:** Another `cargo-ninety-nine` process may be running. WAL mode should handle most concurrent access, but heavy parallel writes can still lock
2. **NFS/network filesystem:** SQLite does not work reliably over network filesystems. Use a local path or switch to PostgreSQL
3. **Stale lock:** If the process crashed, the lock file may remain. Delete `ninety-nine.db-wal` and `ninety-nine.db-shm` next to the database

### PostgreSQL connection failures

**Symptom:** `postgres storage error: connection refused` or similar

**Solutions:**
1. Verify PostgreSQL is running: `pg_isready`
2. Check connection string format: `host=localhost port=5432 dbname=ninety_nine user=postgres password=...`
3. Ensure the database exists: `createdb ninety_nine`
4. Check network/firewall settings for remote connections
5. Verify pool size is reasonable for your connection limits

### Data retention and database size

**Symptom:** Database growing too large.

**Solution:** Configure `retention_days` to automatically purge old data:

```toml
[storage]
retention_days = 30  # default: 90
```

Data is purged at the end of each `test` run.

## Filter DSL Issues

### `filter parse error`

**Symptom:** `error: filter parse error: ...`

**Common mistakes:**
1. **Missing parentheses on predicates:** Use `flaky()` not `flaky`
2. **Wrong operator syntax:** Use `&` for AND, `|` for OR, `!` for NOT
3. **Unclosed parentheses:** Ensure every `(` has a matching `)`
4. **Invalid regex in test():** The pattern must be a valid Rust regex

**Valid examples:**
```
flaky()
test(.*timeout.*)
package(auth) & !quarantined()
(flaky() | test(.*race.*)) & package(core)
```

## CI Integration Issues

### Workflow not triggering

**Symptom:** Generated CI workflow never runs.

**Solutions:**
- **GitHub Actions:** Ensure the workflow file is at `.github/workflows/`. Check that scheduled triggers are on the default branch
- **GitLab CI:** Ensure the pipeline is configured to run on schedules. Check that `rules` allow scheduled execution

### CI environment not detected

**Symptom:** `is_ci` shows `false` in CI.

**Cause:** The CI provider's environment variable is not set or not recognized.

**Recognized variables:**
| Variable | Provider |
|----------|----------|
| `GITHUB_ACTIONS` | GitHub Actions |
| `GITLAB_CI` | GitLab CI |
| `JENKINS_URL` | Jenkins |
| `CIRCLECI` | CircleCI |
| `TF_BUILD` | Azure DevOps |
| `BUILDKITE` | Buildkite |

## Export Issues

### Empty export files

**Symptom:** Export produces a file with no test data.

**Cause:** No flakiness scores have been computed yet.

**Solution:** Run `cargo ninety-nine test` at least once before exporting. Scores are computed and stored during the test run.

### JUnit XML not recognized

**Symptom:** CI system does not parse the JUnit XML output.

**Solution:** Ensure the export path matches what your CI expects. For GitHub Actions:

```yaml
- uses: dorny/test-reporter@v1
  with:
    artifact: ninety-nine-report
    name: Flaky Tests
    path: report.xml
    reporter: java-junit
```

## Getting More Information

Enable verbose output for detailed tracing:

```bash
cargo ninety-nine --verbose test
```

This sets the tracing subscriber to `debug` level, showing:
- Binary discovery details
- Test listing parsing
- Individual test execution results
- Storage operations
- Bayesian computation details
