use anyhow::{bail, Result};

pub mod cache;
pub mod openai;
pub mod prompts;
pub mod provider;

use cache::Cache;
use provider::{Provider, SynthesizeOptions};

/// Per-run token/finding limits, from `livingdocs.config.json`'s `budget`.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub max_tokens: u32,
    pub max_findings: u32,
}

#[derive(Debug, Default)]
struct BudgetUsage {
    tokens_used: u32,
    findings_synthesized: u32,
}

/// Ties a `Provider` to a per-run `Budget` and an in-process `Cache`, so
/// callers (Phase 5's `update`/`sync`) get budget enforcement and
/// cache-hit skipping for free instead of reimplementing it per call site.
pub struct Synthesizer<P: Provider> {
    provider: P,
    budget: Budget,
    usage: BudgetUsage,
    cache: Cache,
}

impl<P: Provider> Synthesizer<P> {
    pub fn new(provider: P, budget: Budget) -> Self {
        Self {
            provider,
            budget,
            usage: BudgetUsage::default(),
            cache: Cache::new(),
        }
    }

    /// Synthesize `prompt`, or return the cached result if this exact
    /// prompt was already synthesized in this run. Errors once the run's
    /// token or finding budget would be exceeded, rather than silently
    /// truncating — callers should stop the loop, not keep calling.
    pub async fn synthesize(&mut self, prompt: &str, opts: &SynthesizeOptions) -> Result<String> {
        if let Some(cached) = self.cache.get(prompt) {
            return Ok(cached.clone());
        }

        if self.usage.findings_synthesized >= self.budget.max_findings {
            bail!(
                "budget exceeded: max {} findings per run",
                self.budget.max_findings
            );
        }
        if self.usage.tokens_used + opts.max_tokens > self.budget.max_tokens {
            bail!(
                "budget exceeded: max {} tokens per run",
                self.budget.max_tokens
            );
        }

        let result = self.provider.synthesize(prompt, opts).await?;

        self.usage.tokens_used += opts.max_tokens;
        self.usage.findings_synthesized += 1;
        self.cache.insert(prompt, result.clone());

        Ok(result)
    }
}

/// `model.default` for ad hoc/low-volume synthesis, `model.bulk` for the
/// scheduled `update` loop synthesizing many drifted nodes at once.
pub fn select_model(config: &crate::config::ModelConfig, bulk: bool) -> &str {
    if bulk {
        &config.bulk
    } else {
        &config.default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use provider::MockProvider;

    fn opts() -> SynthesizeOptions {
        SynthesizeOptions {
            model: "gpt-4.1".to_string(),
            max_tokens: 100,
        }
    }

    #[tokio::test]
    async fn returns_the_provider_response() {
        let mut synth = Synthesizer::new(
            MockProvider::new("UserService owns accounts."),
            Budget {
                max_tokens: 1000,
                max_findings: 10,
            },
        );

        let result = synth
            .synthesize("summarize UserService", &opts())
            .await
            .unwrap();
        assert_eq!(result, "UserService owns accounts.");
    }

    #[tokio::test]
    async fn second_call_with_same_prompt_is_a_cache_hit() {
        let mut synth = Synthesizer::new(
            MockProvider::new("cached prose"),
            Budget {
                max_tokens: 1000,
                max_findings: 10,
            },
        );

        synth.synthesize("same prompt", &opts()).await.unwrap();
        synth.synthesize("same prompt", &opts()).await.unwrap();

        assert_eq!(
            synth.provider.call_count(),
            1,
            "provider should only be called once"
        );
    }

    #[tokio::test]
    async fn different_prompts_each_call_the_provider() {
        let mut synth = Synthesizer::new(
            MockProvider::new("prose"),
            Budget {
                max_tokens: 1000,
                max_findings: 10,
            },
        );

        synth.synthesize("prompt a", &opts()).await.unwrap();
        synth.synthesize("prompt b", &opts()).await.unwrap();

        assert_eq!(synth.provider.call_count(), 2);
    }

    #[tokio::test]
    async fn stops_once_max_findings_reached() {
        let mut synth = Synthesizer::new(
            MockProvider::new("prose"),
            Budget {
                max_tokens: 1000,
                max_findings: 1,
            },
        );

        synth.synthesize("prompt a", &opts()).await.unwrap();
        let err = synth.synthesize("prompt b", &opts()).await.unwrap_err();
        assert!(err.to_string().contains("max 1 findings"));
        assert_eq!(synth.provider.call_count(), 1);
    }

    #[tokio::test]
    async fn stops_once_max_tokens_reached() {
        let mut synth = Synthesizer::new(
            MockProvider::new("prose"),
            Budget {
                max_tokens: 150,
                max_findings: 10,
            },
        );

        synth.synthesize("prompt a", &opts()).await.unwrap(); // uses 100
        let err = synth.synthesize("prompt b", &opts()).await.unwrap_err(); // would use 100 more
        assert!(err.to_string().contains("max 150 tokens"));
        assert_eq!(synth.provider.call_count(), 1);
    }

    #[test]
    fn select_model_picks_default_or_bulk() {
        let config = crate::config::ModelConfig {
            default: "gpt-4.1".to_string(),
            bulk: "gpt-4o-mini".to_string(),
        };
        assert_eq!(select_model(&config, false), "gpt-4.1");
        assert_eq!(select_model(&config, true), "gpt-4o-mini");
    }
}
