# Usage Overview

`cargo ninety-nine` provides seven subcommands:

| Command | Purpose |
|---------|---------|
| `detect` | Run tests repeatedly and compute flakiness scores |
| `init` | Create a default configuration file |
| `status` | View current flakiness scores and test detail |
| `history` | View past detection sessions |
| `export` | Export results to JUnit XML, HTML, or CSV |
| `quarantine` | Manage test quarantine (list, add, remove) |
| `ci` | Generate CI workflow files |

## Global Options

These options apply to all subcommands:

```
--project-dir <PATH>   Project root directory (default: .)
--output <FORMAT>      Output format: console or json (default: console)
--verbose, -v          Verbose output during detection
```

## Typical Workflow

1. **Initialize** — `cargo ninety-nine init`
2. **Detect** — `cargo ninety-nine detect -n 20`
3. **Investigate** — `cargo ninety-nine status tests::suspect_test`
4. **Quarantine** — `cargo ninety-nine quarantine add tests::flaky_test --reason "timing-dependent"`
5. **Export** — `cargo ninety-nine export html report.html`
6. **Automate** — `cargo ninety-nine ci generate github`
