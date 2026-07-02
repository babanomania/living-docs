use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::config::Config;
use crate::parser::{self, Language, ParsedFile};
use crate::scanner;
use crate::util::hash;

use super::extractors::{express::ExpressExtractor, ExtractedRoute, RouteExtractor};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BuildStats {
    pub files: usize,
    pub symbols: usize,
    pub imports: usize,
    pub dependencies: usize,
    pub routes: usize,
}

/// Parse every file under `root` that `config` selects and load it into
/// `conn`. Always a full rebuild (`conn` is expected to already be a fresh
/// database — see `db::open_fresh`), which keeps the graph a pure function
/// of the current repo state.
pub fn build(conn: &Connection, root: &Path, config: &Config) -> Result<BuildStats> {
    let extractors: Vec<Box<dyn RouteExtractor>> = vec![Box::new(ExpressExtractor)];

    let mut parsed_files: Vec<(PathBuf, ParsedFile)> = Vec::new();
    let mut stats = BuildStats::default();

    for rel_path in scanner::scan_all(root, config)? {
        let Some(language) = Language::from_path(&rel_path) else {
            continue; // outside MVP language scope (TS/JS only)
        };

        let full_path = root.join(&rel_path);
        let source = fs::read_to_string(&full_path)
            .with_context(|| format!("failed to read {}", full_path.display()))?;

        insert_file(conn, &rel_path, language)?;
        stats.files += 1;

        let parsed = parser::parse_file(&rel_path, &source)?.unwrap_or_default();
        stats.symbols += insert_symbols(conn, &rel_path, &parsed)?;
        stats.imports += insert_imports(conn, &rel_path, &parsed)?;

        for extractor in &extractors {
            let routes = extractor.extract(&rel_path, &source, language)?;
            stats.routes += insert_routes(conn, &rel_path, extractor.name(), &routes)?;
        }

        parsed_files.push((rel_path, parsed));
    }

    stats.dependencies = resolve_dependencies(conn, &parsed_files)?;

    Ok(stats)
}

fn insert_file(conn: &Connection, path: &Path, language: Language) -> Result<()> {
    conn.execute(
        "INSERT INTO files (path, language) VALUES (?1, ?2)",
        rusqlite::params![path_str(path), language_str(language)],
    )
    .with_context(|| format!("failed to insert file {}", path.display()))?;
    Ok(())
}

fn language_str(language: Language) -> &'static str {
    match language {
        Language::TypeScript => "typescript",
        Language::Tsx => "tsx",
        Language::JavaScript => "javascript",
    }
}

fn insert_symbols(conn: &Connection, file: &Path, parsed: &ParsedFile) -> Result<usize> {
    let mut count = 0;

    for class in &parsed.classes {
        let methods_json = serde_json::to_string(&class.methods)?;
        conn.execute(
            "INSERT OR REPLACE INTO symbols (id, file_path, kind, name, exported, start_line, end_line, methods_json)
             VALUES (?1, ?2, 'class', ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                symbol_id(file, &class.name),
                path_str(file),
                class.name,
                class.exported,
                class.range.start_line as i64,
                class.range.end_line as i64,
                methods_json,
            ],
        )?;
        count += 1;
    }

    for function in &parsed.functions {
        conn.execute(
            "INSERT OR REPLACE INTO symbols (id, file_path, kind, name, exported, start_line, end_line, methods_json)
             VALUES (?1, ?2, 'function', ?3, ?4, ?5, ?6, NULL)",
            rusqlite::params![
                symbol_id(file, &function.name),
                path_str(file),
                function.name,
                function.exported,
                function.range.start_line as i64,
                function.range.end_line as i64,
            ],
        )?;
        count += 1;
    }

    for interface in &parsed.interfaces {
        conn.execute(
            "INSERT OR REPLACE INTO symbols (id, file_path, kind, name, exported, start_line, end_line, methods_json)
             VALUES (?1, ?2, 'interface', ?3, ?4, ?5, ?6, NULL)",
            rusqlite::params![
                symbol_id(file, &interface.name),
                path_str(file),
                interface.name,
                interface.exported,
                interface.range.start_line as i64,
                interface.range.end_line as i64,
            ],
        )?;
        count += 1;
    }

    Ok(count)
}

fn insert_imports(conn: &Connection, file: &Path, parsed: &ParsedFile) -> Result<usize> {
    for import in &parsed.imports {
        conn.execute(
            "INSERT INTO imports (file_path, source, specifiers_json, start_line) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                path_str(file),
                import.source,
                serde_json::to_string(&import.specifiers)?,
                import.range.start_line as i64,
            ],
        )?;
    }
    Ok(parsed.imports.len())
}

