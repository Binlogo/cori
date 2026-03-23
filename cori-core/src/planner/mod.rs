/// Session 05 · Planning & Tasks
///
/// 关键洞察：任务列表不是 harness 维护的，而是 Claude 自己通过工具调用维护的。
/// harness 只提供 TodoWrite / TodoRead 两个工具，Claude 决定什么时候调用它们。
///
/// 这带来一个重要特性：任务列表持久化到文件，即使上下文被截断，
/// Claude 也可以通过 TodoRead 重新加载当前任务状态。

mod tests;

use std::path::{Path, PathBuf};

// ── TaskState ─────────────────────────────────────────────────────────────────

/// 任务状态机：只允许单向流转
///
///   pending → in_progress → completed
///
/// Exercise 1：为什么不允许从 completed 回到 in_progress？
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    InProgress,
    Completed,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskState::Pending => write!(f, "pending"),
            TaskState::InProgress => write!(f, "in_progress"),
            TaskState::Completed => write!(f, "completed"),
        }
    }
}

// ── Task ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub state: TaskState,
}

impl Task {
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            state: TaskState::Pending,
        }
    }
}

// ── TaskList ──────────────────────────────────────────────────────────────────

/// 任务列表，持久化到 JSON 文件。
///
/// 持久化是这个系统的核心价值：
///   - 上下文被截断后，Claude 通过 TodoRead 重新加载任务状态
///   - Agent 崩溃后重启，可以从上次的状态继续
pub struct TaskList {
    tasks: Vec<Task>,
    path: PathBuf,
}

impl TaskList {
    /// 从文件加载，文件不存在时返回空列表
    pub fn load(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let path = path.as_ref().to_path_buf();
        let tasks = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            serde_json::from_str(&content)?
        } else {
            vec![]
        };
        Ok(Self { tasks, path })
    }

    /// 用新的任务列表整体替换（TodoWrite 的语义）
    ///
    /// Exercise 2：补全这个方法。
    /// 为什么是整体替换而不是增量更新？
    ///   Claude 每次 TodoWrite 时传入完整列表，harness 直接覆盖存储。
    ///   这比维护 diff 简单得多，且对 Claude 的认知负担更小。
    pub fn write(&mut self, tasks: Vec<Task>) -> Result<(), anyhow::Error> {
        // TODO: 更新 self.tasks，然后持久化到 self.path
        // 提示：serde_json::to_string_pretty(&self.tasks)?
        todo!("替换任务列表并写入文件")
    }

    /// 以人类可读格式返回当前任务列表（TodoRead 的输出）
    ///
    /// Exercise 3：补全这个方法。
    /// 输出示例：
    ///   [ ] 1. 分析项目结构
    ///   [→] 2. 实现核心功能
    ///   [✓] 3. 编写测试
    pub fn display(&self) -> String {
        // TODO: 遍历 self.tasks，根据 state 选择对应的符号
        // pending      → [ ]
        // in_progress  → [→]
        // completed    → [✓]
        todo!("格式化任务列表")
    }

    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }
}
