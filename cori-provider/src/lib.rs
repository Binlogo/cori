//! LLM provider implementations for the Cori agent framework.
//!
//! This crate provides concrete LLM backend implementations that implement
//! the `Llm` and `StreamingLlm` traits defined in `cori-core`.
//!
//! # Providers
//!
//! - [`claude::ClaudeProvider`] — Anthropic Claude API (with streaming + retry)
//! - [`mock::MockProvider`] — Scripted/preset responses for testing
//! - [`openai_compat::OpenAiCompatProvider`] — OpenAI-compatible endpoints

pub mod claude;
pub mod mock;
pub mod openai_compat;

pub use claude::ClaudeProvider;
pub use mock::MockProvider;
pub use openai_compat::OpenAiCompatProvider;
