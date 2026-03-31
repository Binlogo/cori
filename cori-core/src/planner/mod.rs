/// Session 09 · Task Graph（对齐 Claude Code 2.1.16 策略）
///
/// 设计决策：
///   - 增量 CRUD：task_create / task_update / task_get / task_list
///   - 每个任务独立文件：.tasks/{id}.json
///   - 双向链接由 harness 自动维护（addBlockedBy ↔ blocks）
///   - 依赖只能追加，不能移除（静态 DAG 结构）
///   - 自动分配 ID（max_id + 1）

#[cfg(test)]
mod tests;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

// ── TaskStatus ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Deleted,
}

impl TaskStatus {
    fn symbol(&self) -> &str {
        match self {
            TaskStatus::Pending => "[ ]",
            TaskStatus::InProgress => "[→]",
            TaskStatus::Completed => "[✓]",
            TaskStatus::Deleted => "[✗]",
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Deleted => write!(f, "deleted"),
        }
    }
}

// ── Task ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: String,
    /// 简短标题（imperative form，如 "Fix auth bug"）
    pub subject: String,
    /// 详细说明
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub status: TaskStatus,
    /// 本任务阻塞的后续任务 ID（harness 自动维护）
    #[serde(default)]
    pub blocks: Vec<String>,
    /// 阻塞本任务的前置任务 ID
    #[serde(default)]
    pub blocked_by: Vec<String>,
    /// 任务归属 agent（多 agent 场景）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// 进行中时 spinner 显示的文案（如 "Setting up project"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
}

impl Task {
    fn display_line(&self) -> String {
        format!("{} #{}  {}", self.status.symbol(), self.id, self.subject)
    }
}

// ── TaskGraph ─────────────────────────────────────────────────────────────────

/// 任务图，持久化到 .tasks/{id}.json。
///
/// 无内存缓存——所有操作直接读写磁盘。
/// 这与 Claude Code 的设计一致：状态的唯一来源是磁盘上的文件。
pub struct TaskGraph {
    dir: PathBuf,
}

impl TaskGraph {
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    // ── 内部辅助 ──────────────────────────────────────────────────────────────

    fn task_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn read_task(&self, id: &str) -> Result<Task, anyhow::Error> {
        let path = self.task_path(id);
        if !path.exists() {
            anyhow::bail!("Task {id} not found");
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    fn write_task(&self, task: &Task) -> Result<(), anyhow::Error> {
        let content = serde_json::to_string_pretty(task)?;
        std::fs::write(self.task_path(&task.id), content)?;
        Ok(())
    }

    fn next_id(&self) -> String {
        let max = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name();
                let stem = Path::new(&name).file_stem()?.to_str()?.to_owned();
                stem.parse::<u64>().ok()
            })
            .max()
            .unwrap_or(0);
        (max + 1).to_string()
    }

    fn all_tasks(&self) -> Result<Vec<Task>, anyhow::Error> {
        let mut tasks: Vec<Task> = std::fs::read_dir(&self.dir)?
            .flatten()
            .filter(|e| e.path().extension().map_or(false, |x| x == "json"))
            .filter_map(|e| {
                let content = std::fs::read_to_string(e.path()).ok()?;
                serde_json::from_str(&content).ok()
            })
            .filter(|t: &Task| t.status != TaskStatus::Deleted)
            .collect();
        tasks.sort_by(|a, b| {
            // 按数字排序（如果 id 是数字），否则按字符串
            let na = a.id.parse::<u64>().unwrap_or(u64::MAX);
            let nb = b.id.parse::<u64>().unwrap_or(u64::MAX);
            na.cmp(&nb).then(a.id.cmp(&b.id))
        });
        Ok(tasks)
    }

    // ── 公开 API ──────────────────────────────────────────────────────────────

    /// 创建新任务，返回带自动分配 ID 的 Task。
    pub fn create(
        &self,
        subject: impl Into<String>,
        description: impl Into<String>,
        active_form: Option<String>,
    ) -> Result<Task, anyhow::Error> {
        let task = Task {
            id: self.next_id(),
            subject: subject.into(),
            description: description.into(),
            status: TaskStatus::Pending,
            blocks: vec![],
            blocked_by: vec![],
            owner: None,
            active_form,
        };
        self.write_task(&task)?;
        Ok(task)
    }

    /// 读取单个任务完整信息。
    pub fn get(&self, id: &str) -> Result<Task, anyhow::Error> {
        self.read_task(id)
    }

