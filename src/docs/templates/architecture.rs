use anyhow::Result;
use rusqlite::Connection;

use crate::docs::templates::diagrams;
use crate::graph::queries;

pub const BLOCK_ID: &str = "architecture";

pub fn render(conn: &Connection) -> Result<String> {
    let classes = queries::list_classes(conn)?;
    let exported = classes.iter().filter(|c| c.exported).count();

    let summary = format!(
        "The system is composed of {} exported component{}.",
        exported,
        if exported == 1 { "" } else { "s" }
    );

    let diagram = diagrams::component_diagram(conn)?;

    Ok(format!(
        "## System Summary\n\n{summary}\n\n## Component Diagram\n\n```mermaid\n{diagram}\n```"
    ))
}
