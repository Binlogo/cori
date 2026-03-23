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
    ///
    /// Exercise 1：补全循环逻辑，让它能正确退出。
    /// Exercise 3：加入 max_turns 检查。
    pub async fn run(&mut self, user_input: &str) -> Result<String, anyhow::Error> {
        let mut messages: Vec<Message> = vec![Message::user(user_input)];

        // 实现循环
        //
        // 每轮：
        //   1. 调用 self.llm.send(&messages)
        //   2. 检查 stop_reason
        //      - "end_turn"  → 返回 response.text
        //      - "tool_use"  → 执行所有 tool_calls，收集 ToolResult
        //                      把结果追加到 messages（用 Message::tool_results()）
        //                      继续下一轮
        //      - 其他        → 返回错误或当作 end_turn 处理
        //   3. 检查是否超过 max_turns

        let mut turn = 0;
        let mut last_input_tokens: u32 = 0;
        loop {
            if turn >= self.max_turns {
                return Err(LoopError::MaxTurnsExceeded(turn).into());
            }
            turn += 1;

            // Exercise 2：在发送前检查是否需要截断
            // 用 self.context.should_truncate(last_input_tokens) 判断，
            //       为真时调用 self.context.truncate(&mut messages)
            //       截断后打印一条 tracing::warn! 告知用户上下文被压缩了

            if self.context.should_truncate(last_input_tokens) {
                self.context.truncate(&mut messages);
                tracing::warn!("Context truncate");
            }

            let response = self.llm.send(&messages).await?;
            last_input_tokens = response.usage.input_tokens;

            if response.stop_reason == "end_turn" {
                return Ok(response.text.unwrap_or_default());
            }

            if response.stop_reason == "tool_use" {
                // 所有 tool_use 块合并进一条 assistant 消息（Claude API 要求）
                // 如果拆成多条，真实 API 会报错：tool_use 和 tool_result 必须成对出现在相邻消息里
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
