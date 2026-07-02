use crate::cli::Cli;
use crate::util::exit;

pub fn run(_cli: &Cli) -> anyhow::Result<i32> {
    eprintln!("livingdocs update: not implemented yet (Phase 5)");
    Ok(exit::ERROR)
}
