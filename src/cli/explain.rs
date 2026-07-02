use crate::cli::Cli;
use crate::util::exit;

pub fn run(_cli: &Cli, _name: &str) -> anyhow::Result<i32> {
    eprintln!("livingdocs explain: not implemented yet (Phase 7)");
    Ok(exit::ERROR)
}
