use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::drift::findings::{Finding, Severity};
use crate::drift::{line_of, GraphFacts};

/// A backtick-quoted, package-name-shaped token (lowercase, hyphenated,
/// optionally scoped e.g. `@org/pkg`) under a "Dependencies"/"package(s)"
/// heading that isn't among the packages this repo currently imports.
///
/// DECISION: scoped to sections under such a heading, not the whole doc.
/// Matching any lowercase backtick span anywhere (matching CLAUDE.md's
/// illustrative "Uses Redis" example literally) would flag ordinary
/// inline-code words — `config`, `bootstrap` — that were never dependency
/// claims at all. Restricting to where CLAUDE.md's own generated docs
/// actually list dependencies (component "Dependencies" sections,
/// dependencies.md's "External packages") keeps this deterministic and
/// low-false-positive.
pub fn check(file: &str, content: &str, facts: &GraphFacts) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut in_heading = false;
    let mut heading_text = String::new();
    let mut in_dependency_section = false;

    for (event, range) in Parser::new(content).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                let lower = heading_text.to_lowercase();
                in_dependency_section = lower.contains("depend") || lower.contains("package");
            }
            Event::Text(text) if in_heading => heading_text.push_str(&text),
            Event::Code(text)
                if in_dependency_section
                    && looks_like_package_name(&text)
                    && !facts.external_packages.contains(text.as_ref()) =>
            {
                findings.push(Finding {
                    file: file.to_string(),
                    line: line_of(content, range.start),
                    rule: "unknown-dependency".to_string(),
                    severity: Severity::Error,
                    message: format!("references `{text}`, no such dependency in graph"),
                });
            }
            _ => {}
        }
    }

    findings
}

fn looks_like_package_name(text: &str) -> bool {
    match text.split_once('/') {
        Some((scope, name)) if scope.starts_with('@') => {
            is_lower_kebab(&scope[1..]) && is_lower_kebab(name)
        }
        Some(_) => false, // "/" without a leading "@scope" — looks like a route, not a package
        None => is_lower_kebab(text),
    }
}

fn is_lower_kebab(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn facts_with_packages(names: &[&str]) -> GraphFacts {
        GraphFacts {
            symbol_names: HashSet::new(),
            file_paths: HashSet::new(),
            routes: HashSet::new(),
            external_packages: names.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn flags_unknown_package_under_dependencies_heading() {
        let content = "## Dependencies\n\nUses `redis` for sessions.\n";
        let findings = check(
            "docs/architecture.md",
            content,
            &facts_with_packages(&["express"]),
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "unknown-dependency");
    }

    #[test]
    fn ignores_package_shaped_code_outside_dependency_section() {
        let content = "## Usage\n\nRun `bootstrap` before `config`.\n";
        let findings = check("docs/architecture.md", content, &facts_with_packages(&[]));
        assert!(findings.is_empty());
    }

    #[test]
    fn does_not_flag_known_package() {
        let content = "## Dependencies\n\nUses `express` for routing.\n";
        let findings = check(
            "docs/architecture.md",
            content,
            &facts_with_packages(&["express"]),
        );
        assert!(findings.is_empty());
    }
}
