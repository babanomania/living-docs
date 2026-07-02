use std::future::Future;

/// Model + token-budget knobs for one `synthesize` call.
#[derive(Debug, Clone)]
pub struct SynthesizeOptions {
    pub model: String,
    pub max_tokens: u32,
}

/// Turns a prompt into prose. `openai::OpenAiProvider` is the only
/// production implementation; everything downstream (Phase 5's
/// `update`/`sync`, Phase 7's `explain`) is written against this trait so
/// it can run against `MockProvider` in tests without touching the network.
pub trait Provider {
    fn synthesize(
        &self,
        prompt: &str,
        opts: &SynthesizeOptions,
    ) -> impl Future<Output = anyhow::Result<String>>;
}

#[cfg(test)]
pub(crate) struct MockProvider {
    response: String,
    calls: std::cell::RefCell<u32>,
}

#[cfg(test)]
impl MockProvider {
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
            calls: std::cell::RefCell::new(0),
        }
    }

    pub fn call_count(&self) -> u32 {
        *self.calls.borrow()
    }
}

#[cfg(test)]
impl Provider for MockProvider {
    async fn synthesize(&self, _prompt: &str, _opts: &SynthesizeOptions) -> anyhow::Result<String> {
        *self.calls.borrow_mut() += 1;
        Ok(self.response.clone())
    }
}