    /// 增量更新任务。
    ///
    /// `add_blocked_by` 和 `add_blocks` 触发**双向链接维护**：
    ///   add_blocked_by: ["2"] → 本任务加入 blocked_by，任务 2 的 blocks 自动更新
    ///   add_blocks: ["3"]     → 本任务加入 blocks，   任务 3 的 blocked_by 自动更新
    pub fn update(
        &self,
        id: &str,
        status: Option<TaskStatus>,
        subject: Option<String>,
        description: Option<String>,
        add_blocked_by: Option<Vec<String>>,
        add_blocks: Option<Vec<String>>,
        owner: Option<String>,
    ) -> Result<Task, anyhow::Error> {
        let mut task = self.read_task(id)?;

        if let Some(s) = status {
            task.status = s;
        }
        if let Some(s) = subject {
            task.subject = s;
        }
        if let Some(d) = description {
            task.description = d;
        }
        if let Some(o) = owner {
            task.owner = Some(o);
        }

        // add_blocked_by：本任务依赖这些任务
        // → 同步更新对方的 blocks 列表
        if let Some(deps) = add_blocked_by {
            for dep_id in &deps {
                if !task.blocked_by.contains(dep_id) {
                    task.blocked_by.push(dep_id.clone());
                }
                // 对方 blocks 里加入本任务 ID
                if let Ok(mut dep_task) = self.read_task(dep_id) {
                    if !dep_task.blocks.contains(&task.id) {
                        dep_task.blocks.push(task.id.clone());
                        self.write_task(&dep_task)?;
                    }
                }
            }
        }

        // add_blocks：本任务阻塞这些任务
        // → 同步更新对方的 blocked_by 列表
        if let Some(blocking) = add_blocks {
            for blocked_id in &blocking {
                if !task.blocks.contains(blocked_id) {
                    task.blocks.push(blocked_id.clone());
                }
                if let Ok(mut blocked_task) = self.read_task(blocked_id) {
                    if !blocked_task.blocked_by.contains(&task.id) {
                        blocked_task.blocked_by.push(task.id.clone());
                        self.write_task(&blocked_task)?;
                    }
                }
            }
        }

        self.write_task(&task)?;
        Ok(task)
    }

    // ── 显示 ─────────────────────────────────────────────────────────────────

    /// 返回任务列表摘要，按 IN PROGRESS / READY / BLOCKED / COMPLETED 分区。
    ///
    /// blockedBy 列只显示**尚未完成**的依赖（open blockers），
    /// 与 Claude Code TaskList 的行为一致。
    pub fn display(&self) -> Result<String, anyhow::Error> {
        let tasks = self.all_tasks()?;
        if tasks.is_empty() {
            return Ok("No tasks.".to_string());
        }

        let completed_ids: HashSet<&str> = tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.as_str())
            .collect();

        let mut in_progress = vec![];
        let mut ready = vec![];
        let mut blocked = vec![];
        let mut completed = vec![];

        for t in &tasks {
            match t.status {
                TaskStatus::Completed => completed.push(t),
                TaskStatus::InProgress => in_progress.push(t),
                TaskStatus::Pending => {
                    let open_blockers: Vec<&str> = t
                        .blocked_by
                        .iter()
                        .filter(|dep| !completed_ids.contains(dep.as_str()))
                        .map(|s| s.as_str())
                        .collect();
                    if open_blockers.is_empty() {
                        ready.push(t);
                    } else {
                        blocked.push((t, open_blockers));
                    }
                }
                TaskStatus::Deleted => {}
            }
        }

        let mut out = String::new();

        if !in_progress.is_empty() {
            out.push_str("IN PROGRESS:\n");
            for t in &in_progress {
                out.push_str(&format!("  {}\n", t.display_line()));
            }
        }
        if !ready.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("READY:\n");
            for t in &ready {
                out.push_str(&format!("  {}\n", t.display_line()));
            }
        }
        if !blocked.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("BLOCKED:\n");
            for (t, open) in &blocked {
                out.push_str(&format!(
                    "  {}  ← blocked by: #{}\n",
                    t.display_line(),
                    open.join(", #")
                ));
            }
        }
        if !completed.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str("COMPLETED:\n");
            for t in &completed {
                out.push_str(&format!("  {}\n", t.display_line()));
            }
        }

        Ok(out.trim_end().to_string())
    }
}
