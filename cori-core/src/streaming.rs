/// Session 08 · Streaming
///
/// Claude API 的流式输出协议：SSE（Server-Sent Events）
///
/// 为什么 Streaming 很重要？
///   LLM 本质上是逐 token 生成的。非流式 API 要等全部生成完毕再返回，
///   用户面对空白屏幕等待数秒；流式 API 每生成一个 token 就立刻推送。
///   Claude Code 的"边思考边打字"体验完全依赖于此。
///
/// SSE 协议格式（每个事件由空行分隔）：
///
///   event: message_start
///   data: {"type":"message_start","message":{"usage":{"input_tokens":25,...}}}
///
///   event: content_block_start
///   data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}
///
///   event: content_block_delta
///   data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}
///
///   event: content_block_delta
///   data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":", world"}}
///
///   event: content_block_stop
///   data: {"type":"content_block_stop","index":0}
///
///   event: message_delta
///   data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":10}}
///
///   event: message_stop
///   data: {"type":"message_stop"}
///
/// 工具调用的特殊之处：input JSON 也是流式的，以 partial_json delta 形式发来，
/// 需要在客户端拼接成完整 JSON 再解析。
use futures_util::StreamExt;

use crate::{
    claude::ClaudeLlm,
    loop_::{LlmResponse, StreamingLlm, Usage},
    types::{Message, ToolUse},
};

// ── SSE 事件类型定义 ───────────────────────────────────────────────────────────
//
// 用 serde 的 tag dispatch 把 JSON 的 "type" 字段映射到 Rust enum variant。
// #[serde(other)] 表示"遇到未知类型就用这个 variant"——兼容未来 API 新增的事件。

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum StreamEvent {
    MessageStart {
        message: MessageStartData,
    },
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: Delta,
    },
    ContentBlockStop {
        #[allow(dead_code)]
        index: usize,
    },
    MessageDelta {
        delta: MessageDeltaData,
        usage: DeltaUsage,
    },
    MessageStop,
    Ping,
    #[serde(other)]
    Unknown,
}

#[derive(serde::Deserialize, Debug)]
pub(crate) struct MessageStartData {
    pub usage: StartUsage,
}

#[derive(serde::Deserialize, Debug)]
pub(crate) struct StartUsage {
    pub input_tokens: u32,
}

/// content_block_start 里的 block 类型
#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub(crate) enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String },
}

/// content_block_delta 里的 delta 类型
#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Delta {
    /// 文本 token delta：text 字段是新生成的内容片段
    TextDelta { text: String },
    /// 工具输入 JSON delta：partial_json 需要拼接成完整 JSON
    InputJsonDelta { partial_json: String },
}

#[derive(serde::Deserialize, Debug)]
pub(crate) struct MessageDeltaData {
    pub stop_reason: String,
}

#[derive(serde::Deserialize, Debug)]
pub(crate) struct DeltaUsage {
    pub output_tokens: u32,
}

// ── 内部状态机 ─────────────────────────────────────────────────────────────────
//
// 每个 content block 有一个 BlockState，追踪它的类型和已累积的内容。
// index 对应 content_block_start / content_block_delta / content_block_stop 里的 index。

struct BlockState {
    kind: BlockKind,
}

enum BlockKind {
    Text {
        accumulated: String,
    },
    ToolUse {
        id: String,
        name: String,
        json_buf: String,
    },
}

// ── Exercise 1：parse_sse_line ─────────────────────────────────────────────────

/// 解析 SSE 数据行，返回反序列化后的事件。
///
/// SSE 格式说明：
///   - 每个事件由多行组成，以空行结束
///   - `event: xxx` 行：事件名（我们忽略，因为 JSON 里有 "type" 字段）
///   - `data: {...}` 行：这才是我们要解析的 JSON 内容
///   - 注释行以 `:` 开头（如 `: ping`），可忽略
///
/// 步骤：
///   1. 如果行以 "data: " 开头（注意空格），取出后面的 JSON 字符串
///   2. 用 serde_json::from_str::<StreamEvent>(&json_str) 反序列化
///   3. 如果反序列化失败，记录 debug 日志（tracing::debug!）后返回 None
///   4. 其他行（event:, :, 空行）都返回 None
///
/// 提示：
///   line.strip_prefix("data: ")  →  Option<&str>，成功时得到 JSON 字符串
pub(crate) fn parse_sse_line(line: &str) -> Option<StreamEvent> {
    // Exercise 1 — 实现 SSE 行解析
    let json_str = line.strip_prefix("data: ")?;
    match serde_json::from_str(json_str) {
        Ok(event) => Some(event),
        Err(e) => {
            tracing::debug!("SSE parse error: {e}");
            None
        }
    }
}

// ── Exercise 2：send_streaming ─────────────────────────────────────────────────

