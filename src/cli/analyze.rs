use crate::cli::Cli;
use crate::util::exit;

pub fn run(_cli: &Cli) -> anyhow::Result<i32> {
    eprintln!("livingdocs analyze: not implemented yet (Phase 1)");
    Ok(exit::ERROR)
}
