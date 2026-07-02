use crate::docs::frontmatter;
use crate::drift::findings::{Finding, Severity};
use crate::drift::GraphFacts;

/// A generated file's front-matter `entity` no longer exists in the
/// graph — the file has nothing left to document. Per CLAUDE.md: "If
/// `entity` no longer exists in the graph, the file is flagged stale."
/// Files with no front matter (plain user-authored docs) aren't checked.
pub fn check(file: &str, content: &str, facts: &GraphFacts) -> Vec<Finding> {
    let Some((front_matter, _body)) = frontmatter::parse(content) else {
        return Vec::new();
    };
    let Some(entity) = front_matter.livingdocs.entity else {
        return Vec::new();
    };
    if facts.symbol_names.contains(&entity) || facts.file_paths.contains(&entity) {
        return Vec::new();
    }

    vec![Finding {
        file: file.to_string(),
        line: 1,
        rule: "missing-entity".to_string(),
        severity: Severity::Error,
        message: format!("documents \"{entity}\", which no longer exists in the graph"),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn facts_with_symbols(names: &[&str]) -> GraphFacts {
        GraphFacts {
            symbol_names: names.iter().map(|s| s.to_string()).collect(),
            file_paths: HashSet::new(),
            routes: HashSet::new(),
            external_packages: HashSet::new(),
        }
    }

    #[test]
    fn flags_entity_absent_from_graph() {
        let content = "---\nlivingdocs:\n  generated: true\n  kind: component\n  entity: AuditService\n---\n\nBody.\n";
        let findings = check(
            "docs/components/audit-service.md",
            content,
            &facts_with_symbols(&["UserService"]),
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "missing-entity");
    }

    #[test]
    fn does_not_flag_entity_present_in_graph() {
        let content = "---\nlivingdocs:\n  generated: true\n  kind: component\n  entity: UserService\n---\n\nBody.\n";
        let findings = check(
            "docs/components/user-service.md",
            content,
            &facts_with_symbols(&["UserService"]),
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_files_without_front_matter() {
        let findings = check(
            "docs/guide.md",
            "# Just a guide\n",
            &facts_with_symbols(&[]),
        );
        assert!(findings.is_empty());
    }
}
