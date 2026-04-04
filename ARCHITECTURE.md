# Cori — Architecture Document

> This document describes the physical and logical structure of the Cori codebase, the boundaries between crates, data flows, and the rationale for each architectural decision.

---

## Workspace Layout

```
cori/
├── cori-core/          # Kernel: traits, loop, context, types
├── cori-tools/         # Built-in tool implementations
├── cori-provider/      # LLM provider backends (Claude, mock, OpenAI-compat)
├── cori-cli/           # Interactive CLI + headless mode
├── cori-server/        # HTTP API server (REST + SSE)
├── cori-mcp/           # MCP (Model Context Protocol) server
├── cori-web/           # Course platform (Axum, Markdown rendering)
├── lessons/            # Educational content per session
├── DESIGN.md           # Design goals and principles
└── ARCHITECTURE.md     # This document
```

### Dependency Graph

```
                    ┌─────────────┐
                    │  cori-core  │  ← minimal kernel, no I/O
                    └──────┬──────┘
              ┌────────────┼────────────┐
              │            │            │
        ┌─────▼─────┐ ┌────▼────┐ ┌────▼──────┐
        │cori-tools │ │cori-    │ │  (future) │
        │           │ │provider │ │           │
        └─────┬─────┘ └────┬────┘ └───────────┘
              │            │
              └─────┬──────┘
                    │
        ┌───────────┼───────────┐
        │           │           │
  ┌─────▼─────┐ ┌───▼────┐ ┌───▼────┐
  │ cori-cli  │ │cori-   │ │cori-   │
  │           │ │server  │ │mcp     │
  └───────────┘ └────────┘ └────────┘
```

**Rule**: dependencies only flow downward. No crate may depend on a crate above it in this graph. `cori-core` has no dependency on any other `cori-*` crate.

---

## Crate Responsibilities

### `cori-core` — The Kernel

**Purpose**: define the contracts and orchestrate the agent loop. No concrete I/O.

**What it owns**:
- `types.rs` — `Message`, `Role`, `Content`, `ToolUse`, `ToolResult`, `Usage`
- `traits.rs` — `Llm`, `StreamingLlm`, `ToolExecutor`, `Tool`
- `loop_.rs` — `AgentLoop`, `run()`, `run_turn()`, `run_streaming()`
- `context.rs` — `ContextManager`, token counting, truncation strategies
- `hooks.rs` — `Hook` trait, `HookRegistry`, lifecycle events
- `permission.rs` — `PermissionGate`, `PermissionPolicy` (Allow/Ask/Deny)
- `config.rs` — `CoriConfig`, `ProviderConfig`, `ToolConfig` (typed structs)
- `error.rs` — `CoriError` typed error enum

**What it does NOT own**:
- Any HTTP client
- Any file system access
- Any terminal/UI code
- Any concrete tool implementations
- Any concrete LLM implementations

**Key traits**:

```rust
/// Synchronous (non-streaming) LLM call.
#[async_trait]
pub trait Llm: Send + Sync {
    async fn send(&self, messages: &[Message], tools: &[ToolSchema])
        -> Result<LlmResponse>;
}

/// Streaming LLM call — yields text deltas via callback.
#[async_trait]
pub trait StreamingLlm: Llm {
    async fn send_streaming<F>(
        &self,
        messages: &[Message],
        tools: &[ToolSchema],
        on_delta: F,
    ) -> Result<LlmResponse>
    where
        F: Fn(&str) + Send + Sync;
}

/// Execute a single tool call, return its textual result.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, call: &ToolUse) -> Result<ToolResult>;
}

/// A single tool — schema declaration + execution.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn schema(&self) -> ToolSchema;
    async fn execute(&self, input: &serde_json::Value) -> Result<String>;
}

/// Lifecycle hook — observe or intercept agent events.
#[async_trait]
pub trait Hook: Send + Sync {
    async fn on_event(&self, event: &HookEvent) -> HookAction;
}
```

**`AgentLoop` signature**:

```rust
pub struct AgentLoop {
    provider: Arc<dyn Llm>,
    executor: Arc<dyn ToolExecutor>,
    context: ContextManager,
    hooks: HookRegistry,
    permissions: PermissionGate,
    config: AgentConfig,
}

impl AgentLoop {
    pub async fn run(&mut self, input: &str) -> Result<String>;
    pub async fn run_streaming<F>(&mut self, input: &str, on_delta: F) -> Result<String>
    where F: Fn(&str) + Send + Sync;
    pub async fn run_turn(&mut self, messages: &mut Vec<Message>) -> Result<TurnResult>;
}
```

