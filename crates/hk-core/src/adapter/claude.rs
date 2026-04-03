use super::{AgentAdapter, HookEntry, McpServerEntry, PluginEntry};
use std::path::PathBuf;

pub struct ClaudeAdapter {
    home: PathBuf,
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    fn read_settings(&self) -> Option<serde_json::Value> {
        let path = self.base_dir().join("settings.json");
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for ClaudeAdapter {
    fn name(&self) -> &str {
        "claude"
    }

    fn base_dir(&self) -> PathBuf {
        self.home.join(".claude")
    }

    fn detect(&self) -> bool {
        self.base_dir().exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("skills")]
    }

    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("settings.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("settings.json")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("plugins")]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        let Some(settings) = self.read_settings() else { return vec![] };
        let Some(servers) = settings.get("mcpServers").and_then(|v| v.as_object()) else { return vec![] };

        servers
            .iter()
            .map(|(name, val)| McpServerEntry {
                name: name.clone(),
                command: val.get("command").and_then(|v| v.as_str()).unwrap_or("").into(),
                args: val
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
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
                            entries.push(HookEntry {
                                event: event.clone(),
                                matcher: matcher.clone(),
                                command: cmd_str.to_string(),
                            });
                        }
                    }
                }
            }
        }
        entries
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.base_dir().join("CLAUDE.md")];
        // Also scan ~/.claude/rules/*.md
        let rules_dir = self.base_dir().join("rules");
        if let Ok(entries) = std::fs::read_dir(&rules_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "md") {
                    files.push(p);
                }
            }
        }
        files
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        let projects_dir = self.base_dir().join("projects");
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&projects_dir) {
            for entry in entries.flatten() {
                let memory_dir = entry.path().join("memory");
                if memory_dir.is_dir()
                    && let Ok(mem_entries) = std::fs::read_dir(&memory_dir) {
                        for mem_entry in mem_entries.flatten() {
                            let p = mem_entry.path();
                            if p.extension().is_some_and(|e| e == "md") {
                                files.push(p);
                            }
                        }
                    }
            }
        }
        files
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files = vec![
            self.base_dir().join("settings.json"),
            self.base_dir().join("settings.local.json"),
            self.base_dir().join("keybindings.json"),
        ];
        // ~/.claude/agents/*.md
        let agents_dir = self.base_dir().join("agents");
        if let Ok(entries) = std::fs::read_dir(&agents_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "md") {
                    files.push(p);
                }
            }
        }
        // ~/.claude/commands/*.md (legacy, still functional)
        let commands_dir = self.base_dir().join("commands");
        if let Ok(entries) = std::fs::read_dir(&commands_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "md") {
                    files.push(p);
                }
            }
        }
        // ~/.claude/output-styles/*.md
        let styles_dir = self.base_dir().join("output-styles");
        if let Ok(entries) = std::fs::read_dir(&styles_dir) {
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
            "CLAUDE.md".into(),
            ".claude/CLAUDE.md".into(),
            ".claude/rules/*.md".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![
            ".claude/settings.json".into(),
            ".claude/settings.local.json".into(),
            ".mcp.json".into(),
        ]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![]  // Claude Code does NOT have .claudeignore
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        // Read from installed_plugins.json which has precise per-plugin timestamps
        let registry_path = self.base_dir().join("plugins").join("installed_plugins.json");
        let content = match std::fs::read_to_string(&registry_path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let Some(plugins) = json.get("plugins").and_then(|v| v.as_object()) else {
            return vec![];
        };

        // Also read enabledPlugins from settings.json to know which are enabled
        let enabled_set: std::collections::HashSet<String> = self.read_settings()
            .and_then(|s| s.get("enabledPlugins")?.as_object().cloned())
            .map(|obj| obj.into_iter()
                .filter(|(_, v)| v.as_bool().unwrap_or(false))
                .map(|(k, _)| k)
                .collect())
            .unwrap_or_default();

        let mut entries = Vec::new();
        for (key, installs) in plugins {
            // key format: "plugin-name@marketplace"
            let (name, source) = key.rsplit_once('@')
                .map(|(n, s)| (n.to_string(), s.to_string()))
                .unwrap_or_else(|| (key.clone(), String::new()));

            // installs is an array; take the first entry (user scope)
            let Some(install) = installs.as_array().and_then(|a| a.first()) else { continue };

            let install_path = install.get("installPath")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .and_then(|p| p.parent().map(PathBuf::from)); // strip version component

            let installed_at = install.get("installedAt")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            let updated_at = install.get("lastUpdated")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            entries.push(PluginEntry {
                name,
                source: source.clone(),
                enabled: enabled_set.contains(key),
                path: install_path,
                installed_at,
                updated_at,
            });
        }
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_claude_adapter_name() {
        let adapter = ClaudeAdapter::new();
        assert_eq!(adapter.name(), "claude");
    }

    #[test]
    fn test_claude_detect_with_dir() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let adapter = ClaudeAdapter::with_home(dir.path().to_path_buf());
        assert!(adapter.detect());
    }

    #[test]
    fn test_claude_detect_without_dir() {
        let dir = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_home(dir.path().to_path_buf());
        assert!(!adapter.detect());
    }

    #[test]
    fn test_claude_skill_dirs() {
        let dir = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_home(dir.path().to_path_buf());
        let dirs = adapter.skill_dirs();
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with(".claude/skills"));
    }

    #[test]
    fn test_claude_read_mcp_servers() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"mcpServers":{"github":{"command":"npx","args":["-y","@modelcontextprotocol/server-github"],"env":{"GITHUB_TOKEN":"ghp_test"}}}}"#,
        ).unwrap();
        let adapter = ClaudeAdapter::with_home(dir.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "github");
        assert_eq!(servers[0].command, "npx");
    }

    #[test]
    fn test_claude_read_hooks() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":["echo test"]}]}}"#,
        ).unwrap();
        let adapter = ClaudeAdapter::with_home(dir.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].event, "PreToolUse");
        assert_eq!(hooks[0].command, "echo test");
    }

    #[test]
    fn test_claude_config_methods() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = ClaudeAdapter::with_home(tmp.path().to_path_buf());

        let global_rules = adapter.global_rules_files();
        // Without a rules dir, only CLAUDE.md is returned
        assert_eq!(global_rules.len(), 1);
        assert!(global_rules[0].ends_with("CLAUDE.md"));

        let global_settings = adapter.global_settings_files();
        assert!(global_settings.len() >= 3);
        assert!(global_settings[0].ends_with("settings.json"));
        assert!(global_settings[1].ends_with("settings.local.json"));
        assert!(global_settings[2].ends_with("keybindings.json"));

        let project_rules = adapter.project_rules_patterns();
        assert!(project_rules.contains(&"CLAUDE.md".to_string()));
        assert!(project_rules.contains(&".claude/CLAUDE.md".to_string()));
        assert!(project_rules.contains(&".claude/rules/*.md".to_string()));

        let project_settings = adapter.project_settings_patterns();
        assert!(project_settings.contains(&".claude/settings.json".to_string()));
        assert!(project_settings.contains(&".claude/settings.local.json".to_string()));
        assert!(project_settings.contains(&".mcp.json".to_string()));

        let project_ignore = adapter.project_ignore_patterns();
        assert!(project_ignore.is_empty());
    }
}
