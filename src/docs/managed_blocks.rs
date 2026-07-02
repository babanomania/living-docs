/// One `<!-- LIVINGDOCS:BEGIN id=".." hash=".." --> ... <!-- LIVINGDOCS:END
/// id=".." -->` span. `content` is the raw text between the markers,
/// trimmed — the same trimming used when the hash was originally computed,
/// so a re-hash of unchanged content always matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedBlock {
    pub id: String,
    pub hash: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// Find every managed block in `content`. Unmatched or malformed markers
/// (e.g. a doc mid-edit) are skipped rather than erroring — `check` should
/// degrade gracefully, not crash, on a doc a human is actively touching.
pub fn find_blocks(content: &str) -> Vec<ManagedBlock> {
    let lines: Vec<&str> = content.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        if let Some((id, hash)) = parse_begin_marker(lines[i]) {
            let start_line = i + 1; // 1-based
            let mut j = i + 1;
            let mut inner = Vec::new();
            let mut end_line = None;

            while j < lines.len() {
                if parse_end_marker(lines[j]).as_deref() == Some(id.as_str()) {
                    end_line = Some(j + 1);
                    break;
                }
                inner.push(lines[j]);
                j += 1;
            }

            if let Some(end_line) = end_line {
                blocks.push(ManagedBlock {
                    id,
                    hash,
                    content: inner.join("\n").trim().to_string(),
                    start_line,
                    end_line,
                });
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }

    blocks
}

fn parse_begin_marker(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if !line.starts_with("<!-- LIVINGDOCS:BEGIN") || !line.ends_with("-->") {
        return None;
    }
    Some((extract_attr(line, "id")?, extract_attr(line, "hash")?))
}

fn parse_end_marker(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with("<!-- LIVINGDOCS:END") || !line.ends_with("-->") {
        return None;
    }
    extract_attr(line, "id")
}

fn extract_attr(line: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_single_block() {
        let content = "\
Some prose.

<!-- LIVINGDOCS:BEGIN id=\"user-service.responsibilities\" hash=\"abc123\" -->
UserService owns user accounts.
<!-- LIVINGDOCS:END id=\"user-service.responsibilities\" -->

More prose.
";
        let blocks = find_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, "user-service.responsibilities");
        assert_eq!(blocks[0].hash, "abc123");
        assert_eq!(blocks[0].content, "UserService owns user accounts.");
        assert_eq!(blocks[0].start_line, 3);
        assert_eq!(blocks[0].end_line, 5);
    }

    #[test]
    fn ignores_unterminated_block() {
        let content = "<!-- LIVINGDOCS:BEGIN id=\"x\" hash=\"y\" -->\nno end marker\n";
        assert!(find_blocks(content).is_empty());
    }

    #[test]
    fn finds_multiple_blocks() {
        let content = "\
<!-- LIVINGDOCS:BEGIN id=\"a\" hash=\"1\" -->
A
<!-- LIVINGDOCS:END id=\"a\" -->

<!-- LIVINGDOCS:BEGIN id=\"b\" hash=\"2\" -->
B
<!-- LIVINGDOCS:END id=\"b\" -->
";
        let blocks = find_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].id, "a");
        assert_eq!(blocks[1].id, "b");
    }
}
