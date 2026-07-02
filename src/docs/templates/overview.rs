use anyhow::Result;
use rusqlite::Connection;

use crate::docs::templates::bullet_list;
use crate::graph::queries;

pub const BLOCK_ID: &str = "overview";

pub fn render(conn: &Connection) -> Result<String> {
    let classes = queries::list_classes(conn)?;
    let files = queries::list_files(conn)?;
    let packages = queries::list_external_packages(conn)?;

    let purpose = format!(
        "This repository contains {} component{} across {} file{}.",
        classes.len(),
        plural(classes.len()),
        files.len(),
        plural(files.len()),
    );

    let tech_stack = bullet_list(
        &packages.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>(),
        "no external packages detected",
    );

    let modules: Vec<String> = classes
        .iter()
        .filter(|c| c.exported)
        .map(|c| {
            format!(
                "`{}` — {} method{} ({})",
                c.name,
                c.methods.len(),
                plural(c.methods.len()),
                c.file
            )
        })
        .collect();
    let major_modules = bullet_list(&modules, "no exported components detected");

    // Entry points: files nothing else in the repo depends on.
    let mut entry_points = Vec::new();
    for file in &files {
        if queries::consumers_of(conn, file)?.is_empty() {
            entry_points.push(format!("`{file}`"));
        }
    }
    let entry_points_list = bullet_list(&entry_points, "no clear entry points detected");

    Ok(format!(
        "## Purpose\n\n{purpose}\n\n## Tech Stack\n\n{tech_stack}\n\n\
         ## Major Modules\n\n{major_modules}\n\n## Entry Points\n\n{entry_points_list}"
    ))
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
