use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde::Serialize;

use crate::cli::Cli;
use crate::config::Config;
use crate::parser::{self, ParsedFile};
use crate::scanner;
use crate::util::exit;

#[derive(Serialize)]
struct FileSymbols {
    file: PathBuf,
    #[serde(flatten)]
    parsed: ParsedFile,
}

pub fn run(cli: &Cli) -> anyhow::Result<i32> {
    let config_path = cli.config_path();
    let config = Config::load(&config_path).with_context(|| {
        format!(
            "no config found at {}; run `livingdocs init` first",
            config_path.display()
        )
    })?;

    let root = std::env::current_dir().context("failed to read current directory")?;
    let files = scanner::scan_all(&root, &config)?;

    let mut symbols = Vec::new();
    for rel_path in files {
        let full_path = root.join(&rel_path);
        let source = fs::read_to_string(&full_path)
            .with_context(|| format!("failed to read {}", full_path.display()))?;
        if let Some(parsed) = parser::parse_file(&rel_path, &source)? {
            symbols.push(FileSymbols {
                file: rel_path,
                parsed,
            });
        }
    }

    if cli.dry_run {
        // DECISION: --dry-run always emits JSON regardless of --format —
        // it's a raw symbol preview with no prose template yet (that's
        // Phase 5's job), so a "text" rendering has nothing sensible to say.
        println!("{}", serde_json::to_string_pretty(&symbols)?);
        return Ok(exit::OK);
    }

    eprintln!("livingdocs analyze: graph building not implemented yet (Phase 2)");
    Ok(exit::ERROR)
}
