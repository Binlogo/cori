/// Permission Model
///
/// Every tool execution passes through a permission gate before running.
/// This allows fine-grained control over which tools are allowed in which context.
///
/// Permission levels:
///   Allow — always execute without confirmation
///   Ask   — prompt the user (only meaningful in interactive mode; defaults to Deny in headless)
///   Deny  — never execute
///
/// Resolution order (later overrides earlier):
///   1. DefaultPolicy (global fallback)
///   2. ToolPolicy (per-tool override)
///   3. Session override (e.g. --allow-tools bash)

use std::collections::HashMap;

// ── PermissionPolicy ──────────────────────────────────────────────────────────

/// The permission level for a tool execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionPolicy {
    /// Always execute without confirmation.
    Allow,
    /// Ask the user before executing. In headless mode, treated as Deny.
    Ask,
    /// Never execute; return an error message as tool content.
    Deny,
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        PermissionPolicy::Ask
    }
}

// ── PermissionGate ────────────────────────────────────────────────────────────

/// Resolves the effective permission for any tool call.
///
/// Usage:
/// ```
/// let mut gate = PermissionGate::new();
/// gate.set_default(PermissionPolicy::Ask);
/// gate.allow("read_file");
/// gate.deny("bash");
///
/// assert_eq!(gate.check("read_file"), PermissionPolicy::Allow);
/// assert_eq!(gate.check("bash"),      PermissionPolicy::Deny);
/// assert_eq!(gate.check("glob"),      PermissionPolicy::Ask);  // falls through to default
/// ```
pub struct PermissionGate {
    default_policy: PermissionPolicy,
    tool_policies: HashMap<String, PermissionPolicy>,
    /// If true, Ask is treated as Deny (headless / CI mode).
    headless: bool,
}

impl Default for PermissionGate {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionGate {
    pub fn new() -> Self {
        Self {
            default_policy: PermissionPolicy::Ask,
            tool_policies: HashMap::new(),
            headless: false,
        }
    }

    /// Treat all Ask permissions as Deny (for headless / CI mode).
    pub fn set_headless(&mut self, headless: bool) {
        self.headless = headless;
    }

    /// Set the fallback policy for tools without an explicit entry.
    pub fn set_default(&mut self, policy: PermissionPolicy) {
        self.default_policy = policy;
    }

    /// Explicitly allow a tool.
    pub fn allow(&mut self, tool: impl Into<String>) {
        self.tool_policies.insert(tool.into(), PermissionPolicy::Allow);
    }

    /// Explicitly deny a tool.
    pub fn deny(&mut self, tool: impl Into<String>) {
        self.tool_policies.insert(tool.into(), PermissionPolicy::Deny);
    }

    /// Allow all tools in the list.
    pub fn allow_all(&mut self, tools: impl IntoIterator<Item = impl Into<String>>) {
        for t in tools {
            self.allow(t);
        }
    }

    /// Resolve the effective permission for a tool call.
    pub fn check(&self, tool_name: &str) -> PermissionPolicy {
        let policy = self
            .tool_policies
            .get(tool_name)
            .unwrap_or(&self.default_policy)
            .clone();

        // In headless mode, Ask → Deny
        if self.headless && policy == PermissionPolicy::Ask {
            PermissionPolicy::Deny
        } else {
            policy
        }
    }

    /// Returns true if the tool is allowed to run without user interaction.
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        self.check(tool_name) == PermissionPolicy::Allow
    }

    /// Returns true if the tool requires user confirmation.
    pub fn needs_confirmation(&self, tool_name: &str) -> bool {
        self.check(tool_name) == PermissionPolicy::Ask
    }

    /// Returns true if the tool is denied.
    pub fn is_denied(&self, tool_name: &str) -> bool {
        self.check(tool_name) == PermissionPolicy::Deny
    }
}

// ── Presets ───────────────────────────────────────────────────────────────────

impl PermissionGate {
    /// Read-only preset: allow file reads and glob/grep, ask for writes and bash.
    pub fn read_only() -> Self {
        let mut gate = Self::new();
        gate.allow("read_file");
        gate.allow("glob");
        gate.allow("grep");
        gate.allow("task_list");
        gate.allow("task_get");
        gate.deny("write_file");
        gate.deny("edit_file");
        gate.deny("bash");
        gate
    }

    /// Headless preset: allow common read tools, deny interactive asks.
    pub fn headless() -> Self {
        let mut gate = Self::new();
        gate.set_headless(true);
        gate.allow("read_file");
        gate.allow("glob");
        gate.allow("grep");
        gate.allow("task_list");
        gate.allow("task_get");
        gate
    }

    /// Unrestricted preset: allow everything. Use only in trusted environments.
    pub fn unrestricted() -> Self {
        let mut gate = Self::new();
        gate.set_default(PermissionPolicy::Allow);
        gate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_allow_overrides_default() {
        let mut gate = PermissionGate::new();
        gate.set_default(PermissionPolicy::Deny);
        gate.allow("read_file");
        assert_eq!(gate.check("read_file"), PermissionPolicy::Allow);
        assert_eq!(gate.check("bash"), PermissionPolicy::Deny);
    }

    #[test]
    fn test_headless_ask_becomes_deny() {
        let mut gate = PermissionGate::new();
        gate.set_headless(true);
        // Default is Ask, which becomes Deny in headless
        assert_eq!(gate.check("bash"), PermissionPolicy::Deny);
    }

    #[test]
    fn test_headless_explicit_allow_stays_allow() {
        let mut gate = PermissionGate::new();
        gate.set_headless(true);
        gate.allow("read_file");
        assert_eq!(gate.check("read_file"), PermissionPolicy::Allow);
    }
}
