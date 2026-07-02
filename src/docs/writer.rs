use crate::docs::managed_blocks;
use crate::util::hash;

/// Insert or replace each `(id, body)` pair as a managed block in
/// `content`, recomputing that block's `hash` attribute from `body`.
/// Existing blocks not present in `updates` are copied through verbatim,
/// as is everything outside block markers — CLAUDE.md's "user-authored
/// content outside them is never touched." Blocks in `updates` with no
/// existing match are appended at the end.
pub fn upsert_blocks(content: &str, updates: &[(String, String)]) -> String {
    let existing = managed_blocks::find_blocks(content);
    let lines: Vec<&str> = content.lines().collect();

    let mut result = String::new();
    let mut consumed_to = 0usize;
    let mut written_ids = std::collections::HashSet::new();

    for block in &existing {
        for line in &lines[consumed_to..block.start_line - 1] {
            result.push_str(line);
            result.push('\n');
        }

        if let Some((_, body)) = updates.iter().find(|(id, _)| *id == block.id) {
            write_block(&mut result, &block.id, body);
            written_ids.insert(block.id.clone());
        } else {
            for line in &lines[block.start_line - 1..block.end_line] {
                result.push_str(line);
                result.push('\n');
            }
        }
        consumed_to = block.end_line;
    }

    for line in &lines[consumed_to..] {
        result.push_str(line);
        result.push('\n');
    }

    for (id, body) in updates {
        if !written_ids.contains(id) {
            if !result.is_empty() && !result.ends_with("\n\n") {
                if !result.ends_with('\n') {
                    result.push('\n');
                }
                result.push('\n');
            }
            write_block(&mut result, id, body);
        }
    }

    result
}

fn write_block(out: &mut String, id: &str, body: &str) {
    let body = body.trim();
    let block_hash = hash::stable_id(body);
    out.push_str(&format!(
        "<!-- LIVINGDOCS:BEGIN id=\"{id}\" hash=\"{block_hash}\" -->\n"
    ));
    out.push_str(body);
    out.push('\n');
    out.push_str(&format!("<!-- LIVINGDOCS:END id=\"{id}\" -->\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_a_new_block_to_empty_content() {
        let result = upsert_blocks("", &[("a".to_string(), "Body A.".to_string())]);
        let expected_hash = hash::stable_id("Body A.");
        assert_eq!(
            result,
            format!(
                "<!-- LIVINGDOCS:BEGIN id=\"a\" hash=\"{expected_hash}\" -->\nBody A.\n<!-- LIVINGDOCS:END id=\"a\" -->\n"
            )
        );
    }

    #[test]
    fn replaces_existing_block_content_and_hash() {
        let content = "# Title\n\n<!-- LIVINGDOCS:BEGIN id=\"a\" hash=\"stale\" -->\nOld body.\n<!-- LIVINGDOCS:END id=\"a\" -->\n\nTrailing prose.\n";
        let result = upsert_blocks(content, &[("a".to_string(), "New body.".to_string())]);

        assert!(result.contains("New body."));
        assert!(!result.contains("Old body."));
        assert!(!result.contains("hash=\"stale\""));
    }

    #[test]
    fn preserves_content_outside_blocks() {
        let content = "# Title\n\nHand-written intro.\n\n<!-- LIVINGDOCS:BEGIN id=\"a\" hash=\"x\" -->\nOld.\n<!-- LIVINGDOCS:END id=\"a\" -->\n\nHand-written outro.\n";
        let result = upsert_blocks(content, &[("a".to_string(), "New.".to_string())]);

        assert!(result.contains("# Title"));
        assert!(result.contains("Hand-written intro."));
        assert!(result.contains("Hand-written outro."));
    }

    #[test]
    fn leaves_blocks_not_in_updates_untouched() {
        let content = "<!-- LIVINGDOCS:BEGIN id=\"a\" hash=\"x\" -->\nA.\n<!-- LIVINGDOCS:END id=\"a\" -->\n<!-- LIVINGDOCS:BEGIN id=\"b\" hash=\"y\" -->\nB.\n<!-- LIVINGDOCS:END id=\"b\" -->\n";
        let result = upsert_blocks(content, &[("a".to_string(), "A2.".to_string())]);

        assert!(result.contains("A2."));
        assert!(result.contains("hash=\"y\""));
        assert!(result.contains("B."));
    }

    #[test]
    fn identical_body_produces_byte_identical_output() {
        let content = upsert_blocks("", &[("a".to_string(), "Same body.".to_string())]);
        let resynced = upsert_blocks(&content, &[("a".to_string(), "Same body.".to_string())]);
        assert_eq!(content, resynced);
    }
}
