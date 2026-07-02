use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::docs::managed_blocks;
use crate::docs::manifest::Manifest;
use crate::drift::findings::{Finding, Severity};
use crate::util::hash;

/// A managed block's current content hash doesn't match what the manifest
/// recorded at last sync — someone edited generated content directly
/// instead of going through `sync`/`update`. Not necessarily *wrong*
/// (a `Warning`, not an `Error`), but it means the tool's record of what
/// it last wrote is now a lie too.
pub fn check(root: &Path, manifest: &Manifest) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();

    for (block_id, manifest_block) in &manifest.blocks {
        let doc_path = root.join(&manifest_block.file);
        let Ok(content) = fs::read_to_string(&doc_path) else {
            continue; // missing doc file is a different problem, not this rule's
        };

        let Some(block) = managed_blocks::find_blocks(&content)
            .into_iter()
            .find(|b| &b.id == block_id)
        else {
            continue; // block itself missing — likewise not this rule's concern
        };

        let current_hash = hash::stable_id(&block.content);
        if current_hash != manifest_block.hash {
            findings.push(Finding {
                file: manifest_block.file.clone(),
                line: block.start_line,
                rule: "managed-block-edited".to_string(),
                severity: Severity::Warning,
                message: format!(
                    "block \"{block_id}\" was hand-edited; content no longer matches the last-synced hash"
                ),
            });
        }
    }

    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::manifest::ManifestBlock;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn flags_hash_mismatch() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::write(
            dir.path().join("docs/user-service.md"),
            "<!-- LIVINGDOCS:BEGIN id=\"x\" hash=\"stale\" -->\nEdited by hand.\n<!-- LIVINGDOCS:END id=\"x\" -->\n",
        )
        .unwrap();

        let mut manifest = Manifest::empty();
        manifest.blocks.insert(
            "x".to_string(),
            ManifestBlock {
                file: "docs/user-service.md".to_string(),
                entity: None,
                source: vec![],
                hash: hash::stable_id("Original generated content."),
                facts_hash: None,
            },
        );

        let findings = check(dir.path(), &manifest).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "managed-block-edited");
    }

    #[test]
    fn does_not_flag_matching_hash() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        let content = "Untouched content.";
        fs::write(
            dir.path().join("docs/user-service.md"),
            format!(
                "<!-- LIVINGDOCS:BEGIN id=\"x\" hash=\"y\" -->\n{content}\n<!-- LIVINGDOCS:END id=\"x\" -->\n"
            ),
        )
        .unwrap();

        let mut manifest = Manifest::empty();
        manifest.blocks.insert(
            "x".to_string(),
            ManifestBlock {
                file: "docs/user-service.md".to_string(),
                entity: None,
                source: vec![],
                hash: hash::stable_id(content),
                facts_hash: None,
            },
        );

        assert!(check(dir.path(), &manifest).unwrap().is_empty());
    }
}
