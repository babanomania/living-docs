use anyhow::{Context, Result};
use petgraph::algo::kosaraju_scc;
use petgraph::graphmap::DiGraphMap;
use rusqlite::Connection;

// DECISION: dependencies_of/consumers_of/cycles have no CLI caller yet —
// they land in `livingdocs review` (Phase 7) and the component docs'
// "Consumers" section (Phase 5). Kept now (with tests) because Phase 2's
// Done-when explicitly requires them; #[allow(dead_code)] instead of
// deleting until their caller exists.

/// Files that `file` directly depends on (resolved local imports only),
/// sorted for deterministic output.
#[allow(dead_code)]
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
#[allow(dead_code)]
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
/// the outer list sorted for deterministic output. Used by `livingdocs
/// review` (Phase 7) to flag circular dependencies.
#[allow(dead_code)]
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
