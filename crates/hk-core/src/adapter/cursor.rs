use super::{AgentAdapter, HookEntry, McpServerEntry, PluginEntry};
use std::path::PathBuf;

pub struct CursorAdapter {
    home: PathBuf,
}

impl CursorAdapter {
    pub fn new() -> Self {
        Self { home: dirs::home_dir().unwrap_or_default() }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    fn read_json(&self, filename: &str) -> Option<serde_json::Value> {
        let path = self.base_dir().join(filename);
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for CursorAdapter {
    fn name(&self) -> &str { "cursor" }

    fn base_dir(&self) -> PathBuf { self.home.join(".cursor") }

    fn detect(&self) -> bool { self.base_dir().exists() }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ]
    }

    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("mcp.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("hooks.json")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("plugins")]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        let Some(config) = self.read_json("mcp.json") else { return vec![] };
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

    fn read_plugins(&self) -> Vec<PluginEntry> {
        // Cursor plugins: ~/.cursor/plugins/local/{plugin}/.cursor-plugin/plugin.json
        let local_dir = self.base_dir().join("plugins").join("local");
        let mut entries = Vec::new();
        if let Ok(dirs) = std::fs::read_dir(&local_dir) {
            for dir in dirs.flatten() {
                if !dir.path().is_dir() { continue; }
                let manifest = dir.path().join(".cursor-plugin").join("plugin.json");
                if !manifest.exists() { continue; }
                let name = if let Ok(content) = std::fs::read_to_string(&manifest) {
                    serde_json::from_str::<serde_json::Value>(&content).ok()
                        .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                        .unwrap_or_else(|| dir.file_name().to_string_lossy().to_string())
                } else {
                    dir.file_name().to_string_lossy().to_string()
                };
                entries.push(PluginEntry {
                    name,
                    source: "local".into(),
                    enabled: true,
                    path: Some(dir.path()),
                });
            }
        }
        // Also scan marketplace-installed plugins if they exist
        // Cursor may cache installed plugins similarly to Codex
        let cache_dir = self.base_dir().join("plugins").join("cache");
        if let Ok(marketplaces) = std::fs::read_dir(&cache_dir) {
            for marketplace in marketplaces.flatten() {
                if !marketplace.path().is_dir() { continue; }
                let mp_name = marketplace.file_name().to_string_lossy().to_string();
                if let Ok(plugins) = std::fs::read_dir(marketplace.path()) {
                    for plugin in plugins.flatten() {
                        if !plugin.path().is_dir() { continue; }
                        let manifest = plugin.path().join(".cursor-plugin").join("plugin.json");
                        let name = if manifest.exists() {
                            std::fs::read_to_string(&manifest).ok()
                                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                                .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                                .unwrap_or_else(|| plugin.file_name().to_string_lossy().to_string())
                        } else {
                            plugin.file_name().to_string_lossy().to_string()
                        };
                        entries.push(PluginEntry {
                            name,
                            source: mp_name.clone(),
                            enabled: true,
                            path: Some(plugin.path()),
                        });
                    }
                }
            }
        }
        entries
    }
}
