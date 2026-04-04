/// Cori Core — the agent kernel.
///
/// Each module corresponds to a course session (01–09), plus new architectural
/// additions (hooks, permission, config) introduced in v0.2.
pub mod lesson;

// ── Session 01: Agent Loop ────────────────────────────────────────────────────
pub mod loop_;
mod loop_test;
pub mod types;

// ── Session 02: Tool Dispatch ─────────────────────────────────────────────────
pub mod tools;

// ── Session 03: Real API ──────────────────────────────────────────────────────
pub mod claude;

// ── Session 04: Context Management ───────────────────────────────────────────
pub mod context;

// ── Session 05: Planning & Tasks ─────────────────────────────────────────────
pub mod planner;

// ── Session 08: Streaming ─────────────────────────────────────────────────────
pub mod streaming;

// ── v0.2: Architectural additions ────────────────────────────────────────────

/// Hook / plugin system — observe and intercept agent lifecycle events.
pub mod hooks;

/// Permission model — control which tools can run and when.
pub mod permission;

/// Typed configuration — provider, context, and agent settings.
pub mod config;
