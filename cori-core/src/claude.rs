/// Session 03 · Real API Call
///
/// 用真实的 HTTP 请求替换 MockLlm。
/// 实现后，Cori 就能真正和 Claude 对话了。
use crate::{
    loop_::{Llm, LlmResponse},
    types::{Message, ToolUse},
};

// ── ClaudeLlm ─────────────────────────────────────────────────────────────────

pub struct ClaudeLlm {
    api_key: String,
    model: String,
    /// 已注册工具的 schema 列表，来自 ToolRegistry::all_schemas()
    tools: Vec<serde_json::Value>,
    client: reqwest::Client,
    /// Messages API 端点
    /// 默认 https://api.anthropic.com/v1/messages
    /// 通过 ANTHROPIC_BASE_URL 覆盖，可兼容第三方 Anthropic 兼容服务（如 MiniMax）
    url: String,
}

impl ClaudeLlm {
    /// 从环境变量读取配置，构造 ClaudeLlm。
    ///
    /// 必填：ANTHROPIC_API_KEY
    /// 可选：ANTHROPIC_BASE_URL（默认 https://api.anthropic.com）
    /// 可选：ANTHROPIC_MODEL（默认 claude-opus-4-6）
    pub fn from_env(tools: Vec<serde_json::Value>) -> Result<Self, anyhow::Error> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")?;
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".into());
        let model = std::env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-opus-4-6".into());
        Ok(Self {
            api_key,
            model,
            tools,
            client: reqwest::Client::new(),
            url: format!("{base_url}/v1/messages"),
        })
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

// ── API 请求 / 响应结构（内部用）────────────────────────────────────────────────

/// Claude API 返回的原始响应（只保留需要的字段）
///
/// 文档：https://docs.anthropic.com/en/api/messages
#[derive(serde::Deserialize, Debug)]
struct ApiResponse {
    stop_reason: String,
    content: Vec<ApiContent>,
    // usage 字段暂时忽略，Session 04（Context Management）会用到
}

/// content 数组里的每个元素
#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApiContent {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

// ── Llm trait 实现 ────────────────────────────────────────────────────────────

impl Llm for ClaudeLlm {
    async fn send(&self, messages: &[Message]) -> Result<LlmResponse, anyhow::Error> {
        // Exercise 2：构造请求 body
        //
        // Claude API 的请求格式：
        // {
        //   "model": "...",
        //   "max_tokens": 4096,
        //   "tools": [...],        ← 来自 self.tools
        //   "messages": [...]      ← 直接序列化 messages（我们的 Message 类型已对齐 API 格式）
        // }
        //
        // 思考：max_tokens 设多少合适？太小会发生什么？
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "tools": self.tools,
            "messages": messages
        });

        // Exercise 3：发送 HTTP 请求
        //
        // Endpoint：https://api.anthropic.com/v1/messages
        // Headers：
        //   x-api-key: {self.api_key}
        //   anthropic-version: 2023-06-01      ← 固定值，API 版本号
        //   content-type: application/json
        //
        // 提示：self.client.post(url).header(...).json(&body).send().await?
        let raw_reponse = self
            .client
            .post(&self.url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !raw_reponse.status().is_success() {
            let status = raw_reponse.status();
            let text = raw_reponse.text().await?;
            anyhow::bail!("API error {status}: {text}");
        }

        let response: ApiResponse = raw_reponse.json().await?;

        // Exercise 4：把 ApiResponse 转换成 LlmResponse
        //
        // 需要从 response.content 里：
        //   - 收集所有 Text 块的文本（拼接）
        //   - 收集所有 ToolUse 块，转成 crate::types::ToolUse
        parse_response(response)
    }
}

/// 把原始 API 响应转换成 AgentLoop 使用的 LlmResponse。
///
/// Exercise 4：补全这个函数。
fn parse_response(api: ApiResponse) -> Result<LlmResponse, anyhow::Error> {
    let mut text_parts: Vec<String> = vec![];
    let mut tool_calls: Vec<ToolUse> = vec![];

    for block in api.content {
        match block {
            ApiContent::Text { text } => {
                // 追加到 text_parts
                text_parts.push(text);
            }
            ApiContent::ToolUse { id, name, input } => {
                // 构造 ToolUse，追加到 tool_calls
                tool_calls.push(ToolUse { id, name, input });
            }
        }
    }

    Ok(LlmResponse {
        stop_reason: api.stop_reason,
        text: if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join(""))
        },
        tool_calls,
    })
}
