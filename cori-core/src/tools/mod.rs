/// Session 02 · Tool Dispatch
///
/// 上一节我们用了一个 `EchoExecutor` 假装在执行工具。
/// 这一节，我们把它替换成真正的 Tool 系统：
///   - 每个工具实现 `Tool` trait
///   - `ToolRegistry` 负责注册和分发
pub mod bash;
mod tests;

use std::collections::HashMap;

use crate::types::{ToolResult, ToolUse};

// ── Tool Trait ────────────────────────────────────────────────────────────────

/// 每个工具必须实现这个 trait。
///
/// Claude 通过 `schema()` 知道工具存在及其参数格式。
/// Agent Loop 通过 `execute()` 实际调用工具。
///
/// Exercise 1：理解这三个方法各自的职责，然后看 BashTool 的实现。
pub trait Tool: Send + Sync {
    /// 工具名称，必须与 Claude API 请求中的 `name` 字段完全一致。
    fn name(&self) -> &str;

    /// 执行工具，返回文本结果。
    ///
    /// `input` 是 Claude 传来的 JSON 参数，格式由 `schema()` 约定。
    fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error>;

    /// 生成发送给 Claude 的 JSON Schema 描述。
    ///
    /// Claude 靠这个"知道"有哪些工具、每个工具需要什么参数。
    /// 思考：如果 schema 写错了，会发生什么？
    fn schema(&self) -> serde_json::Value;
}

// ── ToolRegistry ──────────────────────────────────────────────────────────────

/// 工具注册表：name → Box<dyn Tool> 的 dispatch map。
///
/// Exercise 2：补全 `register` 和 `dispatch` 方法。
/// Exercise 3：实现 `all_schemas()`，把所有工具的 schema 收集起来。
#[derive(Default)]
pub struct ToolRegistry {
    // 选择合适的数据结构存储工具
    // 提示：需要通过名字快速查找
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个工具。
    pub fn register(&mut self, tool: impl Tool + 'static) {
        self.tools.insert(tool.name().to_owned(), Box::new(tool));
    }

    /// 根据名字查找并执行工具，返回 ToolResult。
    ///
    /// 思考：找不到工具时应该返回什么？
    ///   选项 A：Err(...) — Agent Loop 崩溃
    ///   选项 B：Ok(ToolResult { content: "unknown tool" }) — 告诉 Claude 工具不存在
    ///   Claude Code 选的是哪个？为什么？
    pub fn dispatch(&self, call: &ToolUse) -> Result<ToolResult, anyhow::Error> {
        let id = call.id.clone();
        let Some(tool) = self.tools.get(&call.name) else {
            return Ok(ToolResult {
                content: "unknown tool".into(),
                tool_use_id: id,
            });
        };

        let result = tool.execute(&call.input)?;

        Ok(ToolResult {
            content: result,
            tool_use_id: id,
        })
    }

    /// 返回所有已注册工具的 schema 列表，用于构造 API 请求的 `tools` 字段。
    pub fn all_schemas(&self) -> Vec<serde_json::Value> {
        self.tools.values().map(|tool| tool.schema()).collect()
    }
}

// ── 让 ToolRegistry 接入 AgentLoop ────────────────────────────────────────────

/// 让 ToolRegistry 实现 Session 01 定义的 ToolExecutor trait，
/// 这样 AgentLoop 不需要修改，直接换掉 EchoExecutor 即可。
///
/// Exercise 4：补全这个实现。
impl crate::loop_::ToolExecutor for ToolRegistry {
    async fn execute(&self, call: &ToolUse) -> Result<ToolResult, anyhow::Error> {
        self.dispatch(call)
    }
}
