/// Session 06 · Subagents
///
/// 核心洞察：子 Agent 不是特殊的基础设施，它只是另一个 AgentLoop。
/// 隔离来自于全新的 Vec<Message>，而不是 OS 级别的沙箱。
///
/// 父 Agent 调用 spawn_subagent 工具，传入子任务描述，
/// 子 Agent 在完全隔离的上下文里完成任务，把结果字符串返回给父 Agent。
///
/// 上下文隔离示意：
///
///   父 Agent context            子 Agent context
///   ─────────────────           ─────────────────
///   [user: 主任务]              [user: 子任务]       ← 全新的 Vec<Message>
///   [assistant: ...]            [assistant: ...]
///   [user: tool_results]        [user: tool_results]
///   [assistant: spawn req]      [assistant: end_turn]  → 结果返回父 Agent
///   [user: "子任务完成：..."]
///
/// 子 Agent 的工具集也是独立的，默认只有 BashTool（无状态工具）。
/// TodoTools 不传给子 Agent——子 Agent 没有持久化需求。

use crate::tools::{Tool, bash::BashTool};

pub struct SubagentTool;

#[async_trait::async_trait]
impl Tool for SubagentTool {
    fn name(&self) -> &str {
        "spawn_subagent"
    }

    /// 创建一个拥有隔离上下文的子 Agent，执行子任务。
    ///
    /// Exercise 1：补全这个方法。
    ///
    /// 步骤：
    ///   1. 从 input["task"] 读取子任务描述
    ///   2. 创建新的 ToolRegistry，只注册 BashTool
    ///   3. 创建新的 ClaudeLlm（from_env）
    ///   4. 创建新的 AgentLoop（这就是隔离的全部秘密）
    ///   5. agent.run(task).await
    ///
    /// 思考：
    ///   - 子 Agent 能不能再 spawn 子 Agent？会发生什么？
    ///   - 如果子任务失败，父 Agent 会怎么处理？
    async fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        let task = input["task"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'task' field"))?;

        // TODO: 创建子 Agent 并运行
        //
        // let mut registry = ToolRegistry::new();
        // registry.register(BashTool);
        // let llm = crate::claude::ClaudeLlm::from_env(registry.all_schemas())?;
        // let mut agent = crate::loop_::AgentLoop::new(llm, registry);
        // agent.run(task).await

        todo!("spawn subagent for task: {task}")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "spawn_subagent",
            "description": "Spawn a subagent with an isolated context to handle a focused subtask. \
                           The subagent runs independently and returns its result as a string. \
                           Use this to break complex tasks into parallel or sequential subtasks, \
                           each with a clean context.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "The subtask for the subagent to complete. Be specific and self-contained — the subagent has no knowledge of the parent context."
                    }
                },
                "required": ["task"]
            }
        })
    }
}
