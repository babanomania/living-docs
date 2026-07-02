use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Node as TsNode, Parser};

use super::{
    ClassNode, FunctionNode, ImportNode, InterfaceNode, Language, ParsedFile, SourceRange,
};

/// Parse `source` (already known to be `language`) into its symbols.
/// Shared between TS/TSX/JS since all three grammars use the same node
/// and field names for the constructs we extract.
pub fn parse(file: &Path, source: &str, language: Language) -> Result<ParsedFile> {
    let mut parser = Parser::new();
    let ts_language = match language {
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
        Language::JavaScript => tree_sitter_javascript::LANGUAGE,
    };
    parser
        .set_language(&ts_language.into())
        .context("failed to load tree-sitter grammar")?;

    let tree = parser
        .parse(source, None)
        .context("tree-sitter produced no parse tree")?;

    let mut out = ParsedFile::default();
    walk(tree.root_node(), source.as_bytes(), file, false, &mut out);
    Ok(out)
}

fn walk(node: TsNode, source: &[u8], file: &Path, exported: bool, out: &mut ParsedFile) {
    match node.kind() {
        "export_statement" => {
            // `export`'s declaration child is exported; everything else
            // under it (named export clauses, etc.) is not a declaration
            // we track, but still worth descending into for completeness.
            let decl_id = node.child_by_field_name("declaration").map(|d| d.id());
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let child_exported = Some(child.id()) == decl_id;
                walk(child, source, file, child_exported, out);
            }
            return;
        }
        "class_declaration" => {
            if let Some(name) = child_text(node, "name", source) {
                out.classes.push(ClassNode {
                    name,
                    file: file.to_path_buf(),
                    range: range_of(node),
                    methods: collect_methods(node, source),
                    exported,
                });
            }
        }
        "function_declaration" => {
            if let Some(name) = child_text(node, "name", source) {
                out.functions.push(FunctionNode {
                    name,
                    file: file.to_path_buf(),
                    range: range_of(node),
                    exported,
                });
            }
        }
        "interface_declaration" => {
            if let Some(name) = child_text(node, "name", source) {
                out.interfaces.push(InterfaceNode {
                    name,
                    file: file.to_path_buf(),
                    range: range_of(node),
                    exported,
                });
            }
        }
        "import_statement" => {
            if let Some(import) = build_import(node, source, file) {
                out.imports.push(import);
            }
            return; // nothing inside an import is worth descending into
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, source, file, false, out);
    }
}

fn child_text(node: TsNode, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|n| n.utf8_text(source).ok())
        .map(str::to_string)
}

fn range_of(node: TsNode) -> SourceRange {
    let start = node.start_position();
    let end = node.end_position();
    SourceRange {
        start_line: start.row + 1,
        end_line: end.row + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    }
}

/// Method names declared directly on this class (not in a nested class).
fn collect_methods(class_node: TsNode, source: &[u8]) -> Vec<String> {
    let Some(body) = class_node.child_by_field_name("body") else {
        return Vec::new();
    };
    let mut methods = Vec::new();
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "method_definition" {
            if let Some(name) = child_text(child, "name", source) {
                methods.push(name);
            }
        }
    }
    methods
}

fn build_import(node: TsNode, source: &[u8], file: &Path) -> Option<ImportNode> {
    let module = node
        .child_by_field_name("source")
        .and_then(|n| n.utf8_text(source).ok())
        .map(|s| s.trim_matches(['\'', '"', '`']).to_string())?;

    let mut specifiers = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "import_clause" {
            collect_specifiers(child, source, &mut specifiers);
        }
    }

    Some(ImportNode {
        file: file.to_path_buf(),
        range: range_of(node),
        source: module,
        specifiers,
    })
}

fn collect_specifiers(clause: TsNode, source: &[u8], out: &mut Vec<String>) {
    let mut cursor = clause.walk();
    for child in clause.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                if let Ok(text) = child.utf8_text(source) {
                    out.push(text.to_string());
                }
            }
            "named_imports" => {
                let mut inner = child.walk();
                for spec in child.children(&mut inner) {
                    if spec.kind() != "import_specifier" {
                        continue;
                    }
                    let name = spec
                        .child_by_field_name("alias")
                        .or_else(|| spec.child_by_field_name("name"))
                        .and_then(|n| n.utf8_text(source).ok());
                    if let Some(name) = name {
                        out.push(name.to_string());
                    }
                }
            }
            "namespace_import" => {
                if let Some(id) = child.named_child(0) {
                    if let Ok(text) = id.utf8_text(source) {
                        out.push(format!("* as {text}"));
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse_ts(source: &str) -> ParsedFile {
        parse(&PathBuf::from("test.ts"), source, Language::TypeScript).unwrap()
    }

    fn parse_js(source: &str) -> ParsedFile {
        parse(&PathBuf::from("test.js"), source, Language::JavaScript).unwrap()
    }

    #[test]
    fn extracts_class_with_methods() {
        let parsed = parse_ts(
            r#"
            class UserService {
                create() {}
                delete() {}
            }
            "#,
        );
        assert_eq!(parsed.classes.len(), 1);
        let class = &parsed.classes[0];
        assert_eq!(class.name, "UserService");
        assert_eq!(class.methods, vec!["create", "delete"]);
        assert!(!class.exported);
        assert_eq!(class.range.start_line, 2);
    }

    #[test]
    fn extracts_exported_class() {
        let parsed = parse_ts("export class PolicyService {}");
        assert_eq!(parsed.classes.len(), 1);
        assert!(parsed.classes[0].exported);
    }

    #[test]
    fn extracts_function_declaration() {
        let parsed = parse_ts("export function double(x: number): number { return x * 2; }");
        assert_eq!(parsed.functions.len(), 1);
        assert_eq!(parsed.functions[0].name, "double");
        assert!(parsed.functions[0].exported);
    }

    #[test]
    fn extracts_interface_declaration() {
        let parsed = parse_ts("export interface Policy { id: string; }");
        assert_eq!(parsed.interfaces.len(), 1);
        assert_eq!(parsed.interfaces[0].name, "Policy");
        assert!(parsed.interfaces[0].exported);
    }

    #[test]
    fn javascript_has_no_interfaces() {
        let parsed = parse_js("class Foo {}");
        assert_eq!(parsed.classes.len(), 1);
        assert!(parsed.interfaces.is_empty());
    }

    #[test]
    fn extracts_named_and_default_imports() {
        let parsed = parse_ts(
            r#"
            import UserService from "./user-service";
            import { PolicyService, AuditService as Audit } from "./services";
            import * as utils from "./utils";
            "#,
        );
        assert_eq!(parsed.imports.len(), 3);
        assert_eq!(parsed.imports[0].source, "./user-service");
        assert_eq!(parsed.imports[0].specifiers, vec!["UserService"]);
        assert_eq!(parsed.imports[1].source, "./services");
        assert_eq!(parsed.imports[1].specifiers, vec!["PolicyService", "Audit"]);
        assert_eq!(parsed.imports[2].source, "./utils");
        assert_eq!(parsed.imports[2].specifiers, vec!["* as utils"]);
    }

    #[test]
    fn source_ranges_cover_the_whole_declaration() {
        let source = "function first() {}\nfunction second() {}\n";
        let parsed = parse_ts(source);
        assert_eq!(parsed.functions.len(), 2);
        assert_eq!(parsed.functions[0].range.start_line, 1);
        assert_eq!(parsed.functions[1].range.start_line, 2);
        assert_eq!(
            &source[parsed.functions[0].range.start_byte..parsed.functions[0].range.end_byte],
            "function first() {}"
        );
    }
}
