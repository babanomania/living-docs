use pulldown_cmark::{Event, Parser};

use crate::drift::findings::{Finding, Severity};
use crate::drift::{line_of, GraphFacts};

const HTTP_METHODS: [&str; 5] = ["GET", "POST", "PUT", "DELETE", "PATCH"];

/// A `METHOD /path` reference (as `apis/<resource>.md` documents routes,
/// e.g. "POST /users") whose route no longer exists in the graph.
pub fn check(file: &str, content: &str, facts: &GraphFacts) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (event, range) in Parser::new(content).into_offset_iter() {
        let text = match &event {
            Event::Text(t) | Event::Code(t) => t.as_ref(),
            _ => continue,
        };

        let words: Vec<&str> = text.split_whitespace().collect();
        for pair in words.windows(2) {
            let [method, path] = pair else { continue };
            let path = path.trim_end_matches(['.', ',', ';', ':', ')']);
            if HTTP_METHODS.contains(method) && path.starts_with('/') {
                let key = (method.to_string(), path.to_string());
                if !facts.routes.contains(&key) {
                    findings.push(Finding {
                        file: file.to_string(),
                        line: line_of(content, range.start),
                        rule: "removed-route".to_string(),
                        severity: Severity::Error,
                        message: format!("references `{method} {path}`, no such route in graph"),
                    });
                }
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn facts_with_routes(routes: &[(&str, &str)]) -> GraphFacts {
        GraphFacts {
            symbol_names: HashSet::new(),
            file_paths: HashSet::new(),
            routes: routes
                .iter()
                .map(|(m, p)| (m.to_string(), p.to_string()))
                .collect(),
            external_packages: HashSet::new(),
        }
    }

    #[test]
    fn flags_removed_route() {
        let content = "`POST /users` creates a user.\n";
        let findings = check(
            "docs/apis/users.md",
            content,
            &facts_with_routes(&[("GET", "/users")]),
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "removed-route");
    }

    #[test]
    fn does_not_flag_existing_route() {
        let content = "POST /users creates a user.\n";
        let findings = check(
            "docs/apis/users.md",
            content,
            &facts_with_routes(&[("POST", "/users")]),
        );
        assert!(findings.is_empty());
    }
}
