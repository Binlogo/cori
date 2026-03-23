# Session 05 · Planning & Tasks

> **Motto**: Let Claude manage its own task list.

---

## 概念先行

一个重要的认知转变：

```
❌ 错误理解：harness 帮 Claude 规划任务
✅ 正确理解：Claude 自己调用工具来管理任务
```

Claude Code 里的 `TodoWrite` 是一个普通工具，和 `bash` 没有本质区别。
Claude 自己决定什么时候调用它，harness 只负责执行和持久化。

```
Claude："我需要完成三件事，先规划一下"
  → 调用 TodoWrite { tasks: [{id:1, 分析结构, pending}, ...] }
  → harness 写入文件，返回 "Tasks updated."
  → Claude 开始执行第一个任务
  → 调用 TodoWrite { tasks: [{id:1, 分析结构, completed}, {id:2, ..., in_progress}] }
  → ...
```

**持久化的价值**：任务文件在磁盘上，不受 context 截断影响。
Claude 随时可以 `TodoRead` 来"想起"自己在做什么。

---

## 状态机

```
pending ──▶ in_progress ──▶ completed
```

只允许单向流转。为什么不允许 `completed → in_progress`？
因为 Claude 如果能"反悔"，就很难追踪真实进度。
已完成的任务不应该被重新打开，应该新建一个任务。

---

## 练习 1 — 理解数据结构

打开 `cori-core/src/planner/mod.rs`，阅读 `Task`、`TaskState`、`TaskList`。

**问题**：
- `TaskList::write()` 为什么是整体替换，而不是按 id 更新单个任务？
- `TaskList::load()` 在文件不存在时不报错，而是返回空列表，为什么？

---

## 练习 2 — 实现 `TaskList::write()`

```rust
pub fn write(&mut self, tasks: Vec<Task>) -> Result<(), anyhow::Error> {
    self.tasks = tasks;
    let content = serde_json::to_string_pretty(&self.tasks)?;
    std::fs::write(&self.path, content)?;
    Ok(())
}
```

注意用 `to_string_pretty`，让文件人类可读（调试时有帮助）。

---

## 练习 3 — 实现 `TaskList::display()`

```
[ ] 1. 分析项目结构
[→] 2. 实现核心功能
[✓] 3. 编写测试
```

用 `enumerate()` 拿到序号，`match state` 选择符号：

```rust
TaskState::Pending     => "[ ]"
TaskState::InProgress  => "[→]"
TaskState::Completed   => "[✓]"
```

---

## 练习 4 — 实现 TodoRead & TodoWrite 工具

打开 `cori-core/src/tools/todo.rs`，补全两个 `execute()` 方法。

**TodoRead**：锁住列表，调用 `display()`，空时返回 `"No tasks."`

**TodoWrite**：
```rust
let tasks: Vec<crate::planner::Task> = serde_json::from_value(input["tasks"].clone())?;
self.list.lock().unwrap().write(tasks)?;
Ok("Tasks updated.".into())
```

**思考**：两个工具共享同一个 `Arc<Mutex<TaskList>>`。
如果不用 `Arc`，直接传 `&mut TaskList`，会遇到什么 Rust 编译错误？

---

## 把工具注册进来

练习完成后，更新 `examples/hello.rs`，注册 `TodoReadTool` 和 `TodoWriteTool`：

```rust
use std::sync::{Arc, Mutex};
use cori_core::planner::TaskList;
use cori_core::tools::todo::{TodoReadTool, TodoWriteTool};

let task_list = Arc::new(Mutex::new(TaskList::load(".cori_tasks.json")?));
registry.register(TodoReadTool::new(Arc::clone(&task_list)));
registry.register(TodoWriteTool::new(Arc::clone(&task_list)));
```

然后让 Claude 执行一个多步任务，观察它是否会自动调用 `TodoWrite` 来规划。

---

## 检查点

```bash
cargo test -p cori-core planner
```

3 个测试全绿后，能回答：

- [ ] 为什么任务列表是整体替换而不是增量更新？
- [ ] `Arc<Mutex<TaskList>>` 解决了什么问题？
- [ ] 任务列表持久化在哪里？context 截断后 Claude 怎么找回它？

---

## 下一课

[Session 06 · Subagents →](/lessons/06-subagents) *(coming soon)*
