# Getting Started

## Prerequisites

- **Rust 1.85+** (Edition 2024)
- **cargo** (included with Rust)
- Optionally, **cargo-nextest** for enhanced test running

## Installation

Install from crates.io:

```bash
cargo install cargo-ninety-nine
```

Or build from source:

```bash
git clone https://github.com/glottologist/ninety-nine.git
cd ninety-nine
cargo install --path .
```

## Verify Installation

```bash
cargo ninety-nine --version
```

## Initialize a Project

Navigate to your Rust project and create a configuration file:

```bash
cd /path/to/your/project
cargo ninety-nine init
```

This creates a `.ninety-nine.toml` file with sensible defaults. You can overwrite an existing config with:

```bash
cargo ninety-nine init --force
```

## First Detection Run

Run detection with default settings (10 iterations per test):

```bash
cargo ninety-nine detect
```

Filter to specific tests:

```bash
cargo ninety-nine detect "my_module::tests"
```

Increase iterations for higher confidence:

```bash
cargo ninety-nine detect -n 50 --confidence 0.99
```

## Output Formats

Add `--output json` to any command for machine-readable output:

```bash
cargo ninety-nine status --output json
cargo ninety-nine history --output json
```
