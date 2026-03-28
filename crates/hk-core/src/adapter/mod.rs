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
}

pub fn all_adapters() -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(claude::ClaudeAdapter::new()),
        Box::new(cursor::CursorAdapter::new()),
        Box::new(codex::CodexAdapter::new()),
        Box::new(gemini::GeminiAdapter::new()),
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
}
