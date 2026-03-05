# CI Integration

## Generating CI Workflows

Generate ready-to-use CI configuration files:

### GitHub Actions

```bash
# Print to stdout
cargo ninety-nine ci generate github

# Write to file
cargo ninety-nine ci generate github .github/workflows/flaky-tests.yml
```

The generated workflow:
- Runs on a weekly schedule (Monday 3:00 AM UTC) and on manual dispatch
- Installs Rust, cargo-nextest, and cargo-ninety-nine
- Runs flaky test detection with your configured `min_runs` and `confidence_threshold`
- Exports JUnit XML results
- Uploads results as a build artifact

### GitLab CI

```bash
cargo ninety-nine ci generate gitlab .gitlab-ci-flaky.yml
```

The generated job:
- Runs on scheduled and manual pipelines
- Uses the `rust:latest` Docker image
- Installs cargo-nextest and cargo-ninety-nine
- Runs detection and exports JUnit XML
- Publishes JUnit artifacts for GitLab's test report

## fail_on_flaky

The `ci.fail_on_flaky` config option controls whether the CI step fails when flaky tests are detected:

```toml
[ci]
fail_on_flaky = true
```

| Value | GitHub Actions | GitLab CI |
|-------|---------------|-----------|
| `false` (default) | Adds `continue-on-error: true` | Adds `allow_failure: true` |
| `true` | Step fails normally | Step fails normally |

When `fail_on_flaky = true`, the `detect` command exits with a non-zero status code if any test is classified as flaky by the Bayesian detector.

## Manual CI Setup

If you prefer to configure CI manually:

```bash
# Install
cargo install cargo-ninety-nine

# Run detection
cargo ninety-nine detect -n 20 --confidence 0.95

# Export for CI test report parsing
cargo ninety-nine export junit flaky-results.xml

# Exit non-zero if flaky (optional, set in config)
# Or check the exit code of detect when fail_on_flaky = true
```

## Environment Detection

`cargo ninety-nine` automatically detects the CI environment:

| Environment Variable | Detected Provider |
|---------------------|-------------------|
| `GITHUB_ACTIONS` | GitHub Actions |
| `GITLAB_CI` | GitLab CI |
| `JENKINS_URL` | Jenkins |
| `CIRCLECI` | CircleCI |
| `BUILDKITE` | Buildkite |
| `TF_BUILD` | Azure DevOps |

The detected provider is stored with each test run, enabling environmental pattern analysis (CI vs local failure rate differences).
