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

    fn read_config(&self) -> Option<serde_json::Value> {
        let path = self.base_dir().join("config.json");
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for CodexAdapter {
    fn name(&self) -> &str { "codex" }
    fn base_dir(&self) -> PathBuf { self.home.join(".codex") }
    fn detect(&self) -> bool { self.base_dir().exists() }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ]
    }
    fn mcp_config_path(&self) -> PathBuf { self.base_dir().join("config.json") }
    fn hook_config_path(&self) -> PathBuf { self.base_dir().join("config.json") }
    fn plugin_dirs(&self) -> Vec<PathBuf> { vec![self.base_dir().join("plugins")] }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("AGENTS.md"),
            self.base_dir().join("AGENTS.override.md"),
        ]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("config.toml"),
            self.base_dir().join("hooks.json"),
        ]
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let memories_dir = self.base_dir().join("memories");
        if let Ok(entries) = std::fs::read_dir(&memories_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().is_some_and(|e| e == "md") {
                    files.push(p);
                }
            }
        }
        files
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            "AGENTS.md".into(),
            "AGENTS.override.md".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".codex/config.toml".into()]
    }

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
                // Find the latest version directory (sorted by semver descending)
                let Ok(versions) = std::fs::read_dir(plugin.path()) else { continue };
                let mut version_dirs: Vec<_> = versions.flatten()
                    .filter(|v| v.path().is_dir())
                    .collect();
                version_dirs.sort_by(|a, b| {
                    let va = a.file_name().to_string_lossy().trim_start_matches('v').to_string();
                    let vb = b.file_name().to_string_lossy().trim_start_matches('v').to_string();
                    match (semver::Version::parse(&va), semver::Version::parse(&vb)) {
                        (Ok(sa), Ok(sb)) => sb.cmp(&sa),
                        _ => b.file_name().cmp(&a.file_name()),
                    }
                });
                for version_dir in version_dirs {
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
                        path: Some(version_dir.path().to_path_buf()), // version level — matches manifest location
                        installed_at: None,
                        updated_at: None,
                    });
                    break; // Take the latest version after sorting
                }
            }
        }
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::AgentAdapter;
    use std::fs;

    /// Helper: create a plugin version directory with a manifest
    fn create_plugin_version(base: &std::path::Path, marketplace: &str, plugin: &str, version: &str, manifest_name: &str) {
        let version_dir = base.join(".codex/plugins/cache").join(marketplace).join(plugin).join(version);
        let manifest_dir = version_dir.join(".codex-plugin");
        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(
            manifest_dir.join("plugin.json"),
            format!(r#"{{"name":"{}"}}"#, manifest_name),
        ).unwrap();
    }

    #[test]
    fn read_plugins_picks_latest_semver() {
        let tmp = tempfile::tempdir().unwrap();
        // Create versions in non-sorted order: 1.9.0 then 1.10.0
        // Lexicographic sort would wrongly pick 1.9.0 > 1.10.0
        create_plugin_version(tmp.path(), "npm", "my-plugin", "1.9.0", "my-plugin");
        create_plugin_version(tmp.path(), "npm", "my-plugin", "1.10.0", "my-plugin");
        create_plugin_version(tmp.path(), "npm", "my-plugin", "1.2.0", "my-plugin");

        let adapter = CodexAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();

        assert_eq!(plugins.len(), 1);
        // The path should end with 1.10.0, not 1.9.0
        let path_str = plugins[0].path.as_ref().unwrap().to_string_lossy().to_string();
        assert!(path_str.ends_with("1.10.0"), "Expected 1.10.0 but got path: {}", path_str);
    }

    #[test]
    fn read_plugins_handles_v_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_version(tmp.path(), "npm", "tool", "v2.0.0", "tool");
        create_plugin_version(tmp.path(), "npm", "tool", "v1.5.0", "tool");

        let adapter = CodexAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();

        assert_eq!(plugins.len(), 1);
        let path_str = plugins[0].path.as_ref().unwrap().to_string_lossy().to_string();
        assert!(path_str.contains("v2.0.0"), "Expected v2.0.0 but got: {}", path_str);
    }

    #[test]
    fn read_plugins_returns_one_entry_per_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_version(tmp.path(), "npm", "alpha", "1.0.0", "alpha");
        create_plugin_version(tmp.path(), "npm", "alpha", "2.0.0", "alpha");
        create_plugin_version(tmp.path(), "npm", "alpha", "3.0.0", "alpha");

        let adapter = CodexAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();

        // Should return exactly 1 entry (latest), not 3
        assert_eq!(plugins.len(), 1, "Expected 1 plugin entry, got {}", plugins.len());
    }

    #[test]
    fn delete_plugin_parent_removes_all_versions() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_version(tmp.path(), "npm", "doomed", "1.0.0", "doomed");
        create_plugin_version(tmp.path(), "npm", "doomed", "2.0.0", "doomed");
        create_plugin_version(tmp.path(), "npm", "doomed", "3.0.0", "doomed");

        let adapter = CodexAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 1);

        // Simulate what commands.rs does for codex adapter: delete parent of version dir
        let version_path = plugins[0].path.as_ref().unwrap();
        let plugin_dir = version_path.parent().unwrap();
        assert!(plugin_dir.file_name().unwrap() == "doomed");
        fs::remove_dir_all(plugin_dir).unwrap();

        // After deletion, no plugins should be found
        let plugins_after = adapter.read_plugins();
        assert_eq!(plugins_after.len(), 0, "Plugin should not come back after deleting parent dir");
    }

    #[test]
    fn read_plugins_empty_cache() {
        let tmp = tempfile::tempdir().unwrap();
        // No .codex directory at all
        let adapter = CodexAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert!(plugins.is_empty());
    }
}
