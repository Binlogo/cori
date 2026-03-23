# Session 04 · Context & Token Management

> **Motto**: Memory costs tokens — manage it or hit the wall.

---

## 概念先行

Agent Loop 每跑一轮，`messages` 就增长：

```
turn 1: [user]                                    ~100 tokens
turn 2: [user, assistant(tool_use), user(result)] ~400 tokens
turn 3: [user, ..., assistant(tool_use), user(result)] ~700 tokens
...
turn N: 撞上 context window 上限 → API 报错
```

Claude Opus 4 的 context window 是 200k tokens。看起来很大，但一个复杂任务跑 30 轮，每轮工具输出几千 tokens，很快就满了。

**三层应对策略**（Claude Code 的实际做法）：

| 层级 | 触发条件 | 策略 |
|------|----------|------|
| Micro | 每轮结束 | 用占位符替换旧的工具结果 |
| Auto | 达到 token 阈值 | 截断旧消息，保留头尾 |
| Manual | 用户手动触发 | 对全部历史做摘要压缩 |

本节实现 **Auto 层**：最简单但最关键的一层。

---

## 数据流

```
AgentLoop::run()
  │
  ├─ send() → response.usage.input_tokens   ← Session 03 新增的字段
  │
  └─ 下一轮开始前：
       ContextManager::should_truncate(input_tokens)?
         ├─ false → 继续
         └─ true  → truncate(&mut messages) → 继续
```

---

## 练习 1 — 实现 `ContextManager::truncate()`

打开 `cori-core/src/context.rs`，补全 `truncate()` 方法。

截断规则：
- `messages[0]` **永远保留**（原始用户请求）
- 末尾 `keep_last` 条保留
- 中间的全部丢弃

```
before: [msg0, msg1, msg2, msg3, msg4, msg5, msg6]  keep_last=3
after:  [msg0,                   msg4, msg5, msg6]
```

代码骨架注释里有提示。完成后运行：

```bash
cargo test -p cori-core context
```

4 个测试全绿即完成。

---

## 练习 2 — 在 AgentLoop 里触发截断

打开 `cori-core/src/loop_.rs`，找到注释 `// Exercise 2`，补全这两行：

```rust
if self.context.should_truncate(last_input_tokens) {
    tracing::warn!(
        input_tokens = last_input_tokens,
        messages = messages.len(),
        "context truncated"
    );
    self.context.truncate(&mut messages);
}
```

**思考**：为什么截断在 `send()` **之前**而不是之后？
截断后 `last_input_tokens` 会不会立刻变小？（不会，要等下一次 send 才能拿到新的 token 数）

---

## 检查点

```bash
cargo test -p cori-core
```

所有测试通过后，能回答：

- [ ] 为什么截断时 `messages[0]` 必须保留？
- [ ] 截断后的 token 数是多少？（提示：不知道，要等下次 API 响应才能知道）
- [ ] 为什么 `keep_last` 应该是偶数？（提示：消息是成对出现的——assistant + user）

---

## 下一课

[Session 05 · Planning & Tasks →](/lessons/05-planning) *(coming soon)*
