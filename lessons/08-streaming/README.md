# Session 08 · Streaming

> **Motto**: LLM 不是"思考完再回答"，它是"边想边说"。

---

## 概念先行

目前 Cori 的体验：

```
❯ 解释一下 Rust 的所有权系统
    （沉默 5 秒）
Rust 的所有权系统...（一次性出现全部）
```

Claude Code 的真实体验：

```
❯ 解释一下 Rust 的所有权系统
Rust 的所有权（边生成边出现）
系统是...（逐字打印）
```

区别不只是"好看"。Streaming 揭示了一个底层事实：**LLM 是逐 token 生成的**，每生成一个 token 都可以立刻发给客户端。非流式 API 是人为把所有 token 缓冲起来、生成结束后才一次性返回。

---

## SSE 协议

Claude API 使用 **SSE（Server-Sent Events）** 格式传输流式数据。

SSE 是基于 HTTP 的单向推送协议：
- Content-Type: `text/event-stream`
- 每个事件：`event: xxx\ndata: {...}\n\n`（以空行结束）
- 服务器持续推送，客户端逐行读取

一次完整的流式响应大致长这样：

```
event: message_start
data: {"type":"message_start","message":{"usage":{"input_tokens":25}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Rust"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" 的"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"所有权"}}

...（更多 text_delta）

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":128}}

event: message_stop
data: {"type":"message_stop"}
```

工具调用时，input JSON 也是流式的：

```
event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_xxx","name":"bash","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"comm"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"and\":\"ls\"}"}}
```

---

## 代码结构

```
cori-core/src/
├── loop_.rs         # 新增 StreamingLlm trait + run_turn_streaming()
└── streaming.rs     # ClaudeLlm 实现 StreamingLlm（本节练习）
```

```
src/main.rs          # CLI 改用 run_turn_streaming（Exercise 3）
```

---

## 练习 1 — 实现 `parse_sse_line()`

打开 `cori-core/src/streaming.rs`，找到 `parse_sse_line()` 函数。

它的职责：把 SSE 的一行文本转成 `StreamEvent`。

```rust
pub(crate) fn parse_sse_line(line: &str) -> Option<StreamEvent> {
    // 如果行以 "data: " 开头，取出 JSON，反序列化为 StreamEvent
    // 否则返回 None
}
```

提示：
```rust
let json_str = line.strip_prefix("data: ")?;
match serde_json::from_str(json_str) {
    Ok(event) => Some(event),
    Err(e) => { tracing::debug!("SSE parse error: {e}"); None }
}
```

**问题**：为什么 `event:` 行我们不解析，而是依赖 JSON 里的 `"type"` 字段？

---

## 练习 2 — 实现流式事件处理循环

在 `send_streaming()` 里找到标注 `// TODO: Exercise 2` 的注释块，取消注释并补全：

```rust
if let Some(event) = parse_sse_line(&line) {
    match event {
        StreamEvent::MessageStart { message } => {
            input_tokens = message.usage.input_tokens;
        }
        StreamEvent::ContentBlockStart { index, content_block } => {
            while blocks.len() <= index { blocks.push(None); }
            blocks[index] = Some(BlockState {
                kind: match content_block {
                    ContentBlock::Text { .. } =>
                        BlockKind::Text { accumulated: String::new() },
                    ContentBlock::ToolUse { id, name } =>
                        BlockKind::ToolUse { id, name, json_buf: String::new() },
                },
            });
        }
        StreamEvent::ContentBlockDelta { index, delta } => {
            if let Some(Some(block)) = blocks.get_mut(index) {
                match (&mut block.kind, delta) {
                    (BlockKind::Text { accumulated }, Delta::TextDelta { text }) => {
                        on_text(&text);           // ← 流式打印！
                        accumulated.push_str(&text);
                    }
                    (BlockKind::ToolUse { json_buf, .. }, Delta::InputJsonDelta { partial_json }) => {
                        json_buf.push_str(&partial_json);
                    }
                    _ => {}
                }
            }
        }
        StreamEvent::MessageDelta { delta, usage } => {
            stop_reason = delta.stop_reason;
            output_tokens = usage.output_tokens;
        }
        _ => {}
    }
}
```

记得删掉 TODO 块末尾的占位符：
```rust
let _ = &mut blocks;      // 删这行
let _ = &mut stop_reason; // 删这行
// ...
```

**问题**：为什么工具输入 JSON 需要拼接（`push_str(&partial_json)`），而不是直接用最后一个 delta？

---

## 练习 3 — 接入 CLI

打开 `src/main.rs`，找到标注 `// TODO: Exercise 3` 的注释：

```rust
// TODO: Exercise 3 — 把下面的 run_turn 换成 run_turn_streaming
//
// match agent.run_turn_streaming(&mut messages, on_text).await {
match agent.run_turn(&mut messages).await {
```

把注释掉的那行取消注释，把下面一行注释掉。

然后运行：

```bash
ANTHROPIC_API_KEY=sk-... cargo run
```

看到文字逐字出现，说明 Streaming 生效了。

---

## 验证

```bash
cargo test -p cori-core   # 测试仍然全通过
cargo build               # 编译成功
```

问自己：

- [ ] SSE 事件的 `event:` 行和 `data:` 行分别是什么作用？
- [ ] 为什么 `bytes_stream()` 拿到的 chunk 可能跨越多行？
- [ ] 工具调用的 input JSON 为什么也是流式的？有什么好处？
- [ ] 第一次调用用流式，工具回调后续用 `send()`，这个设计合理吗？

---

## 延伸思考

**实时 token 计数**：streaming 让你可以在生成过程中实时更新"已用 token"显示。Claude Code 状态栏的 token 数字就是这样实时更新的。

**中断生成**：流式还能支持"用户按 Ctrl+C 停止生成"——因为你持有 HTTP 连接，可以随时关掉它。非流式则必须等 API 响应完才能中断。

**工具调用的两段式流**：注意工具调用时，流会在 `content_block_stop` 后自动变为 `stop_reason: "tool_use"`。你的客户端可以在工具执行期间显示"正在执行工具..."，工具返回后再发起新的流式请求。

---

## 下一课

[Session 09 · Edit Tool →](/lessons/09-edit-tool) *(coming soon)*
