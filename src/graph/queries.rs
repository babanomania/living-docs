use anyhow::{Context, Result};
use petgraph::algo::kosaraju_scc;
use petgraph::graphmap::DiGraphMap;
use rusqlite::{Connection, OptionalExtension};

/// A class symbol plus the metadata `docs::templates` needs to render a
/// `components/<slug>.md` file, without each template issuing its own
/// follow-up queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassSummary {
    pub name: String,
    pub file: String,
    pub exported: bool,
    pub methods: Vec<String>,
}

/// Every class in the graph, ordered by name for deterministic output.
pub fn list_classes(conn: &Connection) -> Result<Vec<ClassSummary>> {
    let mut stmt = conn
        .prepare(
            "SELECT name, file_path, exported, methods_json FROM symbols \
             WHERE kind = 'class' ORDER BY name",
        )
        .context("failed to prepare list_classes query")?;
    let rows = stmt
        .query_map([], |row| {
            let methods_json: Option<String> = row.get(3)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                methods_json,
            ))
        })
        .context("failed to run list_classes query")?;

    let mut classes = Vec::new();
    for row in rows {
        let (name, file, exported, methods_json) = row.context("failed to read symbol row")?;
        let methods: Vec<String> = methods_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        classes.push(ClassSummary {
            name,
            file,
            exported,
            methods,
        });
    }
    Ok(classes)
}

/// The first exported class declared in `file`, if any — used as that
/// file's "component" identity in diagrams and docs (CLAUDE.md's "one
/// entity, one home" convention assumes one primary class per file).
pub fn primary_component_of_file(conn: &Connection, file: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT name FROM symbols WHERE kind = 'class' AND file_path = ?1 AND exported = 1 \
         ORDER BY start_line LIMIT 1",
        [file],
        |row| row.get(0),
    )
    .optional()
    .context("failed to query primary_component_of_file")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteSummary {
    pub method: String,
    pub path: String,
    pub file: String,
    pub handler: Option<String>,
}

/// Every API route in the graph, ordered by path then method for
/// deterministic output.
pub fn list_routes(conn: &Connection) -> Result<Vec<RouteSummary>> {
    let mut stmt = conn
        .prepare(
            "SELECT method, path, file_path, handler FROM api_routes \
             ORDER BY path, method",
        )
        .context("failed to prepare list_routes query")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(RouteSummary {
                method: row.get(0)?,
                path: row.get(1)?,
                file: row.get(2)?,
                handler: row.get(3)?,
            })
        })
        .context("failed to run list_routes query")?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read list_routes rows")
}

/// Every distinct external (non-relative) package this repo imports,
/// sorted for deterministic output.
pub fn list_external_packages(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT source FROM imports ORDER BY source")
        .context("failed to prepare list_external_packages query")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .context("failed to run list_external_packages query")?;

    let mut packages: Vec<String> = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read list_external_packages rows")?
        .into_iter()
        .filter(|s| !(s.starts_with("./") || s.starts_with("../")))
        .map(|s| package_root(&s))
        .collect();
    packages.sort();
    packages.dedup();
    Ok(packages)
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

/// Every file in the graph, sorted for deterministic output.
pub fn list_files(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT path FROM files ORDER BY path")
        .context("failed to prepare list_files query")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .context("failed to run list_files query")?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read list_files rows")
}

/// Files that `file` directly depends on (resolved local imports only),
/// sorted for deterministic output.
pub fn dependencies_of(conn: &Connection, file: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT to_file FROM dependencies WHERE from_file = ?1 ORDER BY to_file")
        .context("failed to prepare dependencies_of query")?;
    let rows = stmt
        .query_map([file], |row| row.get::<_, String>(0))
        .context("failed to run dependencies_of query")?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read dependencies_of rows")
}

/// Files that directly depend on `file` — the reverse edge, used for a
/// component doc's "Consumers" section.
pub fn consumers_of(conn: &Connection, file: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT from_file FROM dependencies WHERE to_file = ?1 ORDER BY from_file",
        )
        .context("failed to prepare consumers_of query")?;
    let rows = stmt
        .query_map([file], |row| row.get::<_, String>(0))
        .context("failed to run consumers_of query")?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read consumers_of rows")
}

/// Groups of files that form a dependency cycle (size > 1), each group and
/// the outer list sorted for deterministic output. Used by `dependencies.md`
/// and (later) `livingdocs review` (Phase 7) to flag circular dependencies.
pub fn cycles(conn: &Connection) -> Result<Vec<Vec<String>>> {
    let mut stmt = conn
        .prepare("SELECT from_file, to_file FROM dependencies")
        .context("failed to prepare cycles query")?;
    let edges = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .context("failed to run cycles query")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read cycles rows")?;

    let mut graph: DiGraphMap<&str, ()> = DiGraphMap::new();
    for (from, to) in &edges {
        graph.add_edge(from.as_str(), to.as_str(), ());
    }

    let mut groups: Vec<Vec<String>> = kosaraju_scc(&graph)
        .into_iter()
        .filter(|group| group.len() > 1)
        .map(|group| {
            let mut names: Vec<String> = group.into_iter().map(str::to_string).collect();
            names.sort();
            names
        })
        .collect();
    groups.sort();
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::db;

    fn seed(conn: &Connection, edges: &[(&str, &str)]) {
        for (from, to) in edges {
            conn.execute(
                "INSERT INTO files (path, language) VALUES (?1, 'typescript') ON CONFLICT DO NOTHING",
                [from],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO files (path, language) VALUES (?1, 'typescript') ON CONFLICT DO NOTHING",
                [to],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO dependencies (from_file, to_file, specifier) VALUES (?1, ?2, '')",
                [from, to],
            )
            .unwrap();
        }
    }

    #[test]
    fn dependencies_and_consumers_are_reverse_of_each_other() {
        let conn = db::open_in_memory().unwrap();
        seed(
            &conn,
            &[("a.ts", "b.ts"), ("a.ts", "c.ts"), ("b.ts", "c.ts")],
        );

        assert_eq!(
            dependencies_of(&conn, "a.ts").unwrap(),
            vec!["b.ts".to_string(), "c.ts".to_string()]
        );
        assert_eq!(
            consumers_of(&conn, "c.ts").unwrap(),
            vec!["a.ts".to_string(), "b.ts".to_string()]
        );
        assert!(dependencies_of(&conn, "c.ts").unwrap().is_empty());
    }

    #[test]
    fn cycles_detects_a_seeded_circular_dependency() {
        let conn = db::open_in_memory().unwrap();
        seed(
            &conn,
            &[("a.ts", "b.ts"), ("b.ts", "c.ts"), ("c.ts", "a.ts")],
        );

        let found = cycles(&conn).unwrap();
        assert_eq!(
            found,
            vec![vec![
                "a.ts".to_string(),
                "b.ts".to_string(),
                "c.ts".to_string()
            ]]
        );
    }

    #[test]
    fn no_cycles_in_a_dag() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn, &[("a.ts", "b.ts"), ("b.ts", "c.ts")]);

        assert!(cycles(&conn).unwrap().is_empty());
    }
}