impl StreamingLlm for ClaudeLlm {
    /// 流式发送消息，每收到一个文本 token 就调用 on_text。
    ///
    /// 实现步骤：
    ///
    ///   **Step A**（已提供）：构造请求 body，加上 "stream": true
    ///
    ///   **Step B**（已提供）：发送请求，获取 bytes_stream()
    ///
    ///   **Step C**（Exercise 2）：逐块解析 SSE，处理事件
    ///     - 把每个 Bytes chunk 转成 UTF-8 字符串，按行分割
    ///     - 调用 parse_sse_line() 解析事件
    ///     - 根据事件类型更新状态
    ///
    /// 关键事件处理逻辑（Exercise 2 中实现）：
    ///   MessageStart         → 记录 input_tokens
    ///   ContentBlockStart    → 在 blocks[index] 初始化 BlockState
    ///   ContentBlockDelta
    ///     TextDelta          → 追加到 accumulated；调用 on_text(&delta.text)
    ///     InputJsonDelta     → 追加到 json_buf（拼接 tool input JSON）
    ///   MessageDelta         → 记录 stop_reason 和 output_tokens
    ///   其他                 → 忽略
    async fn send_streaming<F>(
        &self,
        messages: &[Message],
        on_text: F,
    ) -> Result<LlmResponse, anyhow::Error>
    where
        F: Fn(&str) + Send,
    {
        // Step A：构造 body（与 ClaudeLlm::send() 相同，但加 "stream": true）
        let body = serde_json::json!({
            "model": self.model(),
            "max_tokens": 4096,
            "tools": self.tools(),
            "messages": messages,
            "stream": true          // ← 关键：告诉 API 用 SSE 流式返回
        });

        // Step B：发送请求，获取字节流
        //
        // 注意：流式响应的 Content-Type 是 text/event-stream，
        //       不能用 .json() 一次性反序列化，必须用 .bytes_stream() 逐块读取。
        let raw = self
            .http_client()
            .post(self.url())
            .header("x-api-key", self.api_key())
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !raw.status().is_success() {
            let status = raw.status();
            let text = raw.text().await?;
            anyhow::bail!("API error {status}: {text}");
        }

        let mut stream = raw.bytes_stream();

        // Step C：处理 SSE 流（Exercise 2）
        //
        // SSE 流的每个 chunk 是若干字节，可能跨越多行，也可能不完整。
        // 我们用 line_buf 缓冲已收到但未处理完的文本。
        let mut line_buf = String::new();
        let mut blocks: Vec<Option<BlockState>> = vec![];
        let mut stop_reason = String::from("end_turn");
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            // chunk 是 Bytes，转成 &str（忽略非 UTF-8 字节）
            let text = std::str::from_utf8(&chunk).unwrap_or("");
            line_buf.push_str(text);

            // 每次处理 line_buf 中所有完整的行（以 \n 结尾）
            while let Some(pos) = line_buf.find('\n') {
                let line = line_buf[..pos].trim_end_matches('\r').to_owned();
                line_buf = line_buf[pos + 1..].to_owned();

                // Exercise 2 — 调用 parse_sse_line(&line)，match 返回的事件
                if let Some(event) = parse_sse_line(&line) {
                    match event {
                        StreamEvent::MessageStart { message } => {
                            input_tokens = message.usage.input_tokens;
                        }
                        StreamEvent::ContentBlockStart {
                            index,
                            content_block,
                        } => {
                            // 确保 blocks 足够长
                            blocks.resize_with(index + 1, || None);
                            blocks[index] = Some(BlockState {
                                kind: match content_block {
                                    ContentBlock::Text { .. } => BlockKind::Text {
                                        accumulated: String::new(),
                                    },
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
                                    (
                                        BlockKind::Text { accumulated },
                                        Delta::TextDelta { text },
                                    ) => {
                                        on_text(&text); // ← 流式打印！
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
                        _ => {}
                    }
                }
            }
        }

        // 从 blocks 收集最终结果（完整的文本 + 工具调用）
        let mut text_parts: Vec<String> = vec![];
        let mut tool_calls: Vec<ToolUse> = vec![];

        for block_opt in blocks {
            if let Some(block) = block_opt {
                match block.kind {
                    BlockKind::Text { accumulated } => {
                        if !accumulated.is_empty() {
                            text_parts.push(accumulated);
                        }
                    }
                    BlockKind::ToolUse { id, name, json_buf } => {
                        let input =
                            serde_json::from_str(&json_buf).unwrap_or(serde_json::Value::Null);
                        tool_calls.push(ToolUse { id, name, input });
                    }
                }
            }
        }

        Ok(LlmResponse {
            stop_reason,
            text: if text_parts.is_empty() {
                None
            } else {
                Some(text_parts.join(""))
            },
            tool_calls,
            usage: Usage {
                input_tokens,
                output_tokens,
            },
        })
    }
}
