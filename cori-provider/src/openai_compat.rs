//! OpenAI-compatible provider implementation.
//!
//! `OpenAiCompatProvider` targets any endpoint that speaks the OpenAI Chat
//! Completions API (`POST /v1/chat/completions`). Configuration is carried by
//! a [`ProviderConfig`]: `api_key`, `base_url`, `model`, and `max_tokens` are
//! all used; `timeout_secs` controls the HTTP client timeout.
//!
//! ## Message format conversion
//!
//! Cori's internal `Message` / `Content` types are mapped to OpenAI's format:
//!
//! | Cori content                        | OpenAI message                                      |
//! |-------------------------------------|-----------------------------------------------------|
//! | `Role::User` + `Content::Text`      | `{"role":"user","content":"..."}`                   |
//! | `Role::Assistant` + `Content::Text` | `{"role":"assistant","content":"..."}`              |
//! | `Role::Assistant` + `Content::ToolUse` | `{"role":"assistant","tool_calls":[...]}`       |
//! | `Role::User` + `Content::ToolResult` | `{"role":"tool","tool_call_id":"...","content":"..."}` |
//!
//! ## Tool / function calling
//!
//! Pass tool schemas in OpenAI's `tools` format to the constructor.  The
//! provider forwards them as-is in the `tools` field of the request body.

use std::time::Duration;

use cori_core::{
    config::ProviderConfig,
    loop_::{Llm, LlmResponse, Usage},
    types::{Content, Message, Role, ToolUse},
};

// ── OpenAI request / response types (internal) ────────────────────────────────

/// A single message in OpenAI format (request side).
#[derive(serde::Serialize, Debug)]
#[serde(untagged)]
enum OaiMessage {
    /// user or system text
    Text {
        role: &'static str,
        content: String,
    },
    /// assistant turn with tool calls
    AssistantWithToolCalls {
        role: &'static str,
        content: Option<String>,
        tool_calls: Vec<OaiToolCall>,
    },
    /// tool result
    ToolResult {
        role: &'static str,
        tool_call_id: String,
        content: String,
    },
}

#[derive(serde::Serialize, Debug)]
struct OaiToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str, // always "function"
    function: OaiFunction,
}

#[derive(serde::Serialize, Debug)]
struct OaiFunction {
    name: String,
    arguments: String, // JSON-encoded string
}

/// Subset of the OpenAI Chat Completion response we care about.
#[derive(serde::Deserialize, Debug)]
struct OaiResponse {
    choices: Vec<OaiChoice>,
    usage: OaiUsage,
}

#[derive(serde::Deserialize, Debug)]
struct OaiChoice {
    message: OaiAssistantMessage,
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
struct OaiAssistantMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OaiResponseToolCall>>,
}

#[derive(serde::Deserialize, Debug)]
struct OaiResponseToolCall {
    id: String,
    function: OaiResponseFunction,
}

#[derive(serde::Deserialize, Debug)]
struct OaiResponseFunction {
    name: String,
    arguments: String,
}

#[derive(serde::Deserialize, Debug)]
struct OaiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

// ── OpenAiCompatProvider ──────────────────────────────────────────────────────

/// An LLM provider that targets OpenAI-compatible `/v1/chat/completions`
/// endpoints (non-streaming).
///
/// Construct via [`OpenAiCompatProvider::new()`] or [`OpenAiCompatProvider::from_env()`].
/// Add tool schemas with [`OpenAiCompatProvider::with_tools()`].
#[derive(Debug)]
pub struct OpenAiCompatProvider {
    config: ProviderConfig,
    tools: Vec<serde_json::Value>,
    client: reqwest::Client,
}

impl OpenAiCompatProvider {
    /// Build a provider from the given configuration.
    pub fn new(config: ProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();
        Self { config, tools: vec![], client }
    }

    /// Build from environment variables (`ANTHROPIC_API_KEY` / `OPENAI_API_KEY`,
    /// `ANTHROPIC_BASE_URL`, `ANTHROPIC_MODEL`).
    ///
    /// Falls back to `OPENAI_API_KEY` when `ANTHROPIC_API_KEY` is not set.
    pub fn from_env() -> Result<Self, anyhow::Error> {
        // Try standard Anthropic env first, then OpenAI-style.
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .map_err(|_| anyhow::anyhow!("Neither ANTHROPIC_API_KEY nor OPENAI_API_KEY is set"))?;
        let base_url = std::env::var("OPENAI_BASE_URL")
            .or_else(|_| std::env::var("ANTHROPIC_BASE_URL"))
            .unwrap_or_else(|_| "https://api.openai.com".into());
        let model = std::env::var("OPENAI_MODEL")
            .or_else(|_| std::env::var("ANTHROPIC_MODEL"))
            .unwrap_or_else(|_| "gpt-4o".into());

        let config = ProviderConfig {
            api_key,
            base_url,
            model,
            ..ProviderConfig::default()
        };
        Ok(Self::new(config))
    }

