use std::collections::BTreeMap;

use crate::graph::queries::RouteSummary;

/// Group routes by their first path segment, e.g. `/users/:id` -> `users`,
/// matching CLAUDE.md's `apis/<resource>.md` layout. A `BTreeMap` keeps
/// resource (and therefore file) ordering deterministic.
pub fn group_by_resource(routes: &[RouteSummary]) -> BTreeMap<String, Vec<RouteSummary>> {
    let mut grouped: BTreeMap<String, Vec<RouteSummary>> = BTreeMap::new();
    for route in routes {
        let resource = route
            .path
            .trim_start_matches('/')
            .split('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("root")
            .to_string();
        grouped.entry(resource).or_default().push(route.clone());
    }
    grouped
}

pub const INDEX_BLOCK_ID: &str = "apis-index";

pub fn render_index(routes: &[RouteSummary]) -> String {
    if routes.is_empty() {
        return "_no API routes detected_".to_string();
    }
    routes
        .iter()
        .map(|r| format!("- `{} {}`", r.method, r.path))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn block_id_for_resource(resource: &str) -> String {
    format!("apis-{resource}")
}

pub fn render_resource(routes: &[RouteSummary]) -> String {
    routes
        .iter()
        .map(|r| {
            let handler = r.handler.as_deref().unwrap_or("(anonymous handler)");
            format!(
                "### `{} {}`\n\nHandler: `{}`  \nFile: `{}`",
                r.method, r.path, handler, r.file
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(method: &str, path: &str) -> RouteSummary {
        RouteSummary {
            method: method.to_string(),
            path: path.to_string(),
            file: "src/routes.js".to_string(),
            handler: Some("handler".to_string()),
        }
    }

    #[test]
    fn groups_routes_by_first_path_segment() {
        let routes = vec![
            route("GET", "/users"),
            route("POST", "/users"),
            route("GET", "/policies/:id"),
        ];
        let grouped = group_by_resource(&routes);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["users"].len(), 2);
        assert_eq!(grouped["policies"].len(), 1);
    }
}
