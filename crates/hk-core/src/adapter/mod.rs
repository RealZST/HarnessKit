pub mod claude;
pub mod cursor;
pub mod codex;
pub mod gemini;
pub mod antigravity;
pub mod copilot;

use std::path::PathBuf;

/// Represents an MCP server entry parsed from an agent's config
#[derive(Debug, Clone)]
pub struct McpServerEntry {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: std::collections::HashMap<String, String>,
}

/// Represents a hook entry parsed from an agent's config
#[derive(Debug, Clone)]
pub struct HookEntry {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
}

/// Represents a plugin entry parsed from an agent's config
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub name: String,
    pub source: String,
    pub enabled: bool,
    pub path: Option<std::path::PathBuf>,
    /// Precise install timestamp (e.g. from a registry file). Overrides file-system heuristic.
    pub installed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Precise last-updated timestamp. Overrides file-system heuristic.
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Format used by an agent for hook configuration files.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HookFormat {
    /// Claude, Codex, Gemini: {"hooks": {"Event": [{"matcher": "...", "hooks": ["cmd"]}]}}
    ClaudeLike,
    /// Cursor: {"version": 1, "hooks": {"event": [{"command": "cmd"}]}}
    Cursor,
    /// Copilot: {"version": 1, "hooks": {"event": [{"type": "command", "bash": "cmd"}]}}
    Copilot,
    /// Agent does not support hooks
    None,
}

pub trait AgentAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn base_dir(&self) -> PathBuf;
    fn detect(&self) -> bool;
    fn skill_dirs(&self) -> Vec<PathBuf>;
    fn mcp_config_path(&self) -> PathBuf;
    fn hook_config_path(&self) -> PathBuf;
    fn plugin_dirs(&self) -> Vec<PathBuf>;
    fn read_mcp_servers(&self) -> Vec<McpServerEntry>;
    fn read_hooks(&self) -> Vec<HookEntry>;
    fn read_plugins(&self) -> Vec<PluginEntry> { vec![] }
    fn hook_format(&self) -> HookFormat { HookFormat::ClaudeLike }

    // --- Config file discovery (for Agents page) ---

    /// Global rule/instruction files (absolute paths, e.g. ~/.claude/CLAUDE.md)
    fn global_rules_files(&self) -> Vec<PathBuf> { vec![] }

    /// Global memory files (absolute paths)
    fn global_memory_files(&self) -> Vec<PathBuf> { vec![] }

    /// Global settings files (absolute paths, e.g. ~/.claude/settings.json)
    fn global_settings_files(&self) -> Vec<PathBuf> { vec![] }

    /// Relative paths/globs for rules within a project dir (e.g. "CLAUDE.md")
    fn project_rules_patterns(&self) -> Vec<String> { vec![] }

    /// Relative paths/globs for memory within a project dir
    fn project_memory_patterns(&self) -> Vec<String> { vec![] }

    /// Relative paths/globs for settings within a project dir
    fn project_settings_patterns(&self) -> Vec<String> { vec![] }

    /// Relative paths/globs for ignore files within a project dir
    fn project_ignore_patterns(&self) -> Vec<String> { vec![] }
}

/// Returns all agent adapters in canonical display order.
/// Must match AGENT_ORDER in src/lib/types.ts.
pub fn all_adapters() -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(claude::ClaudeAdapter::new()),
        Box::new(codex::CodexAdapter::new()),
        Box::new(gemini::GeminiAdapter::new()),
        Box::new(cursor::CursorAdapter::new()),
        Box::new(antigravity::AntigravityAdapter::new()),
        Box::new(copilot::CopilotAdapter::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_adapters_returns_six() {
        let adapters = all_adapters();
        assert_eq!(adapters.len(), 6);
        let names: Vec<&str> = adapters.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"cursor"));
        assert!(names.contains(&"codex"));
        assert!(names.contains(&"gemini"));
        assert!(names.contains(&"antigravity"));
        assert!(names.contains(&"copilot"));
    }

    #[test]
    fn test_default_config_methods_return_empty() {
        let adapters = all_adapters();
        for a in &adapters {
            let _ = a.global_rules_files();
            let _ = a.global_memory_files();
            let _ = a.global_settings_files();
            let _ = a.project_rules_patterns();
            let _ = a.project_memory_patterns();
            let _ = a.project_settings_patterns();
            let _ = a.project_ignore_patterns();
        }
    }
}
