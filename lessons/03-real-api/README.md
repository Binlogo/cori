# Session 03 · Real API Call

> **Motto**: Replace the mock — speak to Claude for real.

---

## 概念先行

前两节我们用 `MockLlm` 假装在调用 Claude。现在让它真正跑起来。

Anthropic 的 Messages API 是一个普通的 HTTP POST：

```
POST https://api.anthropic.com/v1/messages
Content-Type: application/json
x-api-key: sk-ant-...
anthropic-version: 2023-06-01

{
  "model": "claude-opus-4-6",
  "max_tokens": 4096,
  "tools": [ ...来自 ToolRegistry::all_schemas()... ],
  "messages": [ ...我们的 Vec<Message> 直接序列化... ]
}
```

响应：

```json
{
  "stop_reason": "tool_use",
  "content": [
    { "type": "tool_use", "id": "toolu_01", "name": "bash", "input": { "command": "ls" } }
  ]
}
```

注意：我们在 Session 01 定义的 `Message` 类型，序列化出来的格式和 API 要求的格式完全一致——这是当时设计时就对齐的。

---

## 练习 1 — 读取 API Key

打开 `cori-core/src/claude.rs`，补全 `ClaudeLlm::from_env()`。

```rust
// 读取环境变量
std::env::var("ANTHROPIC_API_KEY")?
```

**不要** hardcode API Key，永远从环境变量读取。

---

## 练习 2 — 构造请求 Body

在 `send()` 里，用 `serde_json::json!` 宏构造请求 body：

```
需要的字段：
  model       → self.model
  max_tokens  → 4096（先用固定值，Session 04 会动态计算）
  tools       → self.tools
  messages    → messages（直接序列化）
```

**思考**：`max_tokens` 设太小会发生什么？
Claude 会在 `max_tokens` 耗尽时停下，返回 `stop_reason: "max_tokens"`。
现在的 AgentLoop 遇到这种情况会怎么处理？

---

## 练习 3 — 发送 HTTP 请求

```rust
let resp = self.client
    .post("https://api.anthropic.com/v1/messages")
    .header("x-api-key", &self.api_key)
    .header("anthropic-version", "2023-06-01")
    .json(&body)
    .send()
    .await?;
```

发送后，先检查 HTTP 状态码再解析 JSON：

```rust
// 如果状态码不是 2xx，先把错误 body 读出来，方便调试
if !resp.status().is_success() {
    let status = resp.status();
    let text = resp.text().await?;
    anyhow::bail!("API error {status}: {text}");
}

let api_response: ApiResponse = resp.json().await?;
```

**思考**：为什么要先检查状态码，而不是直接 `.json()`？

---

## 练习 4 — 解析响应

补全 `parse_response()`，把 `ApiResponse` 里的 `content` 数组转换成：
- `text_parts`：收集所有 `Text` 块
- `tool_calls`：收集所有 `ToolUse` 块，转成 `crate::types::ToolUse`

这部分没有太多技巧，就是 pattern match + 类型转换。

---

## 本节附带修复：并行工具调用的 History Bug

Session 01 遗留了一个问题：如果 Claude 在一轮里发出多个 tool_call（并行调用），
之前的代码会把它们拆成多条 assistant 消息：

```
❌ 错误的 messages 历史
[user, assistant(tool_use_1), assistant(tool_use_2), user(results)]

✅ 正确的 messages 历史
[user, assistant(tool_use_1 + tool_use_2), user(result_1 + result_2)]
```

Claude API 要求：同一轮的所有 `tool_use` 必须在**同一条** assistant 消息里，否则会报错。
这个 bug 用 MockLlm 时不会暴露，但接上真实 API 就会触发。

这个修复已经在 `loop_.rs` 里完成了（见 `Message::tool_uses()`），这节不需要你动。
但要理解为什么——这是 Session 01 练习里"多个 tool_result 为什么能合并"那个问题的另一面。

---

## 运行验证

```bash
export ANTHROPIC_API_KEY=sk-ant-...
cargo run -p cori-web
```

或者写一个 `examples/hello.rs`：

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut registry = cori_core::tools::ToolRegistry::new();
    registry.register(cori_core::tools::bash::BashTool);

    let llm = cori_core::claude::ClaudeLlm::from_env(registry.all_schemas())?;
    let mut agent = cori_core::loop_::AgentLoop::new(llm, registry);

    let answer = agent.run("用 bash 工具列出当前目录有哪些文件").await?;
    println!("{answer}");
    Ok(())
}
```

---

## 检查点

实现完成后，能回答：

- [ ] `anthropic-version` header 的作用是什么？不传会怎样？
- [ ] `stop_reason: "max_tokens"` 时，AgentLoop 当前的行为是什么？合理吗？
- [ ] 为什么 `content` 是数组而不是单个对象？（提示：想想并行工具调用）

---

## 下一课

[Session 04 · Context & Token Management →](/lessons/04-context) *(coming soon)*
