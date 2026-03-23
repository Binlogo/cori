# Session 02 · Tool Dispatch

> **Motto**: Tools aren't magic — they're a dispatch map.

---

## 概念先行

上一节，AgentLoop 里用的是 `EchoExecutor`——一个假的工具执行器。
这一节，我们把它替换成真正的 Tool 系统。

核心结构很简单：

```
┌─────────────────────────────────────────────────┐
│                  ToolRegistry                   │
│                                                 │
│   "bash"    ──▶  BashTool.execute(input)        │
│   "read"    ──▶  ReadTool.execute(input)        │
│   "glob"    ──▶  GlobTool.execute(input)        │
│                                                 │
│   HashMap<String, Box<dyn Tool>>                │
└─────────────────────────────────────────────────┘
```

Claude 调用工具时，发来的是：

```json
{ "type": "tool_use", "name": "bash", "input": { "command": "ls" } }
```

Registry 做的事：
1. 用 `name` 查 HashMap，找到对应的 `Tool`
2. 把 `input` 传给 `Tool::execute()`
3. 把结果包装成 `ToolResult` 返回

---

## Schema 是工具的"说明书"

Claude 在发出 tool_call 之前，需要先"看到"工具列表。
这通过 API 请求里的 `tools` 字段传递：

```json
{
  "model": "claude-opus-4-6",
  "tools": [
    {
      "name": "bash",
      "description": "Execute a shell command...",
      "input_schema": {
        "type": "object",
        "properties": {
          "command": { "type": "string" }
        },
        "required": ["command"]
      }
    }
  ],
  "messages": [...]
}
```

`input_schema` 是标准 JSON Schema。Claude 用它来决定：
- 这个工具能做什么
- 需要传哪些参数
- 参数是什么类型

**思考**：schema 的 `description` 写得好不好，会影响 Claude 使用工具的质量吗？

---

## 练习 1 — 读懂 Tool trait

打开 `cori-core/src/tools/mod.rs`，阅读 `Tool` trait 的三个方法。

**问题**：
- `name()` 返回的字符串，必须和什么保持一致？
- `execute()` 接收 `&serde_json::Value`，而不是强类型结构体，为什么？
- `schema()` 返回的 JSON 里，哪个字段名是 Claude API 规定的，不能随意更改？

---

## 练习 2 — 实现 BashTool::execute()

打开 `cori-core/src/tools/bash.rs`，补全 `execute()` 方法。

**关键决策**：命令执行失败（exit code != 0）时，返回 `Err` 还是 `Ok`？

```
选项 A：Err(anyhow!("command failed"))
  → AgentLoop 会把这个错误抛出，整个任务终止

选项 B：Ok("error: No such file or directory")
  → 错误信息作为 tool_result 返回给 Claude
  → Claude 可以看到错误，决定下一步怎么办
```

Claude Code 选的是 **B**。这是一个重要的设计原则：
**让 Claude 看到错误，而不是让 Agent 崩溃。**

实现提示：
```rust
use std::process::Command;

let output = Command::new("sh")
    .arg("-c")
    .arg(command)
    .output()?;  // 这里的 ? 只处理"命令无法启动"的情况

// stdout 和 stderr 都需要返回
```

---

## 练习 3 — 实现 ToolRegistry

打开 `cori-core/src/tools/mod.rs`，补全三个方法：

**`register()`**：

```
数据结构选 HashMap<String, Box<dyn Tool>>
key = tool.name().to_string()

思考：为什么需要 Box<dyn Tool>？
能不能用 impl Tool？
```

**`dispatch()`**：

测试 `test_unknown_tool_returns_ok` 给了你方向提示：
未知工具不应该让整个 loop 崩溃，而是返回一条告知 Claude 的消息。

**`all_schemas()`**：

```rust
// 一行能写完
self.tools.values().map(|t| t.schema()).collect()
```

---

## 练习 4 — 接入 AgentLoop

`ToolRegistry` 实现了 `ToolExecutor` trait，可以直接替换 `EchoExecutor`：

```rust
// 之前（Session 01）
let agent = AgentLoop::new(MockLlm::new(...), EchoExecutor);

// 现在（Session 02）
let mut registry = ToolRegistry::new();
registry.register(BashTool);
let agent = AgentLoop::new(MockLlm::new(...), registry);
```

无需修改 AgentLoop。这就是 trait 抽象的价值。

---

## 检查点

运行测试：

```bash
cargo test -p cori-core
```

通过所有测试后，能回答：

- [ ] 为什么工具失败要返回 `Ok` 而不是 `Err`？
- [ ] `ToolRegistry` 的 key 是什么类型？为什么不直接存 `&str`？
- [ ] `all_schemas()` 的结果在哪里被用到？（提示：Session 03 会把它真正发给 Claude）

---

## 下一课

[Session 03 · Real API Call →](/lessons/03-real-api) *(coming soon)*