fn insert_routes(
    conn: &Connection,
    file: &Path,
    framework: &str,
    routes: &[ExtractedRoute],
) -> Result<usize> {
    for route in routes {
        conn.execute(
            "INSERT INTO api_routes (file_path, method, path, handler, start_line, framework) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                path_str(file),
                route.method,
                route.path,
                route.handler,
                route.range.start_line as i64,
                framework,
            ],
        )?;
    }
    Ok(routes.len())
}

/// Resolve each file's relative imports (`./x`, `../x`) to another scanned
/// file and record the edge. Bare package specifiers (e.g. `"express"`)
/// are not part of the local dependency graph and are left out — Phase 2's
/// `dependencies_of`/`consumers_of`/`cycles()` only need to reason about
/// files this repo actually owns.
fn resolve_dependencies(
    conn: &Connection,
    parsed_files: &[(PathBuf, ParsedFile)],
) -> Result<usize> {
    let known: HashSet<&Path> = parsed_files.iter().map(|(p, _)| p.as_path()).collect();
    let mut count = 0;

    for (file, parsed) in parsed_files {
        for import in &parsed.imports {
            let Some(target) = resolve_import(file, &import.source, &known) else {
                continue;
            };
            if target == *file {
                continue; // barrel files re-exporting from themselves, etc.
            }

            let specifiers = if import.specifiers.is_empty() {
                vec![String::new()] // side-effect-only import: `import "./setup"`
            } else {
                import.specifiers.clone()
            };

            for specifier in specifiers {
                conn.execute(
                    "INSERT OR IGNORE INTO dependencies (from_file, to_file, specifier) VALUES (?1, ?2, ?3)",
                    rusqlite::params![path_str(file), path_str(&target), specifier],
                )?;
                count += 1;
            }
        }
    }

    Ok(count)
}

fn resolve_import(from_file: &Path, specifier: &str, known: &HashSet<&Path>) -> Option<PathBuf> {
    if !(specifier.starts_with("./") || specifier.starts_with("../")) {
        return None; // bare/package specifier — outside the local graph
    }

    let base = from_file.parent().unwrap_or_else(|| Path::new(""));
    let joined = normalize(&base.join(specifier));

    const EXTENSIONS: [&str; 4] = ["ts", "tsx", "js", "jsx"];

    if known.contains(joined.as_path()) {
        return Some(joined);
    }
    for ext in EXTENSIONS {
        let candidate = joined.with_extension(ext);
        if known.contains(candidate.as_path()) {
            return Some(candidate);
        }
    }
    for ext in EXTENSIONS {
        let candidate = joined.join(format!("index.{ext}"));
        if known.contains(candidate.as_path()) {
            return Some(candidate);
        }
    }
    None
}

/// Lexically collapse `.`/`..` components without touching the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn symbol_id(file: &Path, name: &str) -> String {
    hash::stable_id(&format!("{}#{name}", path_str(file)))
}

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::db;
    use std::fs;
    use tempfile::tempdir;

    fn write_fixture(root: &Path) {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("livingdocs.config.json"),
            r#"{"include": ["src/**"], "exclude": []}"#,
        )
        .unwrap();
        fs::write(
            root.join("src/user-service.ts"),
            r#"
            import { PolicyService } from "./policy-service";
            export class UserService {
                constructor(private policies: PolicyService) {}
                create() {}
            }
            "#,
        )
        .unwrap();
        fs::write(
            root.join("src/policy-service.ts"),
            r#"
            export function calculatePremium(age: number): number { return age * 12; }
            export class PolicyService {
                quote() {}
            }
            "#,
        )
        .unwrap();
        fs::write(
            root.join("src/routes.js"),
            r#"
            const express = require("express");
            const app = express();
            app.get("/users", listUsers);
            "#,
        )
        .unwrap();
    }

    #[test]
    fn builds_files_symbols_imports_dependencies_and_routes() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        write_fixture(root);

        let config = Config::default();
        let conn = db::open_in_memory().unwrap();
        let stats = build(&conn, root, &config).unwrap();

        assert_eq!(stats.files, 3);
        assert_eq!(stats.symbols, 3); // UserService, PolicyService, calculatePremium
        assert_eq!(stats.routes, 1);
        assert_eq!(stats.dependencies, 1);

        let dep_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM dependencies WHERE from_file = 'src/user-service.ts' AND to_file = 'src/policy-service.ts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dep_count, 1);

        let route_method: String = conn
            .query_row("SELECT method FROM api_routes LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(route_method, "GET");
    }
}
