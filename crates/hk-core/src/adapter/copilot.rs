use super::{AgentAdapter, HookEntry, HookFormat, McpServerEntry, PluginEntry};
use std::path::PathBuf;

pub struct CopilotAdapter { home: PathBuf }

impl Default for CopilotAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CopilotAdapter {
    pub fn new() -> Self { Self { home: dirs::home_dir().unwrap_or_default() } }
    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self { Self { home } }
    fn read_json(&self, filename: &str) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(self.base_dir().join(filename)).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for CopilotAdapter {
    fn hook_format(&self) -> HookFormat { HookFormat::Copilot }
    fn name(&self) -> &str { "copilot" }
    fn base_dir(&self) -> PathBuf { self.home.join(".copilot") }
    fn detect(&self) -> bool { self.base_dir().exists() }
    fn skill_dirs(&self) -> Vec<PathBuf> { vec![self.base_dir().join("skills")] }
    fn mcp_config_path(&self) -> PathBuf { self.base_dir().join("mcp-config.json") }
    fn hook_config_path(&self) -> PathBuf { self.base_dir().join("hooks.json") }
    fn plugin_dirs(&self) -> Vec<PathBuf> { vec![self.base_dir().join("plugins")] }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("copilot-instructions.md")]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files = vec![
            self.base_dir().join("config.json"),
            self.base_dir().join("mcp-config.json"),
        ];
        // ~/.copilot/agents/*.agent.md
        let agents_dir = self.base_dir().join("agents");
        if let Ok(entries) = std::fs::read_dir(&agents_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "md") {
                    files.push(p);
                }
            }
        }
        files
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            ".github/copilot-instructions.md".into(),
            ".github/instructions/*.instructions.md".into(),
            "AGENTS.md".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![
            "copilot/mcp-config.json".into(),
        ]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".copilotignore".into()]
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        // Copilot plugins: ~/.copilot/installed-plugins/{marketplace}/{plugin}/plugin.json
        let base = self.base_dir().join("installed-plugins");
        let Ok(marketplaces) = std::fs::read_dir(&base) else { return vec![] };
        let mut entries = Vec::new();
        for marketplace in marketplaces.flatten() {
            if !marketplace.path().is_dir() { continue; }
            let mp_name = marketplace.file_name().to_string_lossy().to_string();
            let Ok(plugins) = std::fs::read_dir(marketplace.path()) else { continue };
            for plugin in plugins.flatten() {
                if !plugin.path().is_dir() { continue; }
                // Try plugin.json in several locations
                let manifest_paths = [
                    plugin.path().join("plugin.json"),
                    plugin.path().join(".plugin").join("plugin.json"),
                ];
                let name = manifest_paths.iter()
                    .find(|p| p.exists())
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                    .unwrap_or_else(|| plugin.file_name().to_string_lossy().to_string());
                entries.push(PluginEntry {
                    name,
                    source: mp_name.clone(),
                    enabled: true,
                    path: Some(plugin.path()),
                    installed_at: None,
                    updated_at: None,
                });
            }
        }
        entries
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        let Some(config) = self.read_json("mcp-config.json") else { return vec![] };
        let Some(servers) = config.get("mcpServers").and_then(|v| v.as_object()) else { return vec![] };
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
        let Some(config) = self.read_json("hooks.json") else { return vec![] };
        let Some(hooks) = config.get("hooks").and_then(|v| v.as_object()) else { return vec![] };
        let mut entries = Vec::new();
        for (event, hook_list) in hooks {
            let Some(arr) = hook_list.as_array() else { continue };
            for hook in arr {
                // Copilot format: {"type": "command", "bash": "...", "powershell": "..."}
                let cmd = hook.get("bash").and_then(|v| v.as_str())
                    .or_else(|| hook.get("command").and_then(|v| v.as_str()));
                if let Some(cmd_str) = cmd {
                    entries.push(HookEntry { event: event.clone(), matcher: None, command: cmd_str.to_string() });
                }
            }
        }
        entries
    }
}
