// MCP config reference: https://developers.openai.com/codex/mcp
// Config file: ~/.codex/config.toml
// Format: TOML, section [mcp_servers.<name>], sub-keys: command, args, env, url, etc.
//
// Plugin reference: https://developers.openai.com/codex/plugins
// Plugins: ~/.codex/plugins/cache/{marketplace}/{plugin}/{version}/, manifest at .codex-plugin/plugin.json

use super::{AgentAdapter, HookEntry, McpFormat, McpServerEntry, PluginEntry};
use std::path::{Path, PathBuf};

pub struct CodexAdapter {
    home: PathBuf,
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexAdapter {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    fn parse_json(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for CodexAdapter {
    fn name(&self) -> &str {
        "codex"
    }
    fn base_dir(&self) -> PathBuf {
        self.home.join(".codex")
    }
    fn detect(&self) -> bool {
        self.base_dir().exists()
    }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ]
    }
    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("config.toml")
    }
    fn mcp_format(&self) -> McpFormat {
        McpFormat::Toml
    }
    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("hooks.json")
    }
    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("plugins")]
    }

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
        vec!["AGENTS.md".into(), "AGENTS.override.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".codex/config.toml".into()]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        // Codex CLI scans .agents/skills from cwd up to the repo root.
        // Source: https://developers.openai.com/codex/skills
        vec![".agents/skills".into()]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let content = std::fs::read_to_string(path).ok();
        let doc: Option<toml::Table> = content.and_then(|c| c.parse().ok());
        let Some(doc) = doc else { return vec![] };
        let Some(servers) = doc.get("mcp_servers").and_then(|v| v.as_table()) else {
            return vec![];
        };
        servers
            .iter()
            .map(|(name, val)| {
                let table = val.as_table();
                // Prefer `_hk_name` (original name before TOML key sanitization)
                // so that the scanner produces the same Extension name across all
                // agents. Falls back to the TOML key if `_hk_name` is absent.
                let canonical_name = table
                    .and_then(|t| t.get("_hk_name"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| name.clone());
                McpServerEntry {
                    name: canonical_name,
                    command: table
                        .and_then(|t| t.get("command"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .into(),
                    args: table
                        .and_then(|t| t.get("args"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    env: table
                        .and_then(|t| t.get("env"))
                        .and_then(|v| v.as_table())
                        .map(|obj| {
                            obj.iter()
                                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            })
            .collect()
    }

    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_claude(event) // Codex uses same event names as Claude
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        self.read_hooks_from(&self.hook_config_path())
    }

    fn read_hooks_from(&self, path: &Path) -> Vec<HookEntry> {
        let Some(config) = Self::parse_json(path) else {
            return vec![];
        };
        let Some(hooks) = config.get("hooks").and_then(|v| v.as_object()) else {
            return vec![];
        };
        let mut entries = Vec::new();
        for (event, hook_list) in hooks {
            let Some(arr) = hook_list.as_array() else {
                continue;
            };
            for hook in arr {
                let matcher = hook
                    .get("matcher")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                if let Some(cmds) = hook.get("hooks").and_then(|v| v.as_array()) {
                    for cmd in cmds {
                        // String format: "echo test"
                        let cmd_str = if let Some(s) = cmd.as_str() {
                            Some(s.to_string())
                        }
                        // Object format: {"type": "command", "command": "echo test"}
                        else if let Some(s) = cmd.get("command").and_then(|v| v.as_str()) {
                            Some(s.to_string())
                        }
                        // Prompt/agent hook: {"type": "prompt", "prompt": "..."}
                        else {
                            cmd.get("prompt")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        };
                        if let Some(command) = cmd_str {
                            entries.push(HookEntry {
                                event: event.clone(),
                                matcher: matcher.clone(),
                                command,
                            });
                        }
                    }
                }
            }
        }
        entries
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        // Read disabled plugins from config.toml [plugins."name@source"] enabled = false
        let disabled_plugins: std::collections::HashSet<String> = std::fs::read_to_string(self.mcp_config_path()).ok()
            .and_then(|content| content.parse::<toml::Table>().ok())
            .and_then(|doc| doc.get("plugins").and_then(|v| v.as_table()).cloned())
            .map(|plugins| {
                plugins.into_iter()
                    .filter(|(_, v)| {
                        v.as_table()
                            .and_then(|t| t.get("enabled"))
                            .and_then(|e| e.as_bool())
                            == Some(false)
                    })
                    .map(|(k, _)| k)
                    .collect()
            })
            .unwrap_or_default();

        // Codex plugins are cached at ~/.codex/plugins/cache/{marketplace}/{plugin}/{version}/
        // Each has .codex-plugin/plugin.json manifest
        let cache_dir = self.base_dir().join("plugins").join("cache");
        let Ok(marketplaces) = std::fs::read_dir(&cache_dir) else {
            return vec![];
        };
        let mut entries = Vec::new();
        for marketplace in marketplaces.flatten() {
            if !marketplace.path().is_dir() {
                continue;
            }
            let marketplace_name = marketplace.file_name().to_string_lossy().to_string();
            let Ok(plugins) = std::fs::read_dir(marketplace.path()) else {
                continue;
            };
            for plugin in plugins.flatten() {
                if !plugin.path().is_dir() {
                    continue;
                }
                let plugin_name = plugin.file_name().to_string_lossy().to_string();
                // Find the latest version directory (sorted by semver descending)
                let Ok(versions) = std::fs::read_dir(plugin.path()) else {
                    continue;
                };
                let mut version_dirs: Vec<_> =
                    versions.flatten().filter(|v| v.path().is_dir()).collect();
                version_dirs.sort_by(|a, b| {
                    let va = a
                        .file_name()
                        .to_string_lossy()
                        .trim_start_matches('v')
                        .to_string();
                    let vb = b
                        .file_name()
                        .to_string_lossy()
                        .trim_start_matches('v')
                        .to_string();
                    match (semver::Version::parse(&va), semver::Version::parse(&vb)) {
                        (Ok(sa), Ok(sb)) => sb.cmp(&sa),
                        _ => b.file_name().cmp(&a.file_name()),
                    }
                });
                for version_dir in version_dirs {
                    let manifest_path =
                        version_dir.path().join(".codex-plugin").join("plugin.json");
                    if !manifest_path.exists() {
                        continue;
                    }
                    // Read manifest for metadata
                    let name = if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                        serde_json::from_str::<serde_json::Value>(&content)
                            .ok()
                            .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                            .unwrap_or_else(|| plugin_name.clone())
                    } else {
                        plugin_name.clone()
                    };
                    entries.push(PluginEntry {
                        name: name.clone(),
                        source: marketplace_name.clone(),
                        enabled: !disabled_plugins.contains(&format!("{}@{}", name, &marketplace_name)),
                        path: Some(version_dir.path().to_path_buf()), // version level — matches manifest location
                        uri: None,
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
    use super::super::AgentAdapter;
    use super::*;
    use std::fs;

    /// Helper: create a plugin version directory with a manifest
    fn create_plugin_version(
        base: &std::path::Path,
        marketplace: &str,
        plugin: &str,
        version: &str,
        manifest_name: &str,
    ) {
        let version_dir = base
            .join(".codex/plugins/cache")
            .join(marketplace)
            .join(plugin)
            .join(version);
        let manifest_dir = version_dir.join(".codex-plugin");
        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(
            manifest_dir.join("plugin.json"),
            format!(r#"{{"name":"{}"}}"#, manifest_name),
        )
        .unwrap();
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
        let path_str = plugins[0]
            .path
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(
            path_str.ends_with("1.10.0"),
            "Expected 1.10.0 but got path: {}",
            path_str
        );
    }

    #[test]
    fn read_plugins_handles_v_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_version(tmp.path(), "npm", "tool", "v2.0.0", "tool");
        create_plugin_version(tmp.path(), "npm", "tool", "v1.5.0", "tool");

        let adapter = CodexAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();

        assert_eq!(plugins.len(), 1);
        let path_str = plugins[0]
            .path
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(
            path_str.contains("v2.0.0"),
            "Expected v2.0.0 but got: {}",
            path_str
        );
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
        assert_eq!(
            plugins.len(),
            1,
            "Expected 1 plugin entry, got {}",
            plugins.len()
        );
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
        assert_eq!(
            plugins_after.len(),
            0,
            "Plugin should not come back after deleting parent dir"
        );
    }

    #[test]
    fn read_plugins_empty_cache() {
        let tmp = tempfile::tempdir().unwrap();
        // No .codex directory at all
        let adapter = CodexAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert!(plugins.is_empty());
    }

    #[test]
    fn read_plugins_respects_config_toml_enabled() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = CodexAdapter::with_home(tmp.path().to_path_buf());

        // Set up a plugin in cache
        let plugin_dir = tmp.path().join(".codex/plugins/cache/test-marketplace/my-plugin/1.0.0/.codex-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.json"), r#"{"name":"my-plugin"}"#).unwrap();

        // No config.toml → plugin should be enabled (default)
        let entries = adapter.read_plugins();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].enabled);

        // config.toml with plugin disabled
        let config_path = tmp.path().join(".codex/config.toml");
        fs::write(&config_path, r#"
[plugins."my-plugin@test-marketplace"]
enabled = false
"#).unwrap();

        let entries = adapter.read_plugins();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].enabled, "Plugin should be disabled per config.toml");
    }

    #[test]
    fn read_hooks_object_format() {
        let tmp = tempfile::tempdir().unwrap();
        let codex_dir = tmp.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::write(codex_dir.join("hooks.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo test"}]}]}}"#
        ).unwrap();
        let adapter = CodexAdapter::with_home(tmp.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].event, "PreToolUse");
        assert_eq!(hooks[0].command, "echo test");
    }
}
