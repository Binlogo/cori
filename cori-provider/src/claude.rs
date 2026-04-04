//! Anthropic Claude provider implementation.
//!
//! `ClaudeProvider` implements both `Llm` and `StreamingLlm` using the
//! Anthropic Messages API. It reads configuration from a `ProviderConfig`
//! (or directly from environment variables via `from_env()`).
//!
//! ## Retry logic
//!
//! On HTTP 429 (rate limit) or 529 (overloaded), the provider waits
//! `2^attempt` seconds and retries up to 3 times before propagating the error.

use std::time::Duration;

use futures_util::StreamExt;

use cori_core::{
    config::ProviderConfig,
    loop_::{Llm, LlmResponse, StreamingLlm, Usage},
    types::{Message, ToolUse},
};

// ── SSE event types (internal) ────────────────────────────────────────────────

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum StreamEvent {
    MessageStart { message: MessageStartData },
    ContentBlockStart { index: usize, content_block: ContentBlock },
    ContentBlockDelta { index: usize, delta: Delta },
    ContentBlockStop { index: usize },
    MessageDelta { delta: MessageDeltaData, usage: DeltaUsage },
    MessageStop,
    Ping,
    #[serde(other)]
    Unknown,
}

#[derive(serde::Deserialize, Debug)]
struct MessageStartData {
    usage: StartUsage,
}

#[derive(serde::Deserialize, Debug)]
struct StartUsage {
    input_tokens: u32,
}

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String },
}

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Delta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
}

#[derive(serde::Deserialize, Debug)]
struct MessageDeltaData {
    stop_reason: String,
}

#[derive(serde::Deserialize, Debug)]
struct DeltaUsage {
    output_tokens: u32,
}

// ── Internal streaming state ──────────────────────────────────────────────────

struct BlockState {
    kind: BlockKind,
}

enum BlockKind {
    Text { accumulated: String },
    ToolUse { id: String, name: String, json_buf: String },
}

// ── Non-streaming API response types (internal) ───────────────────────────────

#[derive(serde::Deserialize, Debug)]
struct ApiUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(serde::Deserialize, Debug)]
struct ApiResponse {
    stop_reason: String,
    content: Vec<ApiContent>,
    usage: ApiUsage,
}

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApiContent {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    #[serde(other)]
    Unknown,
}

// ── ClaudeProvider ────────────────────────────────────────────────────────────

/// An Anthropic Claude provider that implements `Llm` and `StreamingLlm`.
///
/// Construct via [`ClaudeProvider::from_env()`] or by passing a
/// [`ProviderConfig`] to [`ClaudeProvider::new()`]. Add tool schemas with
/// [`ClaudeProvider::with_tools()`].
#[derive(Debug)]
pub struct ClaudeProvider {
    config: ProviderConfig,
    tools: Vec<serde_json::Value>,
    client: reqwest::Client,
}

impl ClaudeProvider {
    /// Construct a new provider from the given configuration (no tools).
    pub fn new(config: ProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();
        Self { config, tools: vec![], client }
    }

    /// Read configuration from environment variables and build a provider.
    ///
    /// Reads `ANTHROPIC_API_KEY` (required), `ANTHROPIC_BASE_URL` and
    /// `ANTHROPIC_MODEL` (optional).
    pub fn from_env() -> Result<Self, anyhow::Error> {
        let config = ProviderConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Set the tool schemas to advertise to the model (builder-style).
    pub fn with_tools(mut self, tools: Vec<serde_json::Value>) -> Self {
        self.tools = tools;
        self
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn messages_url(&self) -> String {
        self.config.messages_url()
    }

    /// Send a raw (non-streaming) POST to the Messages API, with retry on
    /// 429/529 using exponential back-off (2^attempt seconds, max 3 retries).
    async fn send_with_retry(&self, body: &serde_json::Value) -> Result<reqwest::Response, anyhow::Error> {
        const MAX_RETRIES: u32 = 3;
        let mut attempt = 0u32;

        loop {
            let resp = self
                .client
                .post(self.messages_url())
                .header("x-api-key", &self.config.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(body)
                .send()
                .await?;

            let status = resp.status();
            if status == 429 || status == 529 {
                if attempt >= MAX_RETRIES {
                    let text = resp.text().await?;
                    anyhow::bail!("API error {status} after {attempt} retries: {text}");
                }
                let wait_secs = 2u64.pow(attempt);
                tracing::warn!(
                    "Claude API returned {status}, retrying in {wait_secs}s (attempt {}/{})",
                    attempt + 1,
                    MAX_RETRIES
                );
                tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                attempt += 1;
                continue;
            }

            return Ok(resp);
        }
    }
}

// ── SSE helpers ───────────────────────────────────────────────────────────────

fn parse_sse_line(line: &str) -> Option<StreamEvent> {
    let json_str = line.strip_prefix("data: ")?;
    match serde_json::from_str(json_str) {
        Ok(event) => Some(event),
        Err(e) => {
            tracing::debug!("SSE parse error: {e}");
            None
        }
    }
}

// ── Llm impl ──────────────────────────────────────────────────────────────────

impl Llm for ClaudeProvider {
    async fn send(&self, messages: &[Message]) -> Result<LlmResponse, anyhow::Error> {
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "tools": self.tools,
            "messages": messages,
        });

        let raw = self.send_with_retry(&body).await?;

        if !raw.status().is_success() {
            let status = raw.status();
            let text = raw.text().await?;
            anyhow::bail!("API error {status}: {text}");
        }

        let response: ApiResponse = raw.json().await?;
        parse_api_response(response)
    }
}

// ── StreamingLlm impl ─────────────────────────────────────────────────────────

impl StreamingLlm for ClaudeProvider {
    async fn send_streaming<F>(
        &self,
        messages: &[Message],
        on_text: F,
    ) -> Result<LlmResponse, anyhow::Error>
    where
        F: Fn(&str) + Send,
    {
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "tools": self.tools,
            "messages": messages,
            "stream": true,
        });

