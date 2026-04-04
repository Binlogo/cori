# Cori

> 从零构建一个 AI Coding Agent，理解 Claude Code 的工作原理。

Cori 是一门用 Rust 实现的交互式课程，同时也是一个生产级的 AI Agent 框架。
每个 Session 增量添加一个核心模块，九节课结束时你将得到一个可以真正运行的
AI Agent CLI，以及对 Claude Code 底层机制完整而清晰的认知。

**v0.2 起**，Cori 演化为一个多 crate workspace，复现了 Claude Code 的完整架构：
最小化核心（`cori-core`）、可插拔工具（`cori-tools`）、多 Provider 抽象（`cori-provider`）、
Hook/权限系统，以及三种部署模式（交互式 CLI、HTTP API、MCP Server）。

```
  ◆ Cori
  AI Agent CLI  ·  type /help for commands

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
| 08 | Streaming | SSE 流式输出、`StreamingLlm` trait |
| 09 | Edit Tool | `edit_file`、唯一性约束、unified diff |

---

## 快速开始

### 环境要求

- Rust 1.75+（需要 async fn in trait 稳定版）
- Anthropic API Key

### 运行交互式 CLI（增强版）

```bash
git clone https://github.com/Binlogo/cori
cd cori

export ANTHROPIC_API_KEY=sk-ant-...
cargo run -p cori-cli
```

### 运行原始 CLI（课程兼容版）

```bash
cargo run
```

### 以 MCP Server 模式运行

```bash
# 通过 stdio 提供 MCP 协议，可被 Claude Code / IDE 插件使用
cargo run -p cori-mcp
```

将以下配置添加到 Claude Code 的 MCP 配置文件中：

```json
{
  "mcpServers": {
    "cori": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "/path/to/cori/Cargo.toml", "-p", "cori-mcp", "--quiet"],
      "env": { "ANTHROPIC_API_KEY": "sk-ant-..." }
    }
  }
}
```

### 以 HTTP API Server 模式运行

```bash
# 默认监听 :3001
CORI_PORT=3001 cargo run -p cori-server

# 单次请求（非流式）
curl -X POST http://localhost:3001/v1/chat \
  -H 'Content-Type: application/json' \
  -d '{"message": "列出当前目录的文件"}'

# 流式请求（SSE）
curl -N http://localhost:3001/v1/chat/stream \
  -X POST -H 'Content-Type: application/json' \
  -d '{"message": "分析项目结构"}'

# 查看已注册的工具
curl http://localhost:3001/v1/tools
```

### 无头模式（CI / 脚本）

```bash
# 通过 --prompt 参数
cargo run -p cori-cli -- --prompt "运行测试并汇总结果"

# 通过环境变量
CORI_PROMPT="检查 Cargo.toml 的依赖版本" cargo run -p cori-cli
```

### 运行课程平台（Web）

```bash
cargo run -p cori-web
# 访问 http://localhost:3000
```

### 运行测试

```bash
cargo test --workspace
```

---

## 项目结构

```
cori/
├── cori-core/          # 最小化内核：trait、AgentLoop、Context、Hook、Permission、Config
├── cori-tools/         # 内置工具集：bash、fs、edit、glob、grep、task、subagent
├── cori-provider/      # LLM Provider：Claude、MockProvider、OpenAI-compat
├── cori-cli/           # 增强版交互式 CLI（v0.2 新增）
├── cori-mcp/           # MCP Server（JSON-RPC over stdio）
├── cori-server/        # HTTP API Server（REST + SSE）
├── cori-web/           # 课程平台（Axum）
├── src/main.rs         # 原始 CLI（课程兼容，保留）
├── cori.toml.example   # 配置文件模板
├── DESIGN.md           # 设计目标与原则
├── ARCHITECTURE.md     # 架构设计（crate 边界、数据流）
└── lessons/            # 各 Session 教学文档
```

### 依赖关系

```
cori-core  ←── cori-tools
           ←── cori-provider
                    ↓
              cori-cli / cori-mcp / cori-server
```

---

## 环境变量

| 变量 | 必填 | 说明 |
|------|------|------|
| `ANTHROPIC_API_KEY` | ✅ | Anthropic API Key |
| `ANTHROPIC_BASE_URL` | — | 自定义端点，兼容第三方 Anthropic 格式接口 |
| `ANTHROPIC_MODEL` | — | 默认 `claude-opus-4-6` |
| `CORI_PORT` | — | HTTP Server 端口，默认 `3001` |
| `CORI_PROMPT` | — | 无头模式单次 prompt |
| `CORI_TASKS_DIR` | — | 任务文件存储目录，默认 `.tasks` |
| `RUST_LOG` | — | 日志级别，如 `debug`、`warn` |

---

## 配置文件

将 `cori.toml.example` 复制为 `.cori.toml` 进行项目级配置：

```bash
cp cori.toml.example .cori.toml
```

---

## 设计原则

**可替换的后端**：`Llm`、`ToolExecutor`、`Tool` 都是 trait，测试时用 `MockProvider` 替换，
生产时换成 `ClaudeLlm`，AgentLoop 本身不需要改动。

**Hook / 插件系统**：在 `PreToolCall`、`PostToolCall`、`PreLlmCall` 等生命周期节点
注册 Hook，可以做日志、审计、访问控制，而不改动核心循环。

**权限模型**：每次工具调用都经过 `PermissionGate`，支持 Allow / Ask / Deny 三级，
可按工具名配置，在 CI 模式下 Ask 自动降级为 Deny。

**错误即内容**：工具执行失败（如 bash 非零退出、文件不存在）返回 `Ok(error_message)`
而非 `Err`，让 Claude 自己决定是否重试或换策略，而不是让整个 Agent 崩溃。

**上下文即隔离**：Subagent 就是一个新的 `AgentLoop` + 新的 `Vec<Message>`，
没有 OS 级别的沙箱，隔离来自全新的消息上下文。

**多部署模式**：同一个核心库，三种部署形态（CLI / HTTP / MCP），
共享工具注册表、权限门控、Hook 系统。

---

## 致谢

本项目的课程结构和核心思路受到 [learn-claude-code](https://github.com/shareAI-lab/learn-claude-code) 的启发，感谢其对 Claude Code 工作原理的系统性梳理。

架构设计参考了 [claw-code](https://github.com/ultraworkers/claw-code) 对 Claude Code harness 工程的分析。

---

## License

MIT
