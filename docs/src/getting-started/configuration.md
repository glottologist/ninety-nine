# Configuration

Configuration is stored in `.ninety-nine.toml` at the project root. All fields are optional — missing fields use defaults.

## Generating a Config File

```bash
cargo ninety-nine init
```

## Minimal Configuration

An empty `.ninety-nine.toml` file uses all defaults. You only need to specify values you want to change:

```toml
[detection]
min_runs = 20
confidence_threshold = 0.99

[ci]
fail_on_flaky = true
```

## Full Configuration Reference

See [Configuration Reference](../reference/configuration.md) for all available options and their defaults.

## Config Loading

The config file is loaded from the `--project-dir` path (defaults to `.`). If no config file exists, all default values are used. Unknown fields in the TOML file are silently ignored, so the config file is forward-compatible with newer versions.
