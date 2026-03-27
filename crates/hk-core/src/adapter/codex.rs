use super::{AgentAdapter, HookEntry, McpServerEntry, PluginEntry};
use std::path::PathBuf;

pub struct CodexAdapter {
    home: PathBuf,
}

impl CodexAdapter {
    pub fn new() -> Self {
        Self { home: dirs::home_dir().unwrap_or_default() }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self { Self { home } }

    fn base_dir(&self) -> PathBuf { self.home.join(".codex") }

    fn read_config(&self) -> Option<serde_json::Value> {
        let path = self.base_dir().join("config.json");
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for CodexAdapter {
    fn name(&self) -> &str { "codex" }
    fn detect(&self) -> bool { self.base_dir().exists() }
    fn skill_dirs(&self) -> Vec<PathBuf> { vec![self.base_dir().join("skills")] }
    fn mcp_config_path(&self) -> PathBuf { self.base_dir().join("config.json") }
    fn hook_config_path(&self) -> PathBuf { self.base_dir().join("config.json") }
    fn plugin_dirs(&self) -> Vec<PathBuf> { vec![self.base_dir().join("plugins")] }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        let Some(config) = self.read_config() else { return vec![] };
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
        let Some(config) = self.read_config() else { return vec![] };
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
        // Codex plugins are cached at ~/.codex/plugins/cache/{marketplace}/{plugin}/{version}/
        // Each has .codex-plugin/plugin.json manifest
        let cache_dir = self.base_dir().join("plugins").join("cache");
        let Ok(marketplaces) = std::fs::read_dir(&cache_dir) else { return vec![] };
        let mut entries = Vec::new();
        for marketplace in marketplaces.flatten() {
            if !marketplace.path().is_dir() { continue; }
            let marketplace_name = marketplace.file_name().to_string_lossy().to_string();
            let Ok(plugins) = std::fs::read_dir(marketplace.path()) else { continue };
            for plugin in plugins.flatten() {
                if !plugin.path().is_dir() { continue; }
                let plugin_name = plugin.file_name().to_string_lossy().to_string();
                // Find the latest version directory
                let Ok(versions) = std::fs::read_dir(plugin.path()) else { continue };
                for version_dir in versions.flatten() {
                    if !version_dir.path().is_dir() { continue; }
                    let manifest_path = version_dir.path().join(".codex-plugin").join("plugin.json");
                    if !manifest_path.exists() { continue; }
                    // Read manifest for metadata
                    let name = if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                        serde_json::from_str::<serde_json::Value>(&content).ok()
                            .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                            .unwrap_or_else(|| plugin_name.clone())
                    } else {
                        plugin_name.clone()
                    };
                    entries.push(PluginEntry {
                        name,
                        source: marketplace_name.clone(),
                        enabled: true,
                        path: Some(version_dir.path()),
                    });
                    break; // Only take the first (latest) version
                }
            }
        }
        entries
    }
}
