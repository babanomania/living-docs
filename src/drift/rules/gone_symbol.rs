use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

use crate::drift::findings::{Finding, Severity};
use crate::drift::{line_of, GraphFacts};

/// A backtick-quoted, PascalCase identifier (our convention for
/// referencing a class/interface in prose, e.g. `` `UserService` ``) that
/// no longer names any symbol in the graph. Scoped to PascalCase to avoid
/// false positives on the many other things people put in inline code
/// spans — config keys, CLI flags, generic lowercase words. Mermaid blocks
/// are excluded here; that's `diagram_node_gone`'s job.
pub fn check(file: &str, content: &str, facts: &GraphFacts) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut mermaid_depth = 0usize;

    for (event, range) in Parser::new(content).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(kind)) if is_mermaid(&kind) => mermaid_depth += 1,
            Event::End(TagEnd::CodeBlock) if mermaid_depth > 0 => mermaid_depth -= 1,
            Event::Code(text)
                if mermaid_depth == 0
                    && is_pascal_case(&text)
                    && !facts.symbol_names.contains(text.as_ref()) =>
            {
                findings.push(Finding {
                    file: file.to_string(),
                    line: line_of(content, range.start),
                    rule: "gone-symbol".to_string(),
                    severity: Severity::Error,
                    message: format!("references `{text}`, no symbol named \"{text}\" in graph"),
                });
            }
            _ => {}
        }
    }

    findings
}

fn is_mermaid(kind: &CodeBlockKind) -> bool {
    matches!(kind, CodeBlockKind::Fenced(lang) if lang.as_ref() == "mermaid")
}

fn is_pascal_case(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_uppercase())
        && text.len() > 1
        && text.chars().all(|c| c.is_ascii_alphanumeric())
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
    fn flags_gone_pascal_case_symbol() {
        let content = "The `AuditService` logs writes.\n";
        let findings = check(
            "docs/architecture.md",
            content,
            &facts_with_symbols(&["UserService"]),
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "gone-symbol");
        assert_eq!(findings[0].line, 1);
    }

    #[test]
    fn does_not_flag_existing_symbol() {
        let content = "The `UserService` owns accounts.\n";
        let findings = check(
            "docs/architecture.md",
            content,
            &facts_with_symbols(&["UserService"]),
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_lowercase_inline_code() {
        let content = "Set `config` before calling `bootstrap`.\n";
        let findings = check("docs/architecture.md", content, &facts_with_symbols(&[]));
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_pascal_case_inside_mermaid_block() {
        let content = "```mermaid\ngraph TD\nUserService --> AuditService\n```\n";
        let findings = check(
            "docs/architecture.md",
            content,
            &facts_with_symbols(&["UserService"]),
        );
        assert!(findings.is_empty());
    }
}
