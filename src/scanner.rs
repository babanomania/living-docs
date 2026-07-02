use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use git2::Repository;
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;

use crate::config::Config;

/// Every file under `root` that matches the config's `include` globs and
/// none of its `exclude` globs. Honors `.gitignore` along the way.
/// Paths are relative to `root` and returned in sorted (deterministic) order.
pub fn scan_all(root: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    let include = build_globset(&config.include)?;
    let exclude = build_globset(&config.exclude)?;

    let mut files = Vec::new();
    for entry in WalkBuilder::new(root).build() {
        let entry = entry.context("failed to walk repository")?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        if include.is_match(rel) && !exclude.is_match(rel) {
            files.push(rel.to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

/// Files that changed between `since_commit` and `HEAD`, filtered the same
/// way as [`scan_all`]. Used to make `analyze` incremental (§ Incremental
/// Analysis): a scheduled run only needs to reparse what moved.
// DECISION: not called yet — incremental analyze needs "since when", which
// only exists once the manifest tracks a last-synced commit (Phase 2/5).
// Kept now because scan_diff is explicitly part of Phase 1's scanner.rs.
#[allow(dead_code)]
pub fn scan_diff(root: &Path, config: &Config, since_commit: &str) -> Result<Vec<PathBuf>> {
    let repo = Repository::open(root).context("failed to open git repository")?;

    let since_tree = repo
        .revparse_single(since_commit)
        .with_context(|| format!("failed to resolve revision {since_commit}"))?
        .peel_to_commit()
        .with_context(|| format!("{since_commit} does not resolve to a commit"))?
        .tree()
        .context("failed to read tree for since_commit")?;

    let head_tree = repo
        .head()
        .context("repository has no HEAD")?
        .peel_to_commit()
        .context("HEAD does not resolve to a commit")?
        .tree()
        .context("failed to read tree for HEAD")?;

    let diff = repo
        .diff_tree_to_tree(Some(&since_tree), Some(&head_tree), None)
        .context("failed to diff trees")?;

    let include = build_globset(&config.include)?;
    let exclude = build_globset(&config.exclude)?;

    let mut files = Vec::new();
    diff.foreach(
        &mut |delta, _progress| {
            if let Some(path) = delta.new_file().path() {
                if include.is_match(path) && !exclude.is_match(path) {
                    files.push(path.to_path_buf());
                }
            }
            true
        },
        None,
        None,
        None,
    )
    .context("failed to walk diff deltas")?;

    files.sort();
    files.dedup();
    Ok(files)
}

fn build_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder
            .add(Glob::new(pattern).with_context(|| format!("invalid glob pattern: {pattern}"))?);
    }
    builder.build().context("failed to build globset")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn config_with(include: &[&str], exclude: &[&str]) -> Config {
        Config {
            include: include.iter().map(|s| s.to_string()).collect(),
            exclude: exclude.iter().map(|s| s.to_string()).collect(),
            ..Config::default()
        }
    }

    #[test]
    fn scan_all_respects_include_and_exclude() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(root.join("src/index.ts"), "export {}").unwrap();
        fs::write(root.join("src/index.test.ts"), "test").unwrap();
        fs::write(root.join("dist/bundle.ts"), "bundled").unwrap();
        fs::write(root.join("README.md"), "docs").unwrap();

        let config = config_with(&["src/**"], &["**/*.test.ts", "dist/**"]);
        let files = scan_all(root, &config).unwrap();

        assert_eq!(files, vec![PathBuf::from("src/index.ts")]);
    }

    #[test]
    fn scan_all_honors_gitignore() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        Repository::init(root).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join(".gitignore"), "src/ignored.ts\n").unwrap();
        fs::write(root.join("src/kept.ts"), "kept").unwrap();
        fs::write(root.join("src/ignored.ts"), "ignored").unwrap();

        let config = config_with(&["src/**"], &[]);
        let files = scan_all(root, &config).unwrap();

        assert_eq!(files, vec![PathBuf::from("src/kept.ts")]);
    }

    #[test]
    fn scan_diff_returns_only_changed_files_matching_config() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let repo = Repository::init(root).unwrap();
        let sig = git2::Signature::now("test", "test@example.com").unwrap();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/a.ts"), "a v1").unwrap();
        commit_all(&repo, &sig, "initial");
        let base = repo.head().unwrap().peel_to_commit().unwrap().id();

        fs::write(root.join("src/a.ts"), "a v2").unwrap();
        fs::write(root.join("src/b.ts"), "b v1").unwrap();
        fs::write(root.join("src/b.test.ts"), "test file").unwrap();
        commit_all(&repo, &sig, "second");

        let config = config_with(&["src/**"], &["**/*.test.ts"]);
        let files = scan_diff(root, &config, &base.to_string()).unwrap();

        assert_eq!(
            files,
            vec![PathBuf::from("src/a.ts"), PathBuf::from("src/b.ts")]
        );
    }

    fn commit_all(repo: &Repository, sig: &git2::Signature, message: &str) {
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let parents: Vec<_> = match repo.head().and_then(|h| h.peel_to_commit()) {
            Ok(commit) => vec![commit],
            Err(_) => vec![],
        };
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        repo.commit(Some("HEAD"), sig, sig, message, &tree, &parent_refs)
            .unwrap();
    }
}
