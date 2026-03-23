/// Session 01 · Exercise 1 & 3
/// 实现 Agent Loop 的骨架。
///
/// 这个文件是课程的核心。先通读注释，再动手填 TODO。

use crate::types::{Message, ToolResult, ToolUse};

// ── 错误类型 ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("reached max turns ({0}) without end_turn")]
    MaxTurnsExceeded(usize),
    // 后续章节会在这里添加更多错误类型
}

// ── LLM 响应（简化版）────────────────────────────────────────────────────────

/// Claude API 返回的响应（简化，只保留本节需要的字段）
#[derive(Debug)]
pub struct LlmResponse {
    /// "end_turn" | "tool_use" | "max_tokens"
    pub stop_reason: String,
    pub text: Option<String>,
    pub tool_calls: Vec<ToolUse>,
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

/// Exercise 3：给 AgentLoop 加上安全阀
pub struct AgentLoop<L: Llm, E: ToolExecutor> {
    llm: L,
    executor: E,
    // TODO: 加入 max_turns: usize
}

impl<L: Llm, E: ToolExecutor> AgentLoop<L, E> {
    pub fn new(llm: L, executor: E) -> Self {
        Self {
            llm,
            executor,
            // TODO: 设置默认 max_turns（建议 25，思考：为什么不是无限？）
        }
    }

    /// 运行 Agent Loop，返回最终的文本回答。
    ///
    /// Exercise 1：补全循环逻辑，让它能正确退出。
    /// Exercise 3：加入 max_turns 检查。
    pub async fn run(&mut self, user_input: &str) -> Result<String, anyhow::Error> {
        let mut messages: Vec<Message> = vec![
            // TODO: 用 Message::user() 构造初始消息
        ];

        // TODO: 实现循环
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

        todo!("实现 Agent Loop")
    }
}

// ── Mock（用于本节测试）────────────────────────────────────────────────────────

/// 一个假的 LLM，按预设脚本返回响应。
/// 这样你不需要 API Key 就能测试循环逻辑。
pub struct MockLlm {
    /// 每次调用 send() 弹出队列头部的响应
    responses: std::collections::VecDeque<LlmResponse>,
}

impl MockLlm {
    pub fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses: responses.into(),
        }
    }
}

impl Llm for MockLlm {
    async fn send(&self, _messages: &[Message]) -> Result<LlmResponse, anyhow::Error> {
        // TODO: 从 self.responses 取出下一个响应
        // 提示：需要把 &self 改成 &mut self，或者用 RefCell/Mutex，你来决定取舍
        todo!("弹出并返回下一个预设响应")
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
