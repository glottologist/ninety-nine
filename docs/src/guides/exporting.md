# Exporting Results

Export flakiness scores to various file formats for integration with CI dashboards, issue trackers, or custom tooling.

## Formats

### JUnit XML

```bash
cargo ninety-nine export junit results.xml
```

Produces standard JUnit XML. Tests with P(flaky) >= 5% are marked as failures with details in the failure message. Compatible with CI systems that parse JUnit reports (GitHub Actions, GitLab, Jenkins, etc.).

### HTML Report

```bash
cargo ninety-nine export html report.html
```

Generates a self-contained HTML page with a styled table showing all test scores. Categories are color-coded. Suitable for sharing with teams or archiving.

### CSV

```bash
cargo ninety-nine export csv results.csv
```

Produces a CSV file with columns:

```
test_name,probability_flaky,pass_rate,total_runs,consecutive_failures,category,confidence
```

Values containing commas, quotes, or newlines are properly escaped per RFC 4180.

### JSON

```bash
cargo ninety-nine export json results.json
```

Produces a JSON array of flakiness score objects with full detail, including Bayesian parameters. Useful for programmatic consumption, custom dashboards, or piping into other tools.

```json
[
  {
    "test_name": "tests::example",
    "probability_flaky": 0.167,
    "confidence": 0.95,
    "pass_rate": 0.8,
    "fail_rate": 0.2,
    "total_runs": 10,
    "consecutive_failures": 1,
    "bayesian_params": {
      "alpha": 1.0,
      "beta": 1.0,
      "posterior_mean": 0.167,
      "posterior_variance": 0.01,
      "credible_interval_lower": 0.02,
      "credible_interval_upper": 0.38
    }
  }
]
```

## Data Source

Export uses the most recent flakiness scores stored in the SQLite database. Run `test` first to populate data.
