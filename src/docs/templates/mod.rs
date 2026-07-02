use anyhow::Result;

use crate::docs::frontmatter::{FrontMatter, LivingDocsMeta};

pub mod apis;
pub mod architecture;
pub mod component;
pub mod dependencies;
pub mod diagrams;
pub mod index;
pub mod overview;

/// Render a `livingdocs:` front-matter block for a generated file.
pub fn render_frontmatter(meta: &LivingDocsMeta) -> Result<String> {
    let front_matter = FrontMatter {
        livingdocs: meta.clone(),
    };
    let yaml = serde_yaml::to_string(&front_matter)?;
    Ok(format!("---\n{yaml}---\n"))
}

/// A markdown bullet list, one item per line, or a placeholder line when
/// empty — CLAUDE.md's generated sections never render as blank space.
pub fn bullet_list(items: &[String], empty_message: &str) -> String {
    if items.is_empty() {
        return format!("_{empty_message}_");
    }
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::frontmatter;

    #[test]
    fn rendered_frontmatter_round_trips_through_parse() {
        let meta = LivingDocsMeta {
            generated: true,
            kind: "component".to_string(),
            entity: Some("UserService".to_string()),
            source: vec!["src/user.ts".to_string()],
            commit: Some("abc123".to_string()),
            last_synced: Some("2026-01-01T00:00:00Z".to_string()),
            model: Some("gpt-4.1".to_string()),
        };

        let rendered = render_frontmatter(&meta).unwrap();
        let full = format!("{rendered}\nBody.\n");
        let (parsed, body) = frontmatter::parse(&full).unwrap();

        assert_eq!(parsed.livingdocs, meta);
        assert_eq!(body, "\nBody.\n");
    }

    #[test]
    fn bullet_list_renders_placeholder_when_empty() {
        assert_eq!(bullet_list(&[], "none found"), "_none found_");
    }

    #[test]
    fn bullet_list_renders_items() {
        let items = vec!["a".to_string(), "b".to_string()];
        assert_eq!(bullet_list(&items, "none"), "- a\n- b");
    }
}