---

### `cori-tools` — Built-in Tools

**Purpose**: all production tool implementations. Each tool is a standalone struct implementing `Tool`.

**Module structure**:

```
cori-tools/src/
├── lib.rs          # pub use, register_defaults()
├── bash.rs         # BashTool — shell command execution
├── fs/
│   ├── mod.rs
│   ├── read.rs     # ReadFileTool
│   ├── write.rs    # WriteFileTool
│   ├── edit.rs     # EditFileTool (unique match, unified diff)
│   ├── glob.rs     # GlobTool
│   └── grep.rs     # GrepTool
├── task/
│   ├── mod.rs
│   ├── create.rs   # TaskCreateTool
│   ├── list.rs     # TaskListTool
│   ├── get.rs      # TaskGetTool
│   └── update.rs   # TaskUpdateTool
└── subagent.rs     # SubagentTool — spawn isolated child agent
```

**`register_defaults(registry: &mut ToolRegistry)`** is the primary public API — it wires all built-in tools into a registry with their default configurations.

**Tool design rules**:
- All file paths are validated before I/O
- All shell commands run with timeout enforcement
- Results are UTF-8 strings (binary file reads return an error message, not bytes)
- No tool holds mutable global state

---

### `cori-provider` — LLM Backends

**Purpose**: concrete `Llm` and `StreamingLlm` implementations for each supported provider.

**Module structure**:

```
cori-provider/src/
├── lib.rs
├── claude.rs       # ClaudeProvider — Anthropic Messages API
├── mock.rs         # MockProvider — scripted responses for tests
└── openai_compat.rs # OpenAICompatProvider — any OpenAI-format endpoint
```

**`ClaudeProvider`**:
- Reads `ProviderConfig` (api_key, base_url, model, timeout)
- Implements both `Llm` and `StreamingLlm`
- SSE parsing is internal to this module
- Handles rate limit retry with exponential backoff

**`MockProvider`**:
- Takes a `Vec<LlmResponse>` script at construction time
- Replays responses in order; panics (in tests) if script exhausted
- Used in all unit tests; no network dependency

**Provider selection** is done at startup based on `ProviderConfig.kind` (`claude` | `openai_compat` | `mock`).

---

### `cori-cli` — Interactive CLI & Headless Mode

**Purpose**: the user-facing binary. Rich interactive REPL for daily use; headless mode for scripts and CI.

**Binary entry point**: `cori` (or `cargo run -p cori-cli`)

**Module structure**:

```
cori-cli/src/
├── main.rs         # Entry point, mode dispatch
├── interactive.rs  # REPL with rustyline, colored output, slash commands
├── headless.rs     # Single-shot prompt mode (--prompt flag)
├── commands/
│   ├── mod.rs
│   ├── help.rs     # /help
│   ├── clear.rs    # /clear — reset conversation
│   ├── tools.rs    # /tools — list registered tools
│   ├── model.rs    # /model — show/switch model
│   ├── config.rs   # /config — show current config
│   └── quit.rs     # /quit, /exit
└── display.rs      # Rendering: tool calls, diffs, task lists, progress
```

**Slash command system**: commands start with `/`. The CLI intercepts them before sending to the agent. Each command is a struct implementing a `Command` trait with `name()`, `help()`, and `run()`.

**Rendering**:
- Tool calls: `● Tool: {name}` with spinner during execution, `○ Done` on completion
- Tool results: collapsible, abbreviated for long output
- Diff output: syntax-highlighted unified diff
- Task lists: tree view with status symbols
- Streaming text: printed token-by-token without buffering

**Permission prompts** (for Ask-level tools):
```
  ┌─ Permission Required ──────────────────────┐
  │ Tool:  bash                                │
  │ Input: rm -rf ./build                      │
  │                                            │
  │  [y] Allow   [n] Deny   [a] Always allow  │
  └────────────────────────────────────────────┘
```

---

### `cori-server` — HTTP API Server

**Purpose**: expose the agent as an HTTP service for web frontends, programmatic API consumers, and LLM orchestration platforms.

**Framework**: Axum

**Endpoints**:

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/chat` | Single-turn agent invocation, returns JSON |
| `POST` | `/v1/chat/stream` | Streaming agent run, returns SSE |
| `GET`  | `/v1/tools` | List registered tools and their schemas |
| `GET`  | `/v1/health` | Health check |
| `GET`  | `/v1/config` | Current configuration (redacted secrets) |

**SSE event format** (compatible with Claude API's streaming format):

```
event: text_delta
data: {"text": "Hello"}