        // Streaming requests still benefit from retry on 429/529.
        // We retry the full request; the stream hasn't started yet when the
        // error status arrives.
        let raw = self.send_with_retry(&body).await?;

        if !raw.status().is_success() {
            let status = raw.status();
            let text = raw.text().await?;
            anyhow::bail!("API error {status}: {text}");
        }

        let mut stream = raw.bytes_stream();
        let mut line_buf = String::new();
        let mut blocks: Vec<Option<BlockState>> = vec![];
        let mut stop_reason = String::from("end_turn");
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = std::str::from_utf8(&chunk).unwrap_or("");
            line_buf.push_str(text);

            while let Some(pos) = line_buf.find('\n') {
                let line = line_buf[..pos].trim_end_matches('\r').to_owned();
                line_buf = line_buf[pos + 1..].to_owned();

                if let Some(event) = parse_sse_line(&line) {
                    match event {
                        StreamEvent::MessageStart { message } => {
                            input_tokens = message.usage.input_tokens;
                        }
                        StreamEvent::ContentBlockStart { index, content_block } => {
                            blocks.resize_with(index + 1, || None);
                            blocks[index] = Some(BlockState {
                                kind: match content_block {
                                    ContentBlock::Text { .. } => {
                                        BlockKind::Text { accumulated: String::new() }
                                    }
                                    ContentBlock::ToolUse { id, name } => BlockKind::ToolUse {
                                        id,
                                        name,
                                        json_buf: String::new(),
                                    },
                                },
                            });
                        }
                        StreamEvent::ContentBlockDelta { index, delta } => {
                            if let Some(Some(block)) = blocks.get_mut(index) {
                                match (&mut block.kind, delta) {
                                    (BlockKind::Text { accumulated }, Delta::TextDelta { text }) => {
                                        on_text(&text);
                                        accumulated.push_str(&text);
                                    }
                                    (
                                        BlockKind::ToolUse { json_buf, .. },
                                        Delta::InputJsonDelta { partial_json },
                                    ) => {
                                        json_buf.push_str(&partial_json);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        StreamEvent::MessageDelta { delta, usage } => {
                            stop_reason = delta.stop_reason;
                            output_tokens = usage.output_tokens;
                        }
                        StreamEvent::ContentBlockStop { .. }
                        | StreamEvent::MessageStop
                        | StreamEvent::Ping
                        | StreamEvent::Unknown => {}
                    }
                }
            }
        }

        // Collect accumulated blocks into the final response.
        let mut text_parts: Vec<String> = vec![];
        let mut tool_calls: Vec<ToolUse> = vec![];

        for block_opt in blocks {
            if let Some(block) = block_opt {
                match block.kind {
                    BlockKind::Text { accumulated } if !accumulated.is_empty() => {
                        text_parts.push(accumulated);
                    }
                    BlockKind::ToolUse { id, name, json_buf } => {
                        let input =
                            serde_json::from_str(&json_buf).unwrap_or(serde_json::Value::Null);
                        tool_calls.push(ToolUse { id, name, input });
                    }
                    _ => {}
                }
            }
        }

        Ok(LlmResponse {
            stop_reason,
            text: if text_parts.is_empty() { None } else { Some(text_parts.join("")) },
            tool_calls,
            usage: Usage { input_tokens, output_tokens },
        })
    }
}

// ── Conversion helpers ────────────────────────────────────────────────────────

fn parse_api_response(api: ApiResponse) -> Result<LlmResponse, anyhow::Error> {
    let mut text_parts: Vec<String> = vec![];
    let mut tool_calls: Vec<ToolUse> = vec![];

    for block in api.content {
        match block {
            ApiContent::Text { text } => text_parts.push(text),
            ApiContent::ToolUse { id, name, input } => {
                tool_calls.push(ToolUse { id, name, input });
            }
            ApiContent::Unknown => {}
        }
    }

    Ok(LlmResponse {
        stop_reason: api.stop_reason,
        text: if text_parts.is_empty() { None } else { Some(text_parts.join("")) },
        tool_calls,
        usage: Usage {
            input_tokens: api.usage.input_tokens,
            output_tokens: api.usage.output_tokens,
        },
    })
}
