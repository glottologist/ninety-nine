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

## Data Source

Export uses the most recent flakiness scores stored in the SQLite database. Run `detect` first to populate data.
