use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// `.livingdocs/manifest.json` — maps each managed block to the graph
/// entity/source it was generated from and its content hash at last sync.
/// `check` walks this to find drift without touching OpenAI; `sync`/
/// `update` (Phase 5) write it. A `BTreeMap` keeps block ordering (and
/// therefore JSON output) deterministic across runs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    #[serde(rename = "lastSynced", skip_serializing_if = "Option::is_none")]
    pub last_synced: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default)]
    pub blocks: BTreeMap<String, ManifestBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestBlock {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    #[serde(default)]
    pub source: Vec<String>,
    /// Hash of this block's *rendered content* — used by `check`'s
    /// `managed-block-edited` rule to detect a hand edit (Phase 3).
    pub hash: String,
    /// Hash of the *graph slice* used to synthesize this block, distinct
    /// from `hash` above. Lets `sync` skip re-synthesis (and stay
    /// byte-deterministic despite the LLM being non-deterministic) when
    /// the underlying facts haven't changed. `hash` alone can't serve
    /// both purposes: it changes with synthesis *output* even when the
    /// *input* facts are identical, and vice versa for a hand edit.
    #[serde(rename = "factsHash", default, skip_serializing_if = "Option::is_none")]
    pub facts_hash: Option<String>,
}

impl Manifest {
    /// Version 1, no blocks — what `check` uses when no manifest exists
    /// yet (e.g. before the first `sync`). Not having synced anything
    /// isn't an error; it just means there's nothing generated to check.
    pub fn empty() -> Manifest {
        Manifest {
            version: 1,
            last_synced: None,
            commit: None,
            blocks: BTreeMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Manifest> {
        if !path.exists() {
            return Ok(Manifest::empty());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read manifest at {}", path.display()))?;
        serde_json::from_str(&text)
            .with_context(|| format!("failed to parse manifest at {}", path.display()))
    }

    #[allow(dead_code)] // DECISION: writer has no caller until Phase 5's sync/update land.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, format!("{json}\n"))
            .with_context(|| format!("failed to write manifest at {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_manifest_loads_as_empty() {
        let dir = tempdir().unwrap();
        let manifest = Manifest::load(&dir.path().join("manifest.json")).unwrap();
        assert_eq!(manifest, Manifest::empty());
    }

    #[test]
    fn round_trips_through_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".livingdocs/manifest.json");

        let mut manifest = Manifest::empty();
        manifest.blocks.insert(
            "user-service.responsibilities".to_string(),
            ManifestBlock {
                file: "docs/components/user-service.md".to_string(),
                entity: Some("UserService".to_string()),
                source: vec!["src/user.ts".to_string()],
                hash: "abc123".to_string(),
                facts_hash: Some("def456".to_string()),
            },
        );

        manifest.save(&path).unwrap();
        let loaded = Manifest::load(&path).unwrap();
        assert_eq!(loaded, manifest);
    }
}
