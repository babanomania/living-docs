mod cli;
mod config;
mod docs;
mod drift;
mod graph;
mod parser;
mod scanner;
// DECISION: the synthesis adapter has no caller until Phase 5's
// update/sync and Phase 7's explain wire it up; #[allow(dead_code)]
// on the whole module rather than annotating every item individually.
#[allow(dead_code)]
mod synthesis;
mod util;

use clap::Parser;

use cli::{Cli, Command};
use util::exit;

fn main() {
    let cli = Cli::parse();
    util::log::init(cli.verbose);

    let result = match &cli.command {
        Command::Init => cli::init::run(&cli),
        Command::Analyze => cli::analyze::run(&cli),
        Command::Check => cli::check::run(&cli),
        Command::Update => cli::update::run(&cli),
        Command::Sync => cli::sync::run(&cli),
        Command::Watch => cli::watch::run(&cli),
        Command::Explain { name } => cli::explain::run(&cli, name),
        Command::Review => cli::review::run(&cli),
    };

    let code = match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("livingdocs: error: {err:#}");
            exit::ERROR
        }
    };

    std::process::exit(code);
}
