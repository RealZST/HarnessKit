// MCP config reference: https://cursor.com/docs/mcp
// Config file: ~/.cursor/mcp.json (global), .cursor/mcp.json (project)
// Format: JSON, top-level key "mcpServers", sub-keys: command, args, env, url, headers
//
// Plugin reference: https://cursor.com/docs/plugins
// Plugins: ~/.cursor/plugins/, manifest at .cursor-plugin/plugin.json

use super::{AgentAdapter, HookEntry, HookFormat, McpServerEntry, PluginEntry};
use std::path::PathBuf;

pub struct CursorAdapter {
    home: PathBuf,
}

impl Default for CursorAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorAdapter {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
        }
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
    fn hook_format(&self) -> HookFormat {
        HookFormat::Cursor
    }
    fn name(&self) -> &str {
        "cursor"
    }

    fn base_dir(&self) -> PathBuf {
        self.home.join(".cursor")
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
        self.base_dir().join("mcp.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("hooks.json")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("plugins")]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        let Some(config) = self.read_json("mcp.json") else {
            return vec![];
        };
        let Some(servers) = config.get("mcpServers").and_then(|v| v.as_object()) else {
            return vec![];
        };
        servers
            .iter()
            .map(|(name, val)| McpServerEntry {
                name: name.clone(),
                command: val
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
                args: val
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                env: val
                    .get("env")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect()
    }

    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_cursor(event)
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        let Some(config) = self.read_json("hooks.json") else {
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
                // Cursor format: {"command": "..."} — no matcher, no nested hooks array
                if let Some(cmd) = hook.get("command").and_then(|v| v.as_str()) {
                    entries.push(HookEntry {
                        event: event.clone(),
                        matcher: None,
                        command: cmd.to_string(),
                    });
                }
            }
        }
        entries
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files = vec![
            self.base_dir().join("mcp.json"),
            self.base_dir().join("permissions.json"),
            self.base_dir().join("hooks.json"),
        ];
        // ~/.cursor/agents/*.md
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
            ".cursorrules".into(),
            ".cursor/rules/*.mdc".into(),
            ".cursor/rules/*.md".into(),
            "AGENTS.md".into(),
        ]
    }

    fn project_memory_patterns(&self) -> Vec<String> {
        vec![".cursor/notepads/*.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".cursor/mcp.json".into()]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".cursorignore".into(), ".cursorindexingignore".into()]
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        // Cursor plugins: ~/.cursor/plugins/local/{plugin}/.cursor-plugin/plugin.json
        let local_dir = self.base_dir().join("plugins").join("local");
        let mut entries = Vec::new();
        if let Ok(dirs) = std::fs::read_dir(&local_dir) {
            for dir in dirs.flatten() {
                if !dir.path().is_dir() {
                    continue;
                }
                let manifest = dir.path().join(".cursor-plugin").join("plugin.json");
                if !manifest.exists() {
                    continue;
                }
                let name = if let Ok(content) = std::fs::read_to_string(&manifest) {
                    serde_json::from_str::<serde_json::Value>(&content)
                        .ok()
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
                    installed_at: None,
                    updated_at: None,
                });
            }
        }
        // Also scan marketplace-installed plugins if they exist
        // Cursor may cache installed plugins similarly to Codex
        let cache_dir = self.base_dir().join("plugins").join("cache");
        if let Ok(marketplaces) = std::fs::read_dir(&cache_dir) {
            for marketplace in marketplaces.flatten() {
                if !marketplace.path().is_dir() {
                    continue;
                }
                let mp_name = marketplace.file_name().to_string_lossy().to_string();
                if let Ok(plugins) = std::fs::read_dir(marketplace.path()) {
                    for plugin in plugins.flatten() {
                        if !plugin.path().is_dir() {
                            continue;
                        }
                        let manifest = plugin.path().join(".cursor-plugin").join("plugin.json");
                        let name = if manifest.exists() {
                            std::fs::read_to_string(&manifest)
                                .ok()
                                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                                .and_then(|v| {
                                    v.get("name").and_then(|n| n.as_str()).map(String::from)
                                })
                                .unwrap_or_else(|| plugin.file_name().to_string_lossy().to_string())
                        } else {
                            plugin.file_name().to_string_lossy().to_string()
                        };
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
            }
        }
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    #[test]
    fn read_hooks_cursor_format() {
        let tmp = tempfile::tempdir().unwrap();
        let cursor_dir = tmp.path().join(".cursor");
        std::fs::create_dir_all(&cursor_dir).unwrap();
        std::fs::write(cursor_dir.join("hooks.json"),
            r#"{"version":1,"hooks":{"afterFileEdit":[{"command":"./audit.sh"}],"stop":[{"command":"echo done"}]}}"#
        ).unwrap();
        let adapter = CursorAdapter::with_home(tmp.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 2);
        assert!(
            hooks
                .iter()
                .any(|h| h.event == "afterFileEdit" && h.command == "./audit.sh")
        );
        assert!(
            hooks
                .iter()
                .any(|h| h.event == "stop" && h.command == "echo done")
        );
    }
}
