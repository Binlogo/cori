# Session 01 · The Agent Loop

> **Motto**: One loop & one tool — that's all an agent is.

---

## 概念先行

在开始写代码之前，先建立一个心智模型。

Claude Code 的核心，是一个**循环**：

```
┌─────────────────────────────────────────────┐
│                Agent Loop                   │
│                                             │
│  ┌──────────┐    tool_call    ┌──────────┐  │
│  │          │ ──────────────▶ │          │  │
│  │  Claude  │                 │   Tool   │  │
│  │  (LLM)   │ ◀────────────── │ Executor │  │
│  │          │   tool_result   │          │  │
│  └──────────┘                 └──────────┘  │
│       │                                     │
│       │ stop_reason = "end_turn"            │
│       ▼                                     │
│    [ Done ]                                 │
└─────────────────────────────────────────────┘
```

每一轮循环：

1. 把 `messages`（对话历史）发给 Claude
2. Claude 返回响应，检查 `stop_reason`
3. 如果是 `"tool_use"` → 执行工具，把结果追加到 `messages`，**继续循环**
4. 如果是 `"end_turn"` → 输出最终回答，**退出**

就这么简单。没有魔法。

---

## 关键数据结构

Claude API 的消息格式（JSON）：

```json
{
  "role": "assistant",
  "content": [
    {
      "type": "tool_use",
      "id": "toolu_01abc",
      "name": "bash",
      "input": { "command": "ls -la" }
    }
  ]
}
```

当你把工具结果返回时：

```json
{
  "role": "user",
  "content": [
    {
      "type": "tool_result",
      "tool_use_id": "toolu_01abc",
      "content": "total 8\ndrwxr-xr-x  3 user  staff   96 Mar 23 10:00 ."
    }
  ]
}
```

**注意**：`tool_result` 的 role 是 `"user"`，不是 `"assistant"`。这是理解 Agent 工作原理的关键细节。

---

## 练习 1 — 理解循环退出条件

**问题**：下面的伪代码有什么 bug？

```rust
loop {
    let response = claude.send(&messages).await?;
    let tool_calls = response.tool_calls();

    for call in tool_calls {
        let result = executor.run(&call).await?;
        messages.push_tool_result(call.id, result);
    }

    if response.stop_reason == "end_turn" {
        break;
    }
}
```

> **思考**：如果 Claude 返回了工具调用，同时 `stop_reason` 也是 `"end_turn"`，会发生什么？
> 实际上，`stop_reason = "tool_use"` 时不会携带 `"end_turn"`，但如果工具列表为空时直接 break 会有问题吗？

在 `cori-core/src/` 下，你将实现这个循环。先不用实际调用 Claude API，用一个 mock 替代。

---

## 练习 2 — 实现消息追加

Agent Loop 的核心状态是 `Vec<Message>`。每轮对话都在这个向量上追加。

**你的任务**：在 `cori-core/src/` 中定义以下类型（不要参考答案，先自己想）：

```rust
// 提示：需要表达 user / assistant 两种 role
// 需要表达 text / tool_use / tool_result 三种 content 类型
pub struct Message { /* 你来设计 */ }
pub enum Role { /* ... */ }
pub enum Content { /* ... */ }
```

设计完后，思考：为什么 `tool_result` 的 role 是 `user` 而不是 `tool`？

---

## 练习 3 — 加入安全阀

一个没有终止条件的 Agent 是危险的。

**你的任务**：给循环加入 `max_turns: usize` 限制，超出时返回一个 `Error`。

```rust
pub struct AgentLoop {
    max_turns: usize,
    // ...
}
```

**延伸思考**：Claude Code 的默认 max_turns 是多少？为什么不设成无限？

---

## 检查点

完成以上练习后，你应该能回答：

- [ ] Agent Loop 的退出条件是什么？
- [ ] 为什么 `tool_result` 的 role 是 `"user"`？
- [ ] `messages` 向量在一次任务执行过程中会增长到多大？

---

## 下一课

[Session 02 · Tool Dispatch →](/lessons/02-tool-dispatch) *(coming soon)*
