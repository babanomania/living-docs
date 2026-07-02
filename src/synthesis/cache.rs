use std::collections::HashMap;

use crate::util::hash;

/// In-process cache keyed by the blake3 hash of a synthesis prompt (built
/// from a graph slice — CLAUDE.md's "cache synthesis by node content
/// hash"). Cross-invocation skipping doesn't need a separate persisted
/// cache file: the manifest's per-block `hash` (Phase 3) already tells
/// `sync`/`update` whether a block's source changed since last sync, so
/// they can skip calling `synthesize` at all on a hit. This cache only
/// guards against synthesizing the *same* slice twice within one run.
#[derive(Debug, Default)]
pub struct Cache {
    entries: HashMap<String, String>,
}

impl Cache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, prompt: &str) -> Option<&String> {
        self.entries.get(&hash::stable_id(prompt))
    }

    pub fn insert(&mut self, prompt: &str, result: String) {
        self.entries.insert(hash::stable_id(prompt), result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_after_insert_miss_before() {
        let mut cache = Cache::new();
        assert!(cache.get("prompt a").is_none());

        cache.insert("prompt a", "result a".to_string());
        assert_eq!(cache.get("prompt a"), Some(&"result a".to_string()));
        assert!(cache.get("prompt b").is_none());
    }
}
