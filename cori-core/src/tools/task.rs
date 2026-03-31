/// Session 09 · Task Graph 工具（对齐 Claude Code 2.1.16）
///
/// 四个工具对应 Claude Code 的 TaskCreate / TaskUpdate / TaskGet / TaskList：
///   task_create  — 创建单个任务，系统自动分配 ID
///   task_update  — 增量更新：status / subject / description /
///                  add_blocked_by / add_blocks / owner
///   task_get     — 读取单个任务完整信息（含 blocks / blocked_by）
///   task_list    — 列出所有任务，按 IN PROGRESS / READY / BLOCKED / COMPLETED 分区
///
/// 关键设计：add_blocked_by 和 add_blocks 会触发双向链接维护，
/// Claude 只需声明单向依赖，harness 负责保持图的一致性。
use std::sync::{Arc, Mutex};

use crate::{
    planner::{TaskGraph, TaskStatus},
    tools::Tool,
};

// ── TaskListTool ──────────────────────────────────────────────────────────────

pub struct TaskListTool {
    graph: Arc<Mutex<TaskGraph>>,
}

impl TaskListTool {
    pub fn new(graph: Arc<Mutex<TaskGraph>>) -> Self {
        Self { graph }
    }
}

#[async_trait::async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "task_list"
    }

    async fn execute(&self, _input: &serde_json::Value) -> Result<String, anyhow::Error> {
        self.graph.lock().unwrap().display()
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "task_list",
            "description": "List all tasks grouped by status: IN PROGRESS, READY (can start now), BLOCKED (waiting on dependencies), COMPLETED. The blockedBy shown are only open (incomplete) dependencies.",
            "input_schema": {
                "type": "object",
                "properties": {}
            }
        })
    }
}

// ── TaskCreateTool ────────────────────────────────────────────────────────────

pub struct TaskCreateTool {
    graph: Arc<Mutex<TaskGraph>>,
}

impl TaskCreateTool {
    pub fn new(graph: Arc<Mutex<TaskGraph>>) -> Self {
        Self { graph }
    }
}

#[async_trait::async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "task_create"
    }

    /// input:
    /// {
    ///   "subject":     "Fix auth bug",
    ///   "description": "The login endpoint returns 500 when...",
    ///   "active_form": "Fixing auth bug"   (optional)
    /// }
    async fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        let subject = input["subject"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("subject is required"))?;
        let description = input["description"].as_str().unwrap_or("").to_string();
        let active_form = input["active_form"].as_str().map(str::to_owned);

        let task = self
            .graph
            .lock()
            .unwrap()
            .create(subject, description, active_form)?;

        Ok(format!(
            "Task #{} created: {}",
            task.id, task.subject
        ))
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "task_create",
            "description": "Create a new task. The system assigns the ID automatically. After creating tasks, use task_update to set up dependencies (add_blocked_by / add_blocks).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Brief task title in imperative form (e.g. 'Fix auth bug')"
                    },
                    "description": {
                        "type": "string",
                        "description": "Detailed description of what needs to be done"
                    },
                    "active_form": {
                        "type": "string",
                        "description": "Present continuous form shown in spinner when in_progress (e.g. 'Fixing auth bug')"
                    }
                },
                "required": ["subject", "description"]
            }
        })
    }
}

// ── TaskGetTool ───────────────────────────────────────────────────────────────

pub struct TaskGetTool {
    graph: Arc<Mutex<TaskGraph>>,
}

impl TaskGetTool {
    pub fn new(graph: Arc<Mutex<TaskGraph>>) -> Self {
        Self { graph }
    }
}

#[async_trait::async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "task_get"
    }

    async fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        let id = input["task_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("task_id is required"))?;
        let task = self.graph.lock().unwrap().get(id)?;
        Ok(serde_json::to_string_pretty(&task)?)
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "task_get",
            "description": "Get full details of a task by ID, including blocks and blocked_by lists.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "task_id": { "type": "string", "description": "The task ID" }
                },
                "required": ["task_id"]
            }
        })
    }
}

// ── TaskUpdateTool ────────────────────────────────────────────────────────────

pub struct TaskUpdateTool {
    graph: Arc<Mutex<TaskGraph>>,
}

impl TaskUpdateTool {
    pub fn new(graph: Arc<Mutex<TaskGraph>>) -> Self {
        Self { graph }
    }
}

#[async_trait::async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "task_update"
    }

    /// input:
    /// {
    ///   "task_id":       "2",
    ///   "status":        "in_progress",          (optional)
    ///   "subject":       "New title",             (optional)
    ///   "description":   "New description",       (optional)
    ///   "add_blocked_by": ["1"],                  (optional) — 双向维护
    ///   "add_blocks":     ["3"],                  (optional) — 双向维护
    ///   "owner":         "agent-name"             (optional)
    /// }
    async fn execute(&self, input: &serde_json::Value) -> Result<String, anyhow::Error> {
        let id = input["task_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("task_id is required"))?;

        let status = input["status"].as_str().map(|s| match s {
            "pending" => Ok(TaskStatus::Pending),
            "in_progress" => Ok(TaskStatus::InProgress),
            "completed" => Ok(TaskStatus::Completed),
            "deleted" => Ok(TaskStatus::Deleted),
            other => Err(anyhow::anyhow!("unknown status: {other}")),
        }).transpose()?;

        let subject = input["subject"].as_str().map(str::to_owned);
        let description = input["description"].as_str().map(str::to_owned);
        let owner = input["owner"].as_str().map(str::to_owned);

        let add_blocked_by = input["add_blocked_by"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect());

        let add_blocks = input["add_blocks"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect());

        let task = self.graph.lock().unwrap().update(
            id,
            status,
            subject,
            description,
            add_blocked_by,
            add_blocks,
            owner,
        )?;

        Ok(format!("Task #{} updated: status={}", task.id, task.status))
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "task_update",
            "description": "Update a task. Use add_blocked_by to declare that this task depends on others; use add_blocks to declare that this task blocks others. Both maintain bidirectional links automatically. Dependencies can only be added, not removed.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "The task ID to update"
                    },
                    "status": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "completed", "deleted"],
                        "description": "New status. 'deleted' permanently hides the task."
                    },
                    "subject": {
                        "type": "string",
                        "description": "New brief title"
                    },
                    "description": {
                        "type": "string",
                        "description": "New detailed description"
                    },
                    "add_blocked_by": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Task IDs that must complete before this task can start. Bidirectional link maintained automatically."
                    },
                    "add_blocks": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Task IDs that cannot start until this task completes. Bidirectional link maintained automatically."
                    },
                    "owner": {
                        "type": "string",
                        "description": "Agent name claiming this task"
                    }
                },
                "required": ["task_id"]
            }
        })
    }
}
