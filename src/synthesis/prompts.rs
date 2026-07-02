use serde::Serialize;

/// The "send facts, not source" payload (CLAUDE.md's Local Analysis,
/// OpenAI Synthesis principle): structured graph facts about one entity,
/// never raw file contents. Matches the shape CLAUDE.md shows verbatim:
/// `{"entity": "PolicyService", "type": "class", "methods": [...],
/// "dependencies": [...]}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraphSlice {
    pub entity: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub methods: Vec<String>,
    pub dependencies: Vec<String>,
}

/// Render a graph slice into the prompt sent to the model. Small and
/// deterministic on purpose — the slice is the only thing that varies.
pub fn build_prompt(slice: &GraphSlice) -> anyhow::Result<String> {
    let json = serde_json::to_string_pretty(slice)?;
    Ok(format!(
        "Summarize this component's responsibilities in 2-3 sentences, using only \
the facts below. Do not mention anything not listed here.\n\n{json}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_only_graph_facts_never_raw_source() {
        let slice = GraphSlice {
            entity: "PolicyService".to_string(),
            kind: "class".to_string(),
            methods: vec!["quote".to_string()],
            dependencies: vec!["UserService".to_string()],
        };

        let prompt = build_prompt(&slice).unwrap();
        assert!(prompt.contains("\"entity\": \"PolicyService\""));
        assert!(prompt.contains("\"quote\""));
        assert!(prompt.contains("\"UserService\""));
    }
}