    /// Set tool schemas in OpenAI format (builder-style).
    pub fn with_tools(mut self, tools: Vec<serde_json::Value>) -> Self {
        self.tools = tools;
        self
    }

    fn completions_url(&self) -> String {
        format!("{}/v1/chat/completions", self.config.base_url)
    }
}

// ── Message conversion ────────────────────────────────────────────────────────

/// Convert a slice of Cori `Message`s into the OpenAI messages array.
///
/// The conversion rules are described in the module-level doc.
fn convert_messages(messages: &[Message]) -> Vec<OaiMessage> {
    let mut out: Vec<OaiMessage> = Vec::with_capacity(messages.len());

    for msg in messages {
        match msg.role {
            Role::User => {
                // A user message may contain Text blocks and/or ToolResult blocks.
                // Text blocks become a single user message (content joined).
                // ToolResult blocks each become a separate "tool" role message.
                let mut text_parts: Vec<String> = vec![];
                let mut tool_results: Vec<(String, String)> = vec![]; // (tool_call_id, content)

                for content in &msg.content {
                    match content {
                        Content::Text { text } => text_parts.push(text.clone()),
                        Content::ToolResult(tr) => {
                            tool_results.push((tr.tool_use_id.clone(), tr.content.clone()));
                        }
                        Content::ToolUse(_) => {
                            // Unexpected in user role — skip.
                        }
                    }
                }

                if !text_parts.is_empty() {
                    out.push(OaiMessage::Text {
                        role: "user",
                        content: text_parts.join("\n"),
                    });
                }

                for (tool_call_id, content) in tool_results {
                    out.push(OaiMessage::ToolResult {
                        role: "tool",
                        tool_call_id,
                        content,
                    });
                }
            }
            Role::Assistant => {
                // An assistant message may contain Text blocks and/or ToolUse blocks.
                let mut text_parts: Vec<String> = vec![];
                let mut tool_calls: Vec<OaiToolCall> = vec![];

                for content in &msg.content {
                    match content {
                        Content::Text { text } => text_parts.push(text.clone()),
                        Content::ToolUse(tu) => {
                            let arguments = serde_json::to_string(&tu.input)
                                .unwrap_or_else(|_| "{}".to_string());
                            tool_calls.push(OaiToolCall {
                                id: tu.id.clone(),
                                kind: "function",
                                function: OaiFunction {
                                    name: tu.name.clone(),
                                    arguments,
                                },
                            });
                        }
                        Content::ToolResult(_) => {
                            // Unexpected in assistant role — skip.
                        }
                    }
                }

                if tool_calls.is_empty() {
                    // Pure text assistant message.
                    out.push(OaiMessage::Text {
                        role: "assistant",
                        content: text_parts.join("\n"),
                    });
                } else {
                    // Assistant message with tool calls (text is optional).
                    let content = if text_parts.is_empty() {
                        None
                    } else {
                        Some(text_parts.join("\n"))
                    };
                    out.push(OaiMessage::AssistantWithToolCalls {
                        role: "assistant",
                        content,
                        tool_calls,
                    });
                }
            }
        }
    }

    out
}

/// Map an OpenAI `finish_reason` to Cori's `stop_reason` convention.
fn map_finish_reason(finish_reason: Option<String>) -> String {
    match finish_reason.as_deref() {
        Some("stop") => "end_turn".to_string(),
        Some("tool_calls") => "tool_use".to_string(),
        Some("length") => "max_tokens".to_string(),
        Some(other) => other.to_string(),
        None => "end_turn".to_string(),
    }
}

// ── Llm impl ──────────────────────────────────────────────────────────────────

impl Llm for OpenAiCompatProvider {
    async fn send(&self, messages: &[Message]) -> Result<LlmResponse, anyhow::Error> {
        let oai_messages = convert_messages(messages);

        let mut body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "messages": oai_messages,
            "stream": false,
        });

        if !self.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(self.tools.clone());
        }

        let raw = self
            .client
            .post(self.completions_url())
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
            .await?;

        if !raw.status().is_success() {
            let status = raw.status();
            let text = raw.text().await?;
            anyhow::bail!("API error {status}: {text}");
        }

        let response: OaiResponse = raw.json().await?;

        // Take the first choice (there is almost always exactly one).
        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("OpenAI response had no choices"))?;

        let stop_reason = map_finish_reason(choice.finish_reason);

        let text = choice.message.content.filter(|s| !s.is_empty());

        let tool_calls: Vec<ToolUse> = choice
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| {
                let input: serde_json::Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null);
                ToolUse {
                    id: tc.id,
                    name: tc.function.name,
                    input,
                }
            })
            .collect();

        Ok(LlmResponse {
            stop_reason,
            text,
            tool_calls,
            usage: Usage {
                input_tokens: response.usage.prompt_tokens,
                output_tokens: response.usage.completion_tokens,
            },
        })
    }
}
