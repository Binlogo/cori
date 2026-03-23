# Cori

> 从零构建一个 AI Coding Agent，理解 Claude Code 的工作原理。

Cori 是一门用 Rust 实现的交互式课程。每个 Session 增量添加一个核心模块，
七节课结束时，你将得到一个可以真正运行的 AI Agent CLI，以及对 Claude Code
底层机制完整而清晰的认知。

```
  ◆ Cori
  AI Agent CLI  ·  /help 查看命令

❯ 分析 src/ 目录下有哪些公开 trait，每个定义在哪里

  ● Tool: glob
    ○ Running: cori-core/src/**/*.rs
    cori-core/src/claude.rs
    cori-core/src/loop_.rs
    ...

  ● Tool: grep
    ○ Running: pub trait
    cori-core/src/loop_.rs:54: pub trait Llm {
    cori-core/src/loop_.rs:61: pub trait ToolExecutor {
    cori-core/src/tools/mod.rs:32: pub trait Tool: Send + Sync {

────────────────────────────────────────────────
项目定义了三个核心公开 trait：`Llm`、`ToolExecutor`、`Tool`……
```

---

## 核心概念

Claude Code 的本质是一个循环，没有更多：

```
┌─────────────────────────────────────────────┐
│                 Agent Loop                  │
│                                             │
│  ┌──────────┐   tool_call   ┌────────────┐  │
│  │  Claude  │ ────────────▶ │    Tool    │  │
│  │  (LLM)   │ ◀──────────── │  Executor  │  │
│  └──────────┘  tool_result  └────────────┘  │
│       │                                     │
│       │ stop_reason = "end_turn"            │
│       ▼                                     │
│    [ Done ]                                 │
└─────────────────────────────────────────────┘
```

理解了这个循环，就理解了所有 AI Agent 的共同骨架。

---

## 课程大纲

| Session | 主题 | 核心收获 |
|---------|------|---------|
| 01 | Agent Loop | 循环骨架、消息格式、MockLlm、max_turns 安全阀 |
| 02 | Tool Dispatch | `Tool` trait、`ToolRegistry`、`BashTool` |
| 03 | Real API | `ClaudeLlm`、HTTP 请求 / 响应解析、第三方兼容接口 |
| 04 | Context Management | Token 追踪、超限截断策略 |
| 05 | Planning & Tasks | `TaskList` 持久化、`TodoReadTool` / `TodoWriteTool` |
| 06 | Subagents | `spawn_subagent`、上下文隔离、`async` Tool trait |
| 07 | File System Tools | `read_file`、`write_file`、`glob`、`grep`；交互式 CLI |

---

## 快速开始

### 环境要求

- Rust 1.75+（需要 async fn in trait 稳定版）
- Anthropic API Key

### 运行交互式 CLI

```bash
git clone https://github.com/Binlogo/cori
cd cori

export ANTHROPIC_API_KEY=sk-ant-...
cargo run
```

### 运行课程平台（Web）

```bash
cargo run -p cori-web
# 访问 http://localhost:3000
```

### 运行 Session 示例

```bash
# 完整演示：glob + grep + read_file 探索项目结构
ANTHROPIC_API_KEY=sk-ant-... cargo run -p cori-core --example hello
```

### 运行测试

```bash
cargo test -p cori-core   # 14 个单元测试
```

---

## 项目结构

```
cori/
├── src/
│   └── main.rs            # 交互式 CLI（Session 07）
│
├── cori-core/             # Agent 核心库
│   └── src/
│       ├── loop_.rs       # AgentLoop：run() / run_turn()
│       ├── types.rs       # Message / Role / Content
│       ├── claude.rs      # ClaudeLlm（真实 API）
│       ├── context.rs     # ContextManager（token 截断）
│       ├── planner/       # TaskList（JSON 持久化）
│       └── tools/
│           ├── mod.rs     # Tool trait / ToolRegistry
│           ├── bash.rs    # BashTool
│           ├── todo.rs    # TodoReadTool / TodoWriteTool
│           ├── subagent.rs# SubagentTool
│           └── fs.rs      # ReadFileTool / WriteFileTool / GlobTool / GrepTool
│
├── cori-web/              # 课程平台（Axum）
│   └── src/main.rs        # Markdown 渲染，http://localhost:3000
│
└── lessons/               # 各 Session 教学文档
    ├── 01-agent-loop/
    ├── 02-tool-dispatch/
    ├── 03-real-api/
    ├── 04-context/
    ├── 05-planning/
    ├── 06-subagents/
    └── 07-file-tools/
```

---

## 环境变量

| 变量 | 必填 | 说明 |
|------|------|------|
| `ANTHROPIC_API_KEY` | ✅ | Anthropic API Key |
| `ANTHROPIC_BASE_URL` | — | 自定义端点，兼容第三方 Anthropic 格式接口 |
| `ANTHROPIC_MODEL` | — | 默认 `claude-opus-4-6` |

---

## 设计原则

**可替换的后端**：`Llm` 和 `ToolExecutor` 都是 trait，测试时用 `MockLlm` 替换，
生产时换成 `ClaudeLlm`，AgentLoop 本身不需要改动。

**工具即接口**：每个工具实现三个方法：`name()`、`schema()`、`execute()`。
`schema()` 告诉 Claude 工具存在，`execute()` 实际运行。新增工具只需实现这个 trait
并注册，Loop 不感知具体工具。

**错误即内容**：工具执行失败（如 bash 非零退出、文件不存在）返回 `Ok(error_message)`
而非 `Err`，让 Claude 自己决定是否重试或换策略，而不是让整个 Agent 崩溃。

**上下文即隔离**：Subagent 就是一个新的 `AgentLoop` + 新的 `Vec<Message>`，
没有 OS 级别的沙箱，隔离来自全新的消息上下文。

---

## License

MIT
