use std::sync::Mutex;

use anyhow::anyhow;

/// Session 01 · Exercise 1 & 3
/// 实现 Agent Loop 的骨架。
///
/// 这个文件是课程的核心。先通读注释，再动手填 TODO。
use crate::{
    context::ContextManager,
    types::{Message, ToolResult, ToolUse},
};

// ── 错误类型 ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("reached max turns ({0}) without end_turn")]
    MaxTurnsExceeded(usize),
    // 后续章节会在这里添加更多错误类型
}

// ── LLM 响应（简化版）────────────────────────────────────────────────────────

/// 本轮 API 调用消耗的 token 数（来自响应的 usage 字段）
#[derive(Debug, Default, Clone, Copy)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl Usage {
    pub fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

/// Claude API 返回的响应（简化，只保留本节需要的字段）
#[derive(Debug)]
pub struct LlmResponse {
    /// "end_turn" | "tool_use" | "max_tokens"
    pub stop_reason: String,
    pub text: Option<String>,
    pub tool_calls: Vec<ToolUse>,
    pub usage: Usage,
}

// ── Trait：可替换的 LLM 后端 ──────────────────────────────────────────────────

/// 抽象出"发消息给 LLM"这个动作，方便测试时用 Mock 替换。
///
/// Session 01 不需要真正调用 Claude API，用 `MockLlm` 就够了。
#[allow(async_fn_in_trait)]
pub trait Llm {
    async fn send(&self, messages: &[Message]) -> Result<LlmResponse, anyhow::Error>;
}

// ── Trait：可替换的工具执行器 ──────────────────────────────────────────────────

#[allow(async_fn_in_trait)]
pub trait ToolExecutor {
    async fn execute(&self, call: &ToolUse) -> Result<ToolResult, anyhow::Error>;
}

// ── AgentLoop ─────────────────────────────────────────────────────────────────

pub struct AgentLoop<L: Llm, E: ToolExecutor> {
    llm: L,
    executor: E,
    max_turns: usize,
    context: ContextManager,
}

impl<L: Llm, E: ToolExecutor> AgentLoop<L, E> {
    pub fn new(llm: L, executor: E) -> Self {
        Self {
            llm,
            executor,
            max_turns: 25,
            context: ContextManager::default_config(),
        }
    }

    /// 运行 Agent Loop，返回最终的文本回答。
    pub async fn run(&mut self, user_input: &str) -> Result<String, anyhow::Error> {
        let mut messages: Vec<Message> = vec![Message::user(user_input)];
        self.run_turn(&mut messages).await
    }

    /// 多轮对话：在已有消息列表上运行一个用户回合。
    ///
    /// 调用前：调用方需已把用户消息 push 进 messages。
    /// 返回后：messages 包含完整的对话历史（含本轮 assistant 回复）。
    /// 这样下一次调用时，Claude 能看到完整上下文。
    pub async fn run_turn(&mut self, messages: &mut Vec<Message>) -> Result<String, anyhow::Error> {
        let mut turn = 0;
        let mut last_input_tokens: u32 = 0;
        loop {
            if turn >= self.max_turns {
                return Err(LoopError::MaxTurnsExceeded(turn).into());
            }
            turn += 1;

            if self.context.should_truncate(last_input_tokens) {
                self.context.truncate(messages);
                tracing::warn!("Context truncate");
            }

            let response = self.llm.send(messages).await?;
            last_input_tokens = response.usage.input_tokens;

            if response.stop_reason == "end_turn" {
                let text = response.text.unwrap_or_default();
                // 把 assistant 回复加入历史，保证多轮对话上下文连续
                messages.push(Message::assistant_text(&text));
                return Ok(text);
            }

            if response.stop_reason == "tool_use" {
                // 所有 tool_use 块合并进一条 assistant 消息（Claude API 要求）
                messages.push(Message::tool_uses(response.tool_calls.clone()));

                let mut tool_results = vec![];
                for call in &response.tool_calls {
                    let result = self.executor.execute(call).await?;
                    tool_results.push(result);
                }
                messages.push(Message::tool_results(tool_results));
                continue;
            }
        }
    }
}

