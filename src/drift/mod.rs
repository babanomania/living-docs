use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::docs::manifest::Manifest;

pub mod findings;
pub mod rules;

use findings::Finding;

/// Read-only snapshot of the graph, loaded once per `check` run so rules
/// query in-memory sets instead of hitting SQLite per claim.
pub struct GraphFacts {
    pub symbol_names: HashSet<String>,
    pub file_paths: HashSet<String>,
    pub routes: HashSet<(String, String)>,
    /// Bare import specifiers, normalized to their package root
    /// (`"lodash/get"` -> `"lodash"`).
    pub external_packages: HashSet<String>,
}

impl GraphFacts {
    pub fn load(conn: &Connection) -> Result<GraphFacts> {
        Ok(GraphFacts {
            symbol_names: query_set(conn, "SELECT DISTINCT name FROM symbols")?,
            file_paths: query_set(conn, "SELECT path FROM files")?,
            routes: query_pairs(conn, "SELECT method, path FROM api_routes")?,
            external_packages: query_set(conn, "SELECT DISTINCT source FROM imports")?
                .into_iter()
                .filter(|s| !(s.starts_with("./") || s.starts_with("../")))
                .map(|s| package_root(&s))
                .collect(),
        })
    }
}

fn query_set(conn: &Connection, sql: &str) -> Result<HashSet<String>> {
    let mut stmt = conn
        .prepare(sql)
        .with_context(|| format!("failed to prepare query: {sql}"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<rusqlite::Result<HashSet<_>>>()
        .with_context(|| format!("failed to read rows for: {sql}"))
}

fn query_pairs(conn: &Connection, sql: &str) -> Result<HashSet<(String, String)>> {
    let mut stmt = conn
        .prepare(sql)
        .with_context(|| format!("failed to prepare query: {sql}"))?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<rusqlite::Result<HashSet<_>>>()
        .with_context(|| format!("failed to read rows for: {sql}"))
}

fn package_root(specifier: &str) -> String {
    let mut parts = specifier.split('/');
    match parts.next() {
        Some(scope) if scope.starts_with('@') => match parts.next() {
            Some(name) => format!("{scope}/{name}"),
            None => scope.to_string(),
        },
        Some(root) => root.to_string(),
        None => specifier.to_string(),
    }
}

/// 1-based line number of the byte at `offset` in `content`.
pub(crate) fn line_of(content: &str, offset: usize) -> usize {
    content[..offset.min(content.len())].matches('\n').count() + 1
}

/// Run every drift rule and return findings sorted for deterministic
/// output. Pure function of its inputs: reads `docs_dir` off disk and
/// queries `facts`/`manifest`, both already loaded — makes zero network
/// calls, so `check` never needs `OPENAI_API_KEY`.
pub fn check(
    root: &Path,
    docs_dir: &Path,
    manifest: &Manifest,
    facts: &GraphFacts,
) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();

    findings.extend(rules::managed_block_edited::check(root, manifest)?);

    for path in walk_markdown_files(docs_dir)? {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let file_label = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        findings.extend(rules::missing_entity::check(&file_label, &content, facts));
        findings.extend(rules::gone_symbol::check(&file_label, &content, facts));
        findings.extend(rules::removed_route::check(&file_label, &content, facts));
        findings.extend(rules::unknown_dependency::check(
            &file_label,
            &content,
            facts,
        ));
        findings.extend(rules::diagram_node_gone::check(
            &file_label,
            &content,
            facts,
        ));
    }

    findings.sort_by(|a, b| {
        (a.file.as_str(), a.line, a.rule.as_str()).cmp(&(b.file.as_str(), b.line, b.rule.as_str()))
    });
    Ok(findings)
}

fn walk_markdown_files(docs_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    if !docs_dir.exists() {
        return Ok(files);
    }
    for entry in ignore::WalkBuilder::new(docs_dir).build() {
        let entry = entry.context("failed to walk docs directory")?;
        if entry.file_type().is_some_and(|t| t.is_file())
            && entry.path().extension().and_then(|e| e.to_str()) == Some("md")
        {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}
