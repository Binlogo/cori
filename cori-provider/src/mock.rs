//! Scripted mock provider for testing.
//!
//! `MockProvider` replays a preset sequence of [`LlmResponse`] values in order.
//! It implements both `Llm` (non-streaming) and `StreamingLlm` (simulates
//! streaming by delivering the full text in a single `on_text` call).
//!
//! This is similar to [`cori_core::loop_::MockLlm`] but is built on top of
//! `ProviderConfig` and lives in `cori-provider` so it can be used alongside
//! the production providers.

use std::collections::VecDeque;
use std::sync::Mutex;

use cori_core::loop_::{Llm, LlmResponse, StreamingLlm};
use cori_core::types::Message;

// ── MockProvider ──────────────────────────────────────────────────────────────

/// A scripted LLM provider that replays preset `LlmResponse` values.
///
/// Each call to `send()` or `send_streaming()` pops the front of the
/// internal queue and returns it. When the queue is exhausted, both methods
/// return an error.
#[derive(Debug)]
pub struct MockProvider {
    responses: Mutex<VecDeque<LlmResponse>>,
}

impl MockProvider {
    /// Create a new `MockProvider` that will replay `responses` in order.
    pub fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
        }
    }

    fn pop_next(&self) -> Result<LlmResponse, anyhow::Error> {
        let mut guard = self.responses.lock().unwrap();
        guard
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("MockProvider: no responses left in queue"))
    }
}

// ── Llm impl ──────────────────────────────────────────────────────────────────

impl Llm for MockProvider {
    async fn send(&self, _messages: &[Message]) -> Result<LlmResponse, anyhow::Error> {
        self.pop_next()
    }
}

// ── StreamingLlm impl ─────────────────────────────────────────────────────────

impl StreamingLlm for MockProvider {
    /// Simulated streaming: calls `on_text` once with the full text (if any),
    /// then returns the response as-is.
    async fn send_streaming<F>(
        &self,
        _messages: &[Message],
        on_text: F,
    ) -> Result<LlmResponse, anyhow::Error>
    where
        F: Fn(&str) + Send,
    {
        let response = self.pop_next()?;
        if let Some(ref text) = response.text {
            on_text(text);
        }
        Ok(response)
    }
}
