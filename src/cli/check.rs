use anyhow::Context;

use crate::cli::{Cli, OutputFormat};
use crate::config::Config;
use crate::docs::manifest::Manifest;
use crate::drift::{self, findings, GraphFacts};
use crate::util::exit;

/// Pure graph math: reads `.livingdocs/graph.db`, the manifest, and the
/// docs tree off disk. Never constructs a synthesis client, so it makes
/// zero network calls and never needs `OPENAI_API_KEY` (§0 invariant).
pub fn run(cli: &Cli) -> anyhow::Result<i32> {
    let config_path = cli.config_path();
    let config = Config::load(&config_path).with_context(|| {
        format!(
            "no config found at {}; run `livingdocs init` first",
            config_path.display()
        )
    })?;

    let root = std::env::current_dir().context("failed to read current directory")?;

    let graph_path = root.join(".livingdocs").join("graph.db");
    if !graph_path.exists() {
        anyhow::bail!(
            "no graph found at {}; run `livingdocs analyze` first",
            graph_path.display()
        );
    }
    let conn = rusqlite::Connection::open(&graph_path)
        .with_context(|| format!("failed to open graph at {}", graph_path.display()))?;
    let facts = GraphFacts::load(&conn)?;

    let manifest = Manifest::load(&root.join(".livingdocs").join("manifest.json"))?;
    let docs_dir = root.join(&config.docs);

    let results = drift::check(&root, &docs_dir, &manifest, &facts)?;

    match cli.format {
        OutputFormat::Json => println!("{}", findings::format_json(&results)?),
        OutputFormat::Text => {
            if results.is_empty() {
                println!("no drift found");
            } else {
                println!("{}", findings::format_text(&results));
            }
        }
    }

    Ok(if results.is_empty() {
        exit::OK
    } else {
        exit::DRIFT_FOUND
    })
}