// ── Mock（用于本节测试）────────────────────────────────────────────────────────

/// 一个假的 LLM，按预设脚本返回响应。
/// 这样你不需要 API Key 就能测试循环逻辑。
pub struct MockLlm {
    /// 每次调用 send() 弹出队列头部的响应
    responses: Mutex<std::collections::VecDeque<LlmResponse>>,
}

impl MockLlm {
    pub fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
        }
    }
}

impl Llm for MockLlm {
    async fn send(&self, _messages: &[Message]) -> Result<LlmResponse, anyhow::Error> {
        let mut responses = self.responses.lock().unwrap();
        if let Some(response) = responses.pop_front() {
            return Ok(response);
        }
        Err(anyhow!("No reponse left"))
    }
}

// ── Session 08：StreamingLlm trait ────────────────────────────────────────────

/// 流式 LLM 扩展 trait —— 实现者可将文本逐字推送给回调。
///
/// 使用 supertrait 约束：实现 StreamingLlm 的类型同时也必须实现 Llm。
/// 这样 AgentLoop 可以在有流式能力时用流式，没有时退回 Llm::send()。
#[allow(async_fn_in_trait)]
pub trait StreamingLlm: Llm {
    /// 流式发送消息：每生成一个文本 token 就调用一次 on_text。
    ///
    /// 返回值与 Llm::send() 完全兼容——调用方不需要区分流式/非流式结果。
    async fn send_streaming<F>(
        &self,
        messages: &[Message],
        on_text: F,
    ) -> Result<LlmResponse, anyhow::Error>
    where
        F: Fn(&str) + Send;
}

impl<L: Llm + StreamingLlm, E: ToolExecutor> AgentLoop<L, E> {
    /// 流式版本的 run_turn：
    ///   - 第一次 LLM 调用用流式（on_text 逐字打印）
    ///   - 工具调用后的后续轮次用普通 send()（工具结果通常不需要流式）
    ///
    /// Session 08 · Exercise：CLI 里调用这个方法，感受流式输出的效果。
    pub async fn run_turn_streaming<F>(
        &mut self,
        messages: &mut Vec<Message>,
        on_text: F,
    ) -> Result<String, anyhow::Error>
    where
        F: Fn(&str) + Send + Clone,
    {
        let mut turn = 0;
        let mut last_input_tokens: u32 = 0;
        let mut first_call = true;

        loop {
            if turn >= self.max_turns {
                return Err(LoopError::MaxTurnsExceeded(turn).into());
            }
            turn += 1;

            if self.context.should_truncate(last_input_tokens) {
                self.context.truncate(messages);
                tracing::warn!("Context truncate");
            }

            // 第一次调用使用流式；工具回调后续轮次使用普通 send()
            let response = if first_call {
                first_call = false;
                self.llm.send_streaming(messages, on_text.clone()).await?
            } else {
                self.llm.send(messages).await?
            };
            last_input_tokens = response.usage.input_tokens;

            if response.stop_reason == "end_turn" {
                let text = response.text.unwrap_or_default();
                messages.push(Message::assistant_text(&text));
                return Ok(text);
            }

            if response.stop_reason == "tool_use" {
                messages.push(Message::tool_uses(response.tool_calls.clone()));
                let mut tool_results = vec![];
                for call in &response.tool_calls {
                    let result = self.executor.execute(call).await?;
                    tool_results.push(result);
                }
                messages.push(Message::tool_results(tool_results));
                continue;
            }
        }
    }
}

/// 一个假的执行器，直接返回固定字符串。
pub struct EchoExecutor;

impl ToolExecutor for EchoExecutor {
    async fn execute(&self, call: &ToolUse) -> Result<ToolResult, anyhow::Error> {
        Ok(ToolResult {
            tool_use_id: call.id.clone(),
            content: format!("[mock] executed: {}", call.name),
        })
    }
}