event: tool_call
data: {"name": "bash", "input": {"command": "ls"}}

event: tool_result
data: {"name": "bash", "output": "file1.txt\nfile2.txt"}

event: done
data: {"usage": {"input_tokens": 142, "output_tokens": 87}}
```

**Session management**: each POST creates a fresh `AgentLoop`. Stateful multi-turn sessions are tracked by a `session_id` cookie/header with session state stored in memory (bounded LRU cache).

---

### `cori-mcp` — MCP Server

**Purpose**: implement the [Model Context Protocol](https://spec.modelcontextprotocol.io) so that Cori's tools can be consumed by Claude Code, Claude Desktop, and any MCP-compatible client.

**Transport**: JSON-RPC 2.0 over `stdio` (standard MCP transport).

**Capabilities exposed**:
- `tools/list` — all registered `cori-tools` tools, with their schemas
- `tools/call` — execute a named tool with given arguments
- `prompts/list` — (future) registered prompt templates
- `resources/list` — (future) file resources

**Protocol flow**:

```
MCP Client (e.g. Claude Code)
        │
        │  JSON-RPC over stdio
        ▼
  cori-mcp server
        │
        │  calls Tool trait
        ▼
  cori-tools ToolRegistry
        │
        │  returns tool result
        ▼
  cori-mcp server
        │
        │  JSON-RPC response
        ▼
MCP Client
```

**Mode**: `cori mcp` launches the MCP server. No agent loop runs — the MCP server is a pure tool bridge.

---

### `cori-web` — Course Platform

**Purpose**: render the lesson curriculum as a web application. Not a production concern — kept simple.

**Framework**: Axum with `pulldown-cmark` for Markdown rendering.

**Routes**:
- `GET /` — lesson index
- `GET /lessons/{id}` — render `lessons/{id}/README.md`

**No JavaScript.** Static HTML + inline CSS only.

---

## Cross-Cutting Concerns

### Configuration Resolution

```
CoriConfig::load() resolution order:
  1. Compiled defaults (Config::default())
  2. ~/.config/cori/config.toml
  3. ./.cori.toml
  4. Environment variables (ANTHROPIC_API_KEY, CORI_MODEL, etc.)
  5. CLI flags (--model, --allow-tools, etc.)
```

Configuration is loaded once at startup and passed as `Arc<CoriConfig>` to all components. No runtime mutation.

### Logging & Tracing

All crates use `tracing` for structured logging. Log levels:
- `ERROR`: unrecoverable failures
- `WARN`: unexpected but handled conditions
- `INFO`: session start/end, tool executions, token usage
- `DEBUG`: LLM request/response payloads (redacted by default)
- `TRACE`: byte-level SSE parsing, internal state transitions

The CLI initializes `tracing-subscriber` with `RUST_LOG` env filter. The MCP server logs to stderr (stdout is reserved for JSON-RPC).

### Error Handling

- Library code: `anyhow::Result<T>` for propagation, typed `CoriError` for public API errors
- Tool execution: always `Ok(error_message)` — errors become content for the LLM
- CLI: top-level `main()` pretty-prints `anyhow::Error` and exits non-zero
- Server: errors map to HTTP status codes via `IntoResponse` implementations

### Testing Strategy

| Layer | Strategy |
|-------|----------|
| `cori-core` | Pure unit tests with `MockProvider` and `EchoExecutor` |
| `cori-tools` | Integration tests using `tempfile` for file I/O |
| `cori-provider` | Unit tests for SSE parsing; integration tests with recorded fixtures |
| `cori-cli` | Snapshot tests for output rendering |
| `cori-server` | HTTP integration tests with `axum-test` |
| `cori-mcp` | JSON-RPC protocol tests with stdio pipe |

---

## Data Flow: Single Agent Turn

```
User Input (string)
       │
       ▼
  AgentLoop::run()
       │
       ├─ HookRegistry::on_turn_start()
       │
       ├─ ContextManager::maybe_truncate()
       │
       ├─ messages.push(Message::user(input))
       │
       ▼
  [loop]
       │
       ├─ HookRegistry::pre_llm_call()
       │
       ├─ provider.send(messages, tool_schemas) → LlmResponse
       │
       ├─ HookRegistry::post_llm_call()
       │
       ├─ if stop_reason == "end_turn":
       │     return response.text
       │
       └─ if stop_reason == "tool_use":
             for each tool_call in response.tool_calls:
               │
               ├─ PermissionGate::check(tool_name, input)
               │     → Allow: continue
               │     → Deny: return error content
               │     → Ask: prompt user (CLI only) → Allow or Deny
               │
               ├─ HookRegistry::pre_tool_call(tool_name, input)
               │
               ├─ executor.execute(tool_call) → ToolResult
               │
               └─ HookRegistry::post_tool_call(tool_name, result)

             messages.push(Message::tool_results(results))
             continue loop
