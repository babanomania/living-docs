use std::collections::BTreeSet;

use anyhow::Result;
use rusqlite::Connection;

use crate::graph::queries;

pub const BLOCK_ID: &str = "diagram-component";

/// `graph TD` component diagram: one edge per file-to-file dependency,
/// using each file's primary exported class as its node label. Files
/// with no exported class are omitted — there's nothing sensible to
/// call them, and CLAUDE.md's own examples only diagram named components.
pub fn component_diagram(conn: &Connection) -> Result<String> {
    let classes = queries::list_classes(conn)?;
    let mut edges = BTreeSet::new();

    for class in classes.iter().filter(|c| c.exported) {
        for dep_file in queries::dependencies_of(conn, &class.file)? {
            if let Some(dep_name) = queries::primary_component_of_file(conn, &dep_file)? {
                edges.insert((class.name.clone(), dep_name));
            }
        }
    }

    if edges.is_empty() {
        return Ok("graph TD\n%% no component dependencies detected".to_string());
    }

    let mut lines = vec!["graph TD".to_string()];
    for (from, to) in edges {
        lines.push(format!("{from} --> {to}"));
    }
    Ok(lines.join("\n"))
}

/// The standalone, embeddable version for `diagrams/component.md`.
pub fn render_standalone(conn: &Connection) -> Result<String> {
    let diagram = component_diagram(conn)?;
    Ok(format!("```mermaid\n{diagram}\n```"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::db;

    #[test]
    fn renders_placeholder_when_no_edges() {
        let conn = db::open_in_memory().unwrap();
        let diagram = component_diagram(&conn).unwrap();
        assert!(diagram.starts_with("graph TD"));
        assert!(diagram.contains("no component dependencies"));
    }
}
