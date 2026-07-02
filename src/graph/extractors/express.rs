use std::path::Path;

use anyhow::{Context, Result};
use tree_sitter::{Node as TsNode, Parser};

use crate::parser::{Language, SourceRange};

use super::{ExtractedRoute, RouteExtractor};

const HTTP_METHODS: [&str; 5] = ["get", "post", "put", "delete", "patch"];
// DECISION: match only these conventional receiver names rather than any
// `.get(...)` call. Without type information we can't know a given
// identifier is really an Express app/router; this heuristic covers the
// vast majority of real Express code (including every Phase 6 demo repo)
// while avoiding false positives on unrelated `.get()`/`.post()` calls.
const APP_IDENTIFIERS: [&str; 3] = ["app", "router", "server"];

pub struct ExpressExtractor;

impl RouteExtractor for ExpressExtractor {
    fn name(&self) -> &'static str {
        "express"
    }

    fn extract(
        &self,
        file: &Path,
        source: &str,
        language: Language,
    ) -> Result<Vec<ExtractedRoute>> {
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

        let mut routes = Vec::new();
        walk(tree.root_node(), source.as_bytes(), file, &mut routes);
        Ok(routes)
    }
}

fn walk(node: TsNode, source: &[u8], file: &Path, out: &mut Vec<ExtractedRoute>) {
    if node.kind() == "call_expression" {
        if let Some(route) = try_extract_route(node, source, file) {
            out.push(route);
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, source, file, out);
    }
}

fn try_extract_route(call: TsNode, source: &[u8], file: &Path) -> Option<ExtractedRoute> {
    let func = call.child_by_field_name("function")?;
    if func.kind() != "member_expression" {
        return None;
    }

    let object = func.child_by_field_name("object")?;
    if object.kind() != "identifier" || !APP_IDENTIFIERS.contains(&object.utf8_text(source).ok()?) {
        return None;
    }

    let property = func.child_by_field_name("property")?;
    let method = property.utf8_text(source).ok()?;
    if !HTTP_METHODS.contains(&method) {
        return None;
    }

    let args = call.child_by_field_name("arguments")?;
    let mut cursor = args.walk();
    let arg_nodes: Vec<TsNode> = args.named_children(&mut cursor).collect();

    let path_node = arg_nodes.first()?;
    if path_node.kind() != "string" {
        return None;
    }
    let path = path_node
        .utf8_text(source)
        .ok()?
        .trim_matches(['\'', '"', '`'])
        .to_string();

    let handler = arg_nodes.last().and_then(|n| {
        (n.kind() == "identifier")
            .then(|| n.utf8_text(source).ok())
            .flatten()
            .map(str::to_string)
    });

    Some(ExtractedRoute {
        method: method.to_uppercase(),
        path,
        file: file.to_path_buf(),
        range: range_of(call),
        handler,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn extract(source: &str) -> Vec<ExtractedRoute> {
        ExpressExtractor
            .extract(&PathBuf::from("routes.js"), source, Language::JavaScript)
            .unwrap()
    }

    #[test]
    fn extracts_get_route_with_named_handler() {
        let routes = extract("app.get('/users', listUsers);");
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].method, "GET");
        assert_eq!(routes[0].path, "/users");
        assert_eq!(routes[0].handler.as_deref(), Some("listUsers"));
    }

    #[test]
    fn extracts_post_route_on_router_with_anonymous_handler() {
        let routes = extract("router.post(\"/users/:id\", auth, function (req, res) {});");
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].method, "POST");
        assert_eq!(routes[0].path, "/users/:id");
        assert_eq!(routes[0].handler, None);
    }

    #[test]
    fn ignores_unrelated_get_calls() {
        let routes = extract("const value = cache.get('key');");
        assert!(routes.is_empty());
    }

    #[test]
    fn ignores_unsupported_http_methods() {
        let routes = extract("app.listen(3000);");
        assert!(routes.is_empty());
    }
}