```

---

## Task Graph

Tasks form a directed acyclic graph (DAG) where edges represent `blocks`/`blocked_by` relationships.

```
TaskGraph
├── nodes: HashMap<TaskId, Task>
└── persistence: .tasks/{id}.json (one file per task)

Task {
    id: TaskId,
    subject: String,
    description: String,
    status: TaskStatus,        // Pending | InProgress | Completed | Deleted
    owner: Option<String>,
    blocks: Vec<TaskId>,       // tasks this task must complete before
    blocked_by: Vec<TaskId>,   // tasks that must complete before this
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
```

**Invariants**:
- Edges are bidirectional: `A blocks B` ↔ `B blocked_by A`
- DAG cycle detection runs on every `add_blocked_by` / `add_blocks` mutation
- Disk is the single source of truth; no in-memory cache

---

## Subagent Architecture

```
Parent AgentLoop
       │
       │  SubagentTool.execute()
       ▼
Child AgentLoop (new instance)
├── provider: same Arc<dyn Llm>    ← shared, no cost
├── executor: restricted ToolRegistry (subset of parent's tools)
├── context: fresh Vec<Message>    ← isolated
├── hooks: inherited from parent   ← or overridden
└── permissions: scoped (tighter than parent)
       │
       │  returns final text
       ▼
Parent continues with result as tool output
```

Key properties:
- No shared mutable state between parent and child
- Provider is shared (same API credentials, same model)
- Tool access is scoped: subagents by default only get read-only tools
- Context isolation means subagent history doesn't pollute parent context

---

## MCP Integration Architecture

```
Claude Code / IDE Plugin
       │
       │  MCP (JSON-RPC stdio)
       ▼
cori-mcp server
├── request router
├── tool registry (shared with cori-tools)
└── response serializer
```

When running as MCP server:
- `cori mcp` reads from stdin, writes to stdout
- stderr is used for logging
- The full `cori-tools` registry is exposed
- No `AgentLoop` is involved — pure tool bridge
- `cori-core` types are used for tool execution

---

## File Layout (Target State)

```
cori/
├── Cargo.toml                    # workspace root
├── DESIGN.md
├── ARCHITECTURE.md
├── cori.toml.example             # documented config template
│
├── cori-core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs              # Message, Role, Content, ToolUse, ToolResult
│       ├── traits.rs             # Llm, StreamingLlm, ToolExecutor, Tool, Hook
│       ├── loop_.rs              # AgentLoop
│       ├── context.rs            # ContextManager
│       ├── hooks.rs              # HookRegistry, HookEvent, HookAction
│       ├── permission.rs         # PermissionGate, PermissionPolicy
│       ├── config.rs             # CoriConfig, ProviderConfig
│       ├── error.rs              # CoriError
│       └── registry.rs           # ToolRegistry
│
├── cori-tools/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── bash.rs
│       ├── fs/
│       │   ├── mod.rs
│       │   ├── read.rs
│       │   ├── write.rs
│       │   ├── edit.rs
│       │   ├── glob.rs
│       │   └── grep.rs
│       ├── task/
│       │   ├── mod.rs
│       │   ├── graph.rs
│       │   └── tools.rs
│       └── subagent.rs
│
├── cori-provider/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── claude.rs
│       ├── streaming.rs
│       ├── mock.rs
│       └── openai_compat.rs
│
├── cori-cli/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── interactive.rs
│       ├── headless.rs
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── help.rs
│       │   ├── clear.rs
│       │   ├── tools.rs
│       │   ├── model.rs
│       │   └── config.rs
│       └── display.rs
│
├── cori-server/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── routes/
│       │   ├── chat.rs
│       │   ├── stream.rs
│       │   └── tools.rs
│       └── session.rs
│
├── cori-mcp/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── server.rs
│       ├── protocol.rs           # JSON-RPC types
│       └── handlers.rs
│
├── cori-web/
│   ├── Cargo.toml
│   └── src/main.rs
│
└── lessons/
    ├── 01-agent-loop/
    ├── 02-tool-dispatch/
    ├── 03-real-api/
    ├── 04-context/
    ├── 05-planning/
    ├── 06-subagents/
    ├── 07-file-tools/
    ├── 08-streaming/
    └── 09-edit-tool/
```
