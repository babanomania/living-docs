use crate::cli::Cli;
use crate::util::exit;

pub fn run(_cli: &Cli) -> anyhow::Result<i32> {
    eprintln!("livingdocs check: not implemented yet (Phase 3)");
    Ok(exit::ERROR)
}
