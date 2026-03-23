# Session 06 · Subagents

> **Motto**: A subagent is just another AgentLoop with a fresh context.

---

## 概念先行

子 Agent 没有什么神奇的地方：

```rust
// 父 Agent 调用 spawn_subagent 工具时，内部发生的事：
let mut registry = ToolRegistry::new();
registry.register(BashTool);
let llm = ClaudeLlm::from_env(registry.all_schemas())?;
let mut agent = AgentLoop::new(llm, registry);  // ← 全新的 Vec<Message>
agent.run(subtask).await                         // ← 在隔离上下文里执行
```

就这些。隔离 = 新的 `Vec<Message>`。

---

## 为什么需要子 Agent？

**问题**：长任务的 context 会越来越大。

```
主任务 turn 1:  [user]                           ~100 tokens
主任务 turn 10: [user, ...(10轮对话)...]          ~5000 tokens
主任务 turn 30: [user, ...(30轮对话)...]          ~20000 tokens
```

**解决方案**：把独立的子任务委托给子 Agent。
子 Agent 只携带子任务相关的上下文，完成后把结果（一段文字）返回给父 Agent。

```
父 Agent context            子 Agent context
─────────────────           ─────────────────────────
turn 1: user                turn 1: user (子任务描述)
turn 2: assistant           turn 2: assistant + tool_use
turn 3: tool_results        turn 3: tool_results
turn 4: "子任务完成: ..."   turn 4: end_turn → 返回结果
```

---

## 本节的架构变化

为了让 `SubagentTool` 能 `await` 子 Agent，`Tool::execute` 需要变成 `async`。

**问题**：`async fn` 在 trait 里不是 object-safe，无法用 `Box<dyn Tool>`。

```
async fn 在 trait 里 → 生成与 Self 绑定的关联 future 类型
Box<dyn Tool> 要求 trait object-safe → 矛盾
```

**解决方案**：`#[async_trait]` 宏通过把 future 装箱（`Box<dyn Future>`）绕过这个限制。

```rust
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    async fn execute(&self, input: &Value) -> Result<String>;
}
```

这是本节脚手架已经完成的改动。

---

## 练习 1 — 实现 `SubagentTool::execute()`

打开 `cori-core/src/tools/subagent.rs`，补全 `execute()` 方法。

注释里已有完整的代码骨架，把注释取消掉然后去掉 `todo!` 即可。

核心是这四行：
```rust
let mut registry = ToolRegistry::new();
registry.register(BashTool);
let llm = crate::claude::ClaudeLlm::from_env(registry.all_schemas())?;
let mut agent = crate::loop_::AgentLoop::new(llm, registry);
agent.run(task).await
```

---

## 练习 2 — 注册并验证

更新 `examples/hello.rs`，给父 Agent 注册 `SubagentTool`，然后让父 Agent 执行一个需要委托子任务的工作：

```rust
registry.register(cori_core::tools::subagent::SubagentTool);
```

给父 Agent 的 prompt：
```
请用 spawn_subagent 工具分别完成两个子任务：
1. 列出当前目录的 .rs 文件数量
2. 查看系统的 Rust 版本
最后汇总结果。
```

观察：父 Agent 是否会调用两次 `spawn_subagent`？

---

## 延伸思考

- **子 Agent 能不能再 spawn 子 Agent？**
  可以，但要小心递归深度和成本。

- **父子共享任务列表吗？**
  不共享。子 Agent 有自己独立的工具集，没有 `TodoTools`。
  这是设计上的选择：子任务应该是自包含的。

- **子任务失败时，父 Agent 怎么办？**
  子 Agent 的错误会作为 `tool_result` 返回，
  Claude 可以看到错误并决定是否重试或换策略。

---

## 检查点

```bash
cargo run -p cori-core --example hello
```

观察到父 Agent 调用 `spawn_subagent`，子 Agent 独立执行并返回结果后，能回答：

- [ ] 子 Agent 的上下文隔离体现在哪一行代码？
- [ ] 为什么不直接在父 Agent 里执行子任务？
- [ ] `#[async_trait]` 解决了什么编译器错误？

---

## 下一课

[Session 07 · File System Tools →](/lessons/07-file-tools) *(coming soon)*
