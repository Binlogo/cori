/// Session 01 · Exercise 2
/// 定义 Agent 消息的核心数据结构。
///
/// Claude API 的对话由一个 `Vec<Message>` 表示，每轮追加。
/// 你的任务：补全下面三个类型的定义，让 `TODO` 消失。
///
/// 提示：
/// - `Role` 只有两种：谁在说话？
/// - `Content` 需要表达三种情况：纯文本、工具调用请求、工具调用结果
/// - 想想 `tool_result` 为什么属于 `"user"` 角色，而不是单独一种角色

// ── Role ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    // TODO: 填入两个变体
}

// ── Content ───────────────────────────────────────────────────────────────────

/// 工具调用请求（由 Claude 发出）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// 工具调用结果（由 Executor 返回，封装后发回 Claude）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    // TODO: 三个变体
    // Text { text: String }
    // ToolUse(ToolUse)
    // ToolResult(ToolResult)
}

// ── Message ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Content>,
}

impl Message {
    /// 构造一条用户文本消息
    pub fn user(text: impl Into<String>) -> Self {
        // TODO
        todo!("构造 role=User, content=[Content::Text] 的消息")
    }

    /// 把一批工具结果包装成一条 user 消息发回 Claude
    ///
    /// 思考：为什么多个 tool_result 可以合并在同一条消息里？
    pub fn tool_results(results: Vec<ToolResult>) -> Self {
        // TODO
        todo!("构造 role=User, content=[Content::ToolResult, ...] 的消息")
    }
}
