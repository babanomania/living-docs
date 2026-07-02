use serde::{Deserialize, Serialize};

/// A generated doc file's `livingdocs:` front matter (see CLAUDE.md's
/// "Front matter (provenance + drift signals)"). Governs drift and
/// incremental sync — never hand-written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontMatter {
    pub livingdocs: LivingDocsMeta,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LivingDocsMeta {
    #[serde(default)]
    pub generated: bool,
    pub kind: String,
    #[serde(default)]
    pub entity: Option<String>,
    #[serde(default)]
    pub source: Vec<String>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub last_synced: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

/// Split a doc file's leading `---\n...\n---` YAML front matter from its
/// body. `None` if the file has none — plain user-authored docs (guides,
/// ADRs) aren't required to carry any.
pub fn parse(content: &str) -> Option<(FrontMatter, &str)> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let yaml = &rest[..end];
    let after_marker = &rest[end + 4..];
    let body = after_marker.strip_prefix('\n').unwrap_or(after_marker);

    let front_matter: FrontMatter = serde_yaml::from_str(yaml).ok()?;
    Some((front_matter, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_front_matter_and_returns_remaining_body() {
        let content = "---\nlivingdocs:\n  generated: true\n  kind: component\n  entity: UserService\n  source:\n    - src/user.ts\n---\n\n# Body\n";

        let (fm, body) = parse(content).expect("should parse front matter");
        assert!(fm.livingdocs.generated);
        assert_eq!(fm.livingdocs.kind, "component");
        assert_eq!(fm.livingdocs.entity.as_deref(), Some("UserService"));
        assert_eq!(fm.livingdocs.source, vec!["src/user.ts"]);
        assert_eq!(body, "\n# Body\n");
    }

    #[test]
    fn returns_none_when_no_front_matter() {
        assert!(parse("# Just a heading\n").is_none());
    }
}
