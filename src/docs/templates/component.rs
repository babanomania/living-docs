use anyhow::Result;
use rusqlite::Connection;

use crate::docs::templates::bullet_list;
use crate::graph::queries::{self, ClassSummary};
use crate::synthesis::prompts::GraphSlice;

pub fn responsibilities_block_id(slug: &str) -> String {
    format!("{slug}.responsibilities")
}

pub fn facts_block_id(slug: &str) -> String {
    format!("{slug}.facts")
}

/// The graph-facts payload sent to the model for this class's
/// Responsibilities prose — never raw source.
pub fn build_slice(conn: &Connection, class: &ClassSummary) -> Result<GraphSlice> {
    let dependencies = queries::dependencies_of(conn, &class.file)?;
    Ok(GraphSlice {
        entity: class.name.clone(),
        kind: "class".to_string(),
        methods: class.methods.clone(),
        dependencies,
    })
}

/// The deterministic (no LLM) part of a component doc: Dependencies,
/// Public API, Consumers — all pure functions of the graph.
pub fn render_facts(conn: &Connection, class: &ClassSummary) -> Result<String> {
    let dependencies = queries::dependencies_of(conn, &class.file)?;
    let consumers = queries::consumers_of(conn, &class.file)?;

    let dep_list = bullet_list(
        &dependencies
            .iter()
            .map(|d| format!("`{d}`"))
            .collect::<Vec<_>>(),
        "no dependencies detected",
    );
    let method_list = bullet_list(
        &class
            .methods
            .iter()
            .map(|m| format!("`{m}()`"))
            .collect::<Vec<_>>(),
        "no public methods detected",
    );
    let consumer_list = bullet_list(
        &consumers
            .iter()
            .map(|c| format!("`{c}`"))
            .collect::<Vec<_>>(),
        "no consumers detected",
    );

    Ok(format!(
        "## Dependencies\n\n{dep_list}\n\n## Public API\n\n{method_list}\n\n## Consumers\n\n{consumer_list}"
    ))
}
