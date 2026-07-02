use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

use crate::drift::findings::{Finding, Severity};
use crate::drift::{line_of, GraphFacts};

/// A node in a ```mermaid fenced block's `A --> B` edges that doesn't
/// match any known symbol — the diagram still shows a component that no
/// longer exists in the code.
///
/// DECISION: only the `graph TD`/`graph LR` component-diagram style shown
/// in CLAUDE.md (plain `-->` edges) is handled. classDiagram/sequenceDiagram
/// use different arrow syntax (`--|>`, `->>`) and are out of scope for
/// Phase 3. Purely architectural nodes with no 1:1 code symbol (e.g. an
/// abstract "Frontend" box) aren't flagged as false drift since they're
/// simply never checked against symbol names in the first place — only
/// nodes matching real symbol-name shapes get checked here, but this rule
/// still requires diagram nodes to literally be graph symbol names to
/// avoid flagging every architectural box as "gone."
pub fn check(file: &str, content: &str, facts: &GraphFacts) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut in_mermaid = false;

    for (event, range) in Parser::new(content).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(kind)) if is_mermaid(&kind) => in_mermaid = true,
            Event::End(TagEnd::CodeBlock) => in_mermaid = false,
            Event::Text(text) if in_mermaid => {
                let block_start_line = line_of(content, range.start);
                for (offset, line) in text.lines().enumerate() {
                    for node in edge_nodes(line) {
                        if !facts.symbol_names.contains(node) {
                            findings.push(Finding {
                                file: file.to_string(),
                                line: block_start_line + offset,
                                rule: "diagram-node-gone".to_string(),
                                severity: Severity::Error,
                                message: format!("diagram references \"{node}\", not in graph"),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    findings
}

fn is_mermaid(kind: &CodeBlockKind) -> bool {
    matches!(kind, CodeBlockKind::Fenced(lang) if lang.as_ref() == "mermaid")
}

/// Extract the two node identifiers around a `-->` edge, e.g.
/// `A --> B` or `A -->|label| B`. Neither side is required to be a known
/// symbol already — that's what this rule is checking.
fn edge_nodes(line: &str) -> Vec<&str> {
    let line = line.trim();
    let Some(arrow_pos) = line.find("-->") else {
        return Vec::new();
    };

    let left = line[..arrow_pos].trim();
    let mut right = line[arrow_pos + 3..].trim();
    if let Some(rest) = right.strip_prefix('|') {
        right = rest.split_once('|').map_or("", |(_, after)| after.trim());
    }

    [left, right]
        .into_iter()
        .filter(|n| is_identifier(n))
        .collect()
}

fn is_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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
    fn flags_gone_diagram_node() {
        let content = "```mermaid\ngraph TD\nUserService --> AuditService\n```\n";
        let findings = check(
            "docs/diagrams/component.md",
            content,
            &facts_with_symbols(&["UserService"]),
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "diagram-node-gone");
        assert!(findings[0].message.contains("AuditService"));
    }

    #[test]
    fn does_not_flag_when_both_nodes_exist() {
        let content = "```mermaid\ngraph TD\nUserService --> PolicyService\n```\n";
        let findings = check(
            "docs/diagrams/component.md",
            content,
            &facts_with_symbols(&["UserService", "PolicyService"]),
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_non_mermaid_code_blocks() {
        let content = "```text\nUserService --> AuditService\n```\n";
        let findings = check(
            "docs/diagrams/component.md",
            content,
            &facts_with_symbols(&[]),
        );
        assert!(findings.is_empty());
    }
}
