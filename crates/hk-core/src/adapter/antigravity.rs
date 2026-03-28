use super::{AgentAdapter, HookEntry, McpServerEntry};
use std::path::PathBuf;

pub struct AntigravityAdapter { home: PathBuf }

impl AntigravityAdapter {
    pub fn new() -> Self { Self { home: dirs::home_dir().unwrap_or_default() } }
    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self { Self { home } }
    fn read_settings(&self) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(self.base_dir().join("settings.json")).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for AntigravityAdapter {
    fn name(&self) -> &str { "antigravity" }
    fn base_dir(&self) -> PathBuf { self.home.join(".antigravity") }
    fn detect(&self) -> bool { self.base_dir().exists() }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".gemini").join("antigravity").join("skills"),
        ]
    }
    fn mcp_config_path(&self) -> PathBuf { self.base_dir().join("settings.json") }
    fn hook_config_path(&self) -> PathBuf { self.base_dir().join("settings.json") }
    fn plugin_dirs(&self) -> Vec<PathBuf> { vec![self.base_dir().join("plugins")] }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        let Some(settings) = self.read_settings() else { return vec![] };
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
        let Some(settings) = self.read_settings() else { return vec![] };
        let Some(hooks) = settings.get("hooks").and_then(|v| v.as_object()) else { return vec![] };
        let mut entries = Vec::new();
        for (event, hook_list) in hooks {
            let Some(arr) = hook_list.as_array() else { continue };
            for hook in arr {
                let matcher = hook.get("matcher").and_then(|v| v.as_str()).map(String::from);
                if let Some(cmds) = hook.get("hooks").and_then(|v| v.as_array()) {
                    for cmd in cmds {
                        if let Some(cmd_str) = cmd.as_str() {
                            entries.push(HookEntry { event: event.clone(), matcher: matcher.clone(), command: cmd_str.to_string() });
                        }
                    }
                }
            }
        }
        entries
    }

}
