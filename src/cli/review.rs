use crate::cli::Cli;
use crate::util::exit;

pub fn run(_cli: &Cli) -> anyhow::Result<i32> {
    eprintln!("livingdocs review: not implemented yet (Phase 7)");
    Ok(exit::ERROR)
}
