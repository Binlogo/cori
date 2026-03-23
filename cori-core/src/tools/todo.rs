/// Session 05 · TodoWrite & TodoRead 工具
///
/// 这两个工具的职责分工：
///   TodoWrite：Claude 用来更新任务计划（整体替换）
///   TodoRead ：Claude 用来查看当前任务状态（只读）
///
/// Exercise 4：补全两个工具的 execute() 实现。
use std::sync::{Arc, Mutex};

use crate::{
    planner::{Task, TaskList},
    tools::Tool,
};

// ── TodoRead ──────────────────────────────────────────────────────────────────

pub struct TodoReadTool {
    list: Arc<Mutex<TaskList>>,
}

impl TodoReadTool {
    pub fn new(list: Arc<Mutex<TaskList>>) -> Self {
        Self { list }
    }
}

impl Tool for TodoReadTool {
    fn name(&self) -> &str {
        "todo_read"
    }

    fn execute(&self, _input: &serde_json::Value) -> Result<String, anyhow::Error> {
        // TODO: 锁住 self.list，调用 display()，返回结果
        // 如果任务列表为空，返回 "No tasks." 而不是空字符串
        let list = self.list.lock().unwrap();
        if list.tasks().is_empty() {
            Ok("No  tasks".to_string())
        } else {
            Ok(list.display())
        }
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "todo_read",
            "description": "Read the current task list. Use this to check what tasks are planned and their status.",
            "input_schema": {
                "type": "object",
                "properties": {}
            }
        })
    }
}

// ── TodoWrite ─────────────────────────────────────────────────────────────────

pub struct TodoWriteTool {
    list: Arc<Mutex<TaskList>>,
}

impl TodoWriteTool {
    pub fn new(list: Arc<Mutex<TaskList>>) -> Self {
        Self { list }
    }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "todo_write"
    }

    /// input 格式：
    /// {
    ///   "tasks": [
    ///     { "id": "1", "description": "...", "state": "pending" },
    ///     { "id": "2", "description": "...", "state": "in_progress" }
    ///   ]
    /// }
    fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        //   1. 从 input["tasks"] 反序列化出 Vec<Task>
        //      提示：serde_json::from_value(input["tasks"].clone())?
        //   2. 调用 self.list.lock().unwrap().write(tasks)?
        //   3. 返回 "Tasks updated." 作为确认
        let tasks = serde_json::from_value(input["tasks"].clone())?;

        self.list.lock().unwrap().write(tasks)?;

        Ok("Tasks updated".to_string())
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "todo_write",
            "description": "Create or update the task list. Replace the entire list with the provided tasks. Use this to plan work and track progress by updating task states.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "tasks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id":          { "type": "string" },
                                "description": { "type": "string" },
                                "state":       { "type": "string", "enum": ["pending", "in_progress", "completed"] }
                            },
                            "required": ["id", "description", "state"]
                        }
                    }
                },
                "required": ["tasks"]
            }
        })
    }
}
