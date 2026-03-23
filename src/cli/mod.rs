pub mod export;
pub mod output;

pub use output::OutputFormat;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "cargo-ninety-nine",
    about = "Detect and track flaky tests in Rust projects",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CargoSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum CargoSubcommand {
    #[command(name = "ninety-nine", about = "Flaky test detection and tracking")]
    NinetyNine(NinetyNineArgs),
}

#[derive(Debug, Parser)]
pub struct NinetyNineArgs {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long, default_value = ".", global = true)]
    pub project_dir: PathBuf,

    #[arg(long, value_enum, default_value_t = OutputFormat::Console, global = true)]
    pub output: OutputFormat,

    #[arg(long, short, global = true)]
    pub verbose: bool,

    /// Disable interactive TUI mode (for CI/scripting)
    #[arg(long, short = 'N', global = true)]
    pub non_interactive: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CiTarget {
    Github,
    Gitlab,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportFormat {
    Junit,
    Html,
    Csv,
    Json,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Run tests and detect flakiness")]
    Test {
        #[arg(help = "Filter expression (DSL or test name pattern)")]
        filter_expr: Option<String>,

        #[arg(long, short = 'n', help = "Number of iterations per test")]
        iterations: Option<u32>,

        #[arg(long, help = "Confidence threshold for flakiness detection")]
        confidence: Option<f64>,
    },

    #[command(about = "Initialize configuration file")]
    Init {
        #[arg(long)]
        force: bool,
    },

    #[command(about = "Show detection history")]
    History {
        #[arg(help = "Test name filter")]
        filter: Option<String>,

        #[arg(long, short = 'n', default_value_t = 20)]
        limit: u32,
    },

    #[command(about = "Show flakiness status for tests")]
    Status {
        #[arg(help = "Specific test name")]
        test_name: Option<String>,
    },

    #[command(about = "Export flakiness data to a file")]
    Export {
        #[arg(value_enum, help = "Export format")]
        format: ExportFormat,

        #[arg(help = "Output file path")]
        path: PathBuf,
    },

    #[command(subcommand, about = "Manage test quarantine")]
    Quarantine(QuarantineCommand),

    #[command(subcommand, about = "CI integration helpers")]
    Ci(CiCommand),
}

#[derive(Debug, Subcommand)]
pub enum CiCommand {
    #[command(about = "Generate CI workflow file")]
    Generate {
        #[arg(value_enum, help = "CI provider")]
        provider: CiTarget,

        #[arg(help = "Output file path (default: stdout)")]
        path: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum QuarantineCommand {
    #[command(about = "List quarantined tests")]
    List,

    #[command(about = "Quarantine a flaky test")]
    Add {
        #[arg(help = "Test name to quarantine")]
        test_name: String,

        #[arg(long, default_value = "manually quarantined")]
        reason: String,
    },

    #[command(about = "Remove a test from quarantine")]
    Remove {
        #[arg(help = "Test name to unquarantine")]
        test_name: String,
    },
}
