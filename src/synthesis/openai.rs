use anyhow::{Context, Result};
use async_openai::config::OpenAIConfig;
use async_openai::types::responses::CreateResponseArgs;
use async_openai::Client;

use super::provider::{Provider, SynthesizeOptions};

const SYSTEM_PROMPT: &str = "You are LivingDocs, a documentation synthesis engine. \
Given structured facts about a code entity as JSON, write concise, accurate prose \
describing it. Never invent behavior, dependencies, or methods that aren't present \
in the input. Write plain prose with no markdown headings.";

pub struct OpenAiProvider {
    client: Client<OpenAIConfig>,
}

impl OpenAiProvider {
    /// Reads `OPENAI_API_KEY` from the environment. Only constructed by
    /// commands that actually need synthesis — `check` never calls this,
    /// which is what keeps its "zero network calls" invariant true by
    /// construction rather than by discipline.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY is not set; synthesis requires an OpenAI API key")?;
        let config = OpenAIConfig::new().with_api_key(api_key);
        Ok(Self {
            client: Client::with_config(config),
        })
    }
}

impl Provider for OpenAiProvider {
    async fn synthesize(&self, prompt: &str, opts: &SynthesizeOptions) -> Result<String> {
        let request = CreateResponseArgs::default()
            .model(opts.model.as_str())
            .instructions(SYSTEM_PROMPT)
            .input(prompt)
            .max_output_tokens(opts.max_tokens)
            .build()
            .context("failed to build OpenAI request")?;

        let response = self
            .client
            .responses()
            .create(request)
            .await
            .context("OpenAI request failed")?;

        response
            .output_text()
            .filter(|s| !s.is_empty())
            .context("OpenAI response had no text output")
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use crate::synthesis::prompts::{build_prompt, GraphSlice};

    /// Real network call against the live OpenAI API. Ignored by default —
    /// the rest of the suite (and CI) stays hermetic. Run explicitly with:
    /// `OPENAI_API_KEY=sk-... cargo test --lib synthesis::openai::live_tests -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn synthesizes_against_the_real_api() {
        let provider = OpenAiProvider::from_env().expect("OPENAI_API_KEY must be set");
        let slice = GraphSlice {
            entity: "PolicyService".to_string(),
            kind: "class".to_string(),
            methods: vec!["quote".to_string()],
            dependencies: vec!["UserService".to_string()],
        };
        let prompt = build_prompt(&slice).unwrap();
        let opts = SynthesizeOptions {
            model: "gpt-4o-mini".to_string(),
            max_tokens: 200,
        };

        let result = provider.synthesize(&prompt, &opts).await.unwrap();
        println!("OpenAI response: {result}");
        assert!(!result.is_empty());
    }
}
