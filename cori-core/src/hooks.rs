/// Hook / Plugin System
///
/// Hooks are async callbacks registered at well-defined lifecycle points in the agent.
/// They can observe, mutate, or block agent events.
///
/// Lifecycle points:
///   PreToolCall  →  [tool execution]  →  PostToolCall
///   PreLlmCall   →  [LLM request]     →  PostLlmCall
///   OnTurnStart  →  [agent turn]      →  OnTurnEnd
///   OnSessionEnd

use crate::types::{Message, ToolResult, ToolUse};

// ── HookEvent ─────────────────────────────────────────────────────────────────

/// An event that occurred during agent execution.
#[derive(Debug, Clone)]
pub enum HookEvent {
    /// Before sending messages to the LLM.
    PreLlmCall { messages: Vec<Message> },
    /// After receiving a response from the LLM.
    PostLlmCall { stop_reason: String, input_tokens: u32, output_tokens: u32 },
    /// Before executing a tool call.
    PreToolCall { call: ToolUse },
    /// After a tool call completes.
    PostToolCall { call: ToolUse, result: ToolResult },
    /// At the start of a new agent turn (user message received).
    OnTurnStart { turn: usize },
    /// At the end of an agent turn.
    OnTurnEnd { turn: usize, response: String },
    /// When the agent session ends.
    OnSessionEnd,
}

// ── HookAction ────────────────────────────────────────────────────────────────

/// What a hook wants to do with the event.
#[derive(Debug, Clone)]
pub enum HookAction {
    /// Continue processing normally.
    Continue,
    /// Block the operation and return this error message as tool content.
    Block(String),
}

impl HookAction {
    pub fn is_block(&self) -> bool {
        matches!(self, HookAction::Block(_))
    }
}

// ── Hook trait ────────────────────────────────────────────────────────────────

/// A hook that can observe or intercept agent lifecycle events.
#[async_trait::async_trait]
pub trait Hook: Send + Sync {
    fn name(&self) -> &str;
    async fn on_event(&self, event: &HookEvent) -> HookAction;
}

// ── HookRegistry ──────────────────────────────────────────────────────────────

/// Registry of all active hooks. Hooks are called in registration order.
#[derive(Default)]
pub struct HookRegistry {
    hooks: Vec<Box<dyn Hook>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a hook. Hooks are called in registration order.
    pub fn register(&mut self, hook: impl Hook + 'static) {
        self.hooks.push(Box::new(hook));
    }

    /// Fire an event to all hooks. Returns the first Block action, or Continue.
    pub async fn fire(&self, event: &HookEvent) -> HookAction {
        for hook in &self.hooks {
            let action = hook.on_event(event).await;
            if action.is_block() {
                tracing::debug!(hook = %hook.name(), "hook blocked event");
                return action;
            }
        }
        HookAction::Continue
    }

    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }
}

// ── Built-in: LoggingHook ─────────────────────────────────────────────────────

/// A hook that logs all events at INFO level. Useful for debugging.
pub struct LoggingHook;

#[async_trait::async_trait]
impl Hook for LoggingHook {
    fn name(&self) -> &str {
        "logging"
    }

    async fn on_event(&self, event: &HookEvent) -> HookAction {
        match event {
            HookEvent::PreLlmCall { messages } => {
                tracing::info!(messages = messages.len(), "→ LLM call");
            }
            HookEvent::PostLlmCall { stop_reason, input_tokens, output_tokens } => {
                tracing::info!(
                    stop_reason,
                    input_tokens,
                    output_tokens,
                    "← LLM response"
                );
            }
            HookEvent::PreToolCall { call } => {
                tracing::info!(tool = %call.name, "→ tool call");
            }
            HookEvent::PostToolCall { call, .. } => {
                tracing::info!(tool = %call.name, "← tool done");
            }
            HookEvent::OnTurnStart { turn } => {
                tracing::info!(turn, "turn start");
            }
            HookEvent::OnTurnEnd { turn, .. } => {
                tracing::info!(turn, "turn end");
            }
            HookEvent::OnSessionEnd => {
                tracing::info!("session end");
            }
        }
        HookAction::Continue
    }
}
