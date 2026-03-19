# Usage Overview

`cargo ninety-nine` provides seven subcommands:

| Command | Purpose |
|---------|---------|
| `test` | Run tests repeatedly and compute flakiness scores |
| `init` | Create a default configuration file |
| `status` | View current flakiness scores and test detail |
| `history` | View past detection sessions |
| `export` | Export results to JUnit XML, HTML, CSV, or JSON |
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

1. **Initialize** -- `cargo ninety-nine init`
2. **Test** -- `cargo ninety-nine test -n 20`
3. **Filter** -- `cargo ninety-nine test "flaky & !quarantined"` (see [Filter DSL](./filter-dsl.md))
4. **Investigate** -- `cargo ninety-nine status tests::suspect_test`
5. **Quarantine** -- `cargo ninety-nine quarantine add tests::flaky_test --reason "timing-dependent"`
6. **Export** -- `cargo ninety-nine export html report.html`
7. **Automate** -- `cargo ninety-nine ci generate github`
