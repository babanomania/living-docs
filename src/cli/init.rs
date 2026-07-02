use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::cli::Cli;
use crate::config::Config;
use crate::util::exit;

const GITIGNORE_ENTRIES: [&str; 2] = [".livingdocs/graph.db", ".livingdocs/drift.json"];

pub fn run(cli: &Cli) -> anyhow::Result<i32> {
    let config_path = cli.config_path();

    if config_path.exists() {
        println!("{} already exists, leaving it alone", config_path.display());
    } else {
        Config::write_default(&config_path)?;
        println!("wrote {}", config_path.display());
    }

    let config = Config::load(&config_path)?;

    fs::create_dir_all(&config.docs)
        .with_context(|| format!("failed to create docs directory at {}", config.docs))?;
    fs::create_dir_all(".livingdocs").context("failed to create .livingdocs directory")?;

    update_gitignore(Path::new(".gitignore"))?;

    println!("livingdocs is ready. Run `livingdocs analyze` next.");
    Ok(exit::OK)
}

/// Only `manifest.json` under `.livingdocs/` is meant to be committed
/// (see CLAUDE.md's Output layout) — the graph db and drift report are
/// regenerated on every run and would otherwise produce noisy diffs.
fn update_gitignore(path: &Path) -> anyhow::Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let missing: Vec<&str> = GITIGNORE_ENTRIES
        .iter()
        .copied()
        .filter(|entry| !existing.lines().any(|line| line.trim() == *entry))
        .collect();

    if missing.is_empty() {
        return Ok(());
    }

    let mut updated = existing;
    if !updated.is_empty() {
        if !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push('\n');
    }
    updated.push_str("# livingdocs\n");
    for entry in missing {
        updated.push_str(entry);
        updated.push('\n');
    }

    fs::write(path, updated).with_context(|| format!("failed to update {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_gitignore_when_absent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".gitignore");

        update_gitignore(&path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains(".livingdocs/graph.db"));
        assert!(content.contains(".livingdocs/drift.json"));
    }

    #[test]
    fn does_not_duplicate_existing_entries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".gitignore");
        fs::write(&path, "node_modules\n.livingdocs/graph.db\n").unwrap();

        update_gitignore(&path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.matches(".livingdocs/graph.db").count(), 1);
        assert!(content.contains(".livingdocs/drift.json"));
    }
}
