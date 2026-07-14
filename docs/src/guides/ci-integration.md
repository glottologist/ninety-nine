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

## Failure Behaviour

Generated workflows never fail the pipeline on flaky tests: the GitHub Actions job carries `continue-on-error: true` and the GitLab job carries `allow_failure: true`, so detection results arrive as reports rather than as red builds. Remove those lines from the generated file if you would rather have flaky detections break the build.

## Manual CI Setup

If you prefer to configure CI manually:

```bash
# Install
cargo install cargo-ninety-nine

# Run detection
cargo ninety-nine test -n 20 --confidence 0.95

# Export for CI test report parsing
cargo ninety-nine export junit flaky-results.xml

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
