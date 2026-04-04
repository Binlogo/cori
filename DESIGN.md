# Cori — Design Document

> Cori is an AI coding agent framework built in Rust, designed to be the Rust-native reference implementation of the Claude Code agent architecture. It operates as a teaching tool, a production CLI, an HTTP API server, and an MCP (Model Context Protocol) server — all sharing a single unified query engine, tool system, and state management layer.

---

## Vision

**Cori's goal**: demonstrate that an AI coding agent isn't magic — it's a well-designed loop with pluggable parts. Every production concern (streaming, context limits, tool safety, multi-model support, protocol compatibility) can be expressed cleanly in a typed, composable Rust system.

The north star is *edge extensibility with a stable core*: new tools, providers, hooks, and deployment modes can be added without touching the central agent loop.

---

## Design Goals

### 1. Minimal, Stable Core

The agent loop (`cori-core`) is intentionally narrow. It only knows about:
- `Message` — the unit of communication
- `Llm` — something that responds to messages
- `ToolExecutor` — something that executes tool calls
- `AgentLoop` — the orchestration of the above

Everything else (concrete tools, concrete providers, UI, protocol) lives in a separate crate. This boundary must never blur.

### 2. Provider Abstraction

The `Llm` trait is the only interface between the agent loop and any language model. Claude, OpenAI-compatible endpoints, local models (Ollama), and mock implementations all satisfy the same trait. Switching providers requires zero changes to the loop, tools, or CLI.

Provider configuration (API key, base URL, model ID, timeout, retry policy) is expressed as a typed struct, not as environment variable lookups scattered throughout the code.

### 3. Tool System as Open Extension Point

Tools are the primary surface for extending Cori. The tool system is designed around three principles:

- **Discoverability**: every tool declares its JSON Schema, which is forwarded to the model verbatim
- **Composability**: tools can create subagent loops that themselves have tool registries
- **Safety**: the permission layer intercepts every tool call before execution

Built-in tools (bash, file I/O, edit, glob, grep, task management) live in `cori-tools`. User-defined tools, MCP tools, and dynamically loaded plugins all satisfy the same `Tool` trait.

### 4. Permission Model

Every tool execution passes through a permission gate before running. The permission model supports three levels:

- **Allow**: always execute without confirmation (e.g., read-only tools in trusted context)
- **Ask**: prompt the user before executing (default for write/execute tools in interactive mode)
- **Deny**: never execute (e.g., network tools in sandboxed mode)

Permission decisions are:
- Configurable per-tool via `cori.toml`
- Overridable per-session via CLI flags (`--allow-tools bash,read_file`)
- Mode-sensitive: headless/CI mode auto-denies interactive asks and requires explicit allow lists

### 5. Hook / Plugin System

Hooks are synchronous or asynchronous callbacks registered at well-defined lifecycle points:

```
PreToolCall  →  [tool execution]  →  PostToolCall
PreLlmCall   →  [LLM request]     →  PostLlmCall
OnTurnStart  →  [agent turn]      →  OnTurnEnd
OnSessionEnd
```

Hooks receive the event payload and can:
- **Observe**: log, audit, telemetry
- **Mutate**: modify tool input/output (e.g., scrub secrets)
- **Block**: abort the operation (e.g., permission enforcement)

This is the same pattern Claude Code uses for its settings-driven hooks — shell commands that run before/after tool execution.

### 6. Multiple Deployment Modes

Cori runs in four modes, all sharing the same core:

| Mode | Entry Point | Use Case |
|------|-------------|----------|
| **Interactive CLI** | `cori` binary | Developer daily use |
| **Headless / CI** | `cori --prompt "..."` | Scripts, CI pipelines |
| **HTTP Server** | `cori serve` | Web frontends, REST API |
| **MCP Server** | `cori mcp` | Claude Code integration, IDE plugins |

No mode has special knowledge of any other. The core doesn't know which mode it's running in.

### 7. Structured Configuration

Configuration follows a layered resolution order (later layers override earlier):

1. Compiled defaults
2. `~/.config/cori/config.toml` — user-level
3. `.cori.toml` in the project directory — project-level
4. Environment variables (`CORI_MODEL`, `ANTHROPIC_API_KEY`, etc.)
5. CLI flags

The configuration schema is typed (no stringly-typed lookups) and documented in full.

### 8. Context Management as First-Class Concern

Context windows are finite. The context manager is not an afterthought — it is a core component that every deployment mode respects. It supports:

- **Token counting**: accurate per-message token tracking
- **Truncation strategies**: configurable (keep-first, sliding window, summarize)
- **Compaction**: periodic summarization of conversation history via a secondary LLM call

### 9. Subagents and Task Parallelism

Subagents are first-class: spawning a subagent is as natural as calling a tool. Each subagent is an isolated `AgentLoop` with its own context, tool registry, and permission scope. The parent agent coordinates results.

Future: structured concurrency — multiple subagents running in parallel with `tokio::task`, their results merged by the parent.

### 10. Streaming First

All LLM communication is streaming by default. The non-streaming path is a special case (tests, simple scripts). The streaming interface (`StreamingLlm`) composes with hooks: every text token can be observed and the stream can be interrupted.

---

## Non-Goals

- **GUI / web app**: Cori is a backend. The `cori-web` crate is a course platform, not a production UI.
- **Model fine-tuning**: Cori sends prompts; it does not train models.
- **Multi-agent mesh**: Cori supports parent→child subagent spawning. Peer-to-peer agent graphs are out of scope.
- **Plugin hot-reloading**: Plugins are compiled in. Dynamic loading (`.so` / WASM) is future work.

---

## Key Constraints

- **No `unsafe`** in non-performance-critical paths.
- **No `unwrap()`** in library code — all errors propagate via `anyhow::Result` or typed errors.
- **No global state** — all configuration and state is threaded explicitly.
- **Tokio runtime required** — no sync-only API is planned; the async boundary is at the crate level.
- **Rust 1.75+** — relies on stable `async fn in trait`.

---

## Design Principles

### Error as Content
Tool execution failures return `Ok(error_message)` not `Err(...)`. The LLM decides whether to retry, explain the error, or take a different path. This prevents agent crashes and enables richer error recovery behavior.

### State via Disk
Persistent state (tasks, conversation history) lives on disk in deterministic locations. There is no in-memory cache that diverges from disk. Process restarts are safe.

### Transactional Edits
`edit_file` requires an exact, unique match of `old_string`. The operation is all-or-nothing. This prevents silent corruption from partial edits and ensures the LLM's mental model matches the actual file state.

### Context Isolation
A subagent is just a new `AgentLoop` with a fresh `Vec<Message>`. Isolation is achieved through message history boundaries, not OS-level sandboxing.

### Traits Over Concretions
The core crate (`cori-core`) exports traits. Concrete implementations live in provider/tool crates. This keeps the test surface minimal and enables compile-time verification of contracts.

---

## Roadmap

| Milestone | Description |
|-----------|-------------|
| **v0.2** | Workspace restructure: `cori-core`, `cori-tools`, `cori-provider`, `cori-cli` |
| **v0.3** | Permission model + hook system + `cori.toml` config |
| **v0.4** | MCP server (`cori-mcp`) — tools exposed over JSON-RPC stdio |
| **v0.5** | HTTP API server (`cori-server`) — REST + SSE streaming |
| **v0.6** | Parallel subagents + structured task graph with DAG cycle detection |
| **v1.0** | Production-grade: telemetry, audit log, rate limiting, multi-provider |
