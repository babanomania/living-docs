pub const BLOCK_ID: &str = "index";

/// A managed table of contents grouped by kind, in a fixed section order
/// so re-runs never reorder the list. `components` and `api_resources`
/// should already be sorted by the caller.
pub fn render(components: &[String], api_resources: &[String], has_routes: bool) -> String {
    let mut sections = vec![
        "## Overview\n\n- [Overview](overview.md)\n- [Architecture](architecture.md)\n\
         - [Dependencies](dependencies.md)"
            .to_string(),
    ];

    if !components.is_empty() {
        let links = components
            .iter()
            .map(|slug| format!("- [{slug}](components/{slug}.md)"))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("## Components\n\n{links}"));
    }

    if has_routes {
        let mut links = vec!["- [All Routes](apis/index.md)".to_string()];
        for resource in api_resources {
            links.push(format!("- [{resource}](apis/{resource}.md)"));
        }
        sections.push(format!("## APIs\n\n{}", links.join("\n")));
    }

    sections.push("## Diagrams\n\n- [Component Diagram](diagrams/component.md)".to_string());

    sections.join("\n\n")
}
