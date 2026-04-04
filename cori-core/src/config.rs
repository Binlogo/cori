/// Typed configuration for the Cori agent system.
///
/// Configuration follows a layered resolution order (later layers override earlier):
///   1. Compiled defaults (CoriConfig::default())
///   2. ~/.config/cori/config.toml — user-level
///   3. .cori.toml in the project directory — project-level
///   4. Environment variables (ANTHROPIC_API_KEY, CORI_MODEL, etc.)
///   5. CLI flags (--model, --allow-tools, etc.)
///
/// This module only defines the types. Resolution logic lives in cori-cli.

// ── ProviderKind ──────────────────────────────────────────────────────────────

/// The LLM provider backend to use.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Claude,
    OpenAiCompat,
    Mock,
}

impl Default for ProviderKind {
    fn default() -> Self {
        ProviderKind::Claude
    }
}

// ── ProviderConfig ────────────────────────────────────────────────────────────

/// Configuration for the LLM provider.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    /// API authentication key.
    pub api_key: String,
    /// Base URL for the API (without trailing slash).
    /// Default: https://api.anthropic.com
    pub base_url: String,
    /// Model identifier.
    /// Default: claude-opus-4-6
    pub model: String,
    /// Maximum tokens to generate per response.
    pub max_tokens: u32,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            kind: ProviderKind::Claude,
            api_key: String::new(),
            base_url: "https://api.anthropic.com".into(),
            model: "claude-opus-4-6".into(),
            max_tokens: 8192,
            timeout_secs: 120,
        }
    }
}

impl ProviderConfig {
    /// Build from environment variables.
    ///
    /// Reads:
    ///   ANTHROPIC_API_KEY  (required)
    ///   ANTHROPIC_BASE_URL (optional, default: https://api.anthropic.com)
    ///   ANTHROPIC_MODEL    (optional, default: claude-opus-4-6)
    pub fn from_env() -> Result<Self, anyhow::Error> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY is not set"))?;
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".into());
        let model = std::env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-opus-4-6".into());

        Ok(Self {
            api_key,
            base_url,
            model,
            ..Default::default()
        })
    }

    /// Returns the full Messages API URL.
    pub fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url)
    }
}

// ── ContextConfig ─────────────────────────────────────────────────────────────

/// Configuration for context window management.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextConfig {
    /// Trigger truncation when input_tokens exceeds this value.
    pub token_threshold: u32,
    /// Number of recent messages to keep after truncation.
    pub keep_last: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            token_threshold: 80_000,
            keep_last: 20,
        }
    }
}

// ── AgentConfig ───────────────────────────────────────────────────────────────

/// Configuration for the agent loop.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentConfig {
    /// Maximum number of turns before aborting with MaxTurnsExceeded.
    pub max_turns: usize,
    /// Tools to allow by name. If empty, default permissions apply.
    pub allow_tools: Vec<String>,
    /// Tools to deny by name.
    pub deny_tools: Vec<String>,
    /// Run in headless mode (Ask permissions become Deny).
    pub headless: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_turns: 25,
            allow_tools: vec![],
            deny_tools: vec![],
            headless: false,
        }
    }
}

// ── CoriConfig ────────────────────────────────────────────────────────────────

/// Top-level Cori configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CoriConfig {
    pub provider: ProviderConfig,
    pub context: ContextConfig,
    pub agent: AgentConfig,
}

impl CoriConfig {
    /// Load from environment variables only (simplest setup).
    pub fn from_env() -> Result<Self, anyhow::Error> {
        Ok(Self {
            provider: ProviderConfig::from_env()?,
            context: ContextConfig::default(),
            agent: AgentConfig::default(),
        })
    }
}
