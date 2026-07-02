use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

pub mod analyze;
pub mod check;
pub mod explain;
pub mod init;
pub mod review;
pub mod sync;
pub mod update;
pub mod watch;

#[derive(Parser, Debug)]
#[command(
    name = "livingdocs",
    version,
    about = "Documentation that evolves with your code."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Increase log verbosity.
    #[arg(long, global = true)]
    pub verbose: bool,

    /// Output format for machine-readable commands.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    /// Print what would happen without writing anything.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Open a pull request with the result.
    #[arg(long, global = true)]
    pub pr: bool,

    /// Path to livingdocs.config.json.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Token budget for this run.
    #[arg(long, global = true, value_name = "TOKENS")]
    pub budget: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Scaffold config + managed doc sections in this repo.
    Init,
    /// Build/refresh the knowledge graph; generate initial docs.
    Analyze,
    /// Detect drift only. Exit 0 = clean, 1 = drift, 2 = error.
    Check,
    /// Detect drift -> synthesize drifted -> verify -> write/PR.
    Update,
    /// Regenerate all managed sections from the graph.
    Sync,
    /// Local dev: re-check on file changes (debounced).
    Watch,
    /// Ad hoc grounded explanation of a symbol.
    Explain {
        /// Symbol name or free-text description to explain.
        name: String,
    },
    /// Architecture review: cycles, large modules, tight coupling.
    Review,
}

impl Cli {
    /// Resolve the config path, defaulting to `./livingdocs.config.json`.
    pub fn config_path(&self) -> PathBuf {
        self.config
            .clone()
            .unwrap_or_else(|| PathBuf::from("livingdocs.config.json"))
    }
}
