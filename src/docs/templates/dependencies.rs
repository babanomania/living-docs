use anyhow::Result;
use rusqlite::Connection;

use crate::docs::templates::{bullet_list, diagrams};
use crate::graph::queries;

pub const BLOCK_ID: &str = "dependencies";

pub fn render(conn: &Connection) -> Result<String> {
    let packages = queries::list_external_packages(conn)?;
    let external = bullet_list(
        &packages.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>(),
        "no external packages detected",
    );

    let diagram = diagrams::component_diagram(conn)?;

    let cycles = queries::cycles(conn)?;
    let flagged = if cycles.is_empty() {
        "_no circular dependencies detected_".to_string()
    } else {
        cycles
            .iter()
            .map(|group| format!("- {}", group.join(" -> ")))
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(format!(
        "## External Packages\n\n{external}\n\n## Internal Module Dependency Graph\n\n\
         ```mermaid\n{diagram}\n```\n\n## Flagged: Circular Dependencies\n\n{flagged}"
    ))
}
