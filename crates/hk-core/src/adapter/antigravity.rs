// MCP config reference: https://antigravity.google/docs/mcp
// Config file: ~/.gemini/antigravity/mcp_config.json
// Format: JSON, top-level key "mcpServers", sub-keys: command, args, env, serverUrl, headers, etc.

use super::{AgentAdapter, HookEntry, HookFormat, McpServerEntry};
use std::path::PathBuf;

pub struct AntigravityAdapter { home: PathBuf }

impl Default for AntigravityAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AntigravityAdapter {
    pub fn new() -> Self { Self { home: dirs::home_dir().unwrap_or_default() } }
    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self { Self { home } }
}

impl AgentAdapter for AntigravityAdapter {
    fn hook_format(&self) -> HookFormat { HookFormat::None }
    fn name(&self) -> &str { "antigravity" }
    fn base_dir(&self) -> PathBuf { self.home.join(".antigravity") }
    fn detect(&self) -> bool { self.base_dir().exists() }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".gemini").join("antigravity").join("skills"),
        ]
    }
    fn mcp_config_path(&self) -> PathBuf { self.home.join(".gemini").join("antigravity").join("mcp_config.json") }
    fn hook_config_path(&self) -> PathBuf { self.base_dir().join("settings.json") }
    fn plugin_dirs(&self) -> Vec<PathBuf> { vec![self.base_dir().join("plugins")] }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![
            self.home.join(".gemini").join("GEMINI.md"),
        ]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![
            self.home.join(".gemini").join("antigravity").join("mcp_config.json"),
        ]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            ".agents/rules/*.md".into(),
            ".agent/rules/*.md".into(),  // backward compat
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".geminiignore".into()]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        let content = std::fs::read_to_string(self.mcp_config_path()).ok();
        let settings: Option<serde_json::Value> = content.and_then(|c| serde_json::from_str(&c).ok());
        let Some(settings) = settings else { return vec![] };
        let Some(servers) = settings.get("mcpServers").and_then(|v| v.as_object()) else { return vec![] };
        servers.iter().map(|(name, val)| McpServerEntry {
            name: name.clone(),
            command: val.get("command").and_then(|v| v.as_str()).unwrap_or("").into(),
            args: val.get("args").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            env: val.get("env").and_then(|v| v.as_object())
                .map(|obj| obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
                .unwrap_or_default(),
        }).collect()
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::AgentAdapter;

    #[test]
    fn read_hooks_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        // Even with a hooks-like config, Antigravity should return nothing
        let ag_dir = tmp.path().join(".antigravity");
        std::fs::create_dir_all(&ag_dir).unwrap();
        std::fs::write(ag_dir.join("settings.json"),
            r#"{"hooks":{"Stop":[{"hooks":["echo fake"]}]}}"#
        ).unwrap();
        let adapter = AntigravityAdapter::with_home(tmp.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert!(hooks.is_empty(), "Antigravity should not support hooks");
    }
}
