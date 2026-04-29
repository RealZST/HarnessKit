// MCP config reference: https://code.visualstudio.com/docs/copilot/customization/mcp-servers
// Config file: VS Code user profile mcp.json
//   macOS: ~/Library/Application Support/Code/User/mcp.json
//   Linux: ~/.config/Code/User/mcp.json
//   Windows: ~/AppData/Roaming/Code/User/mcp.json
// Format: JSON, top-level key "servers" (NOT "mcpServers"), sub-keys: type, command, args, env, url, headers
//
// Plugin reference (CLI): https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/plugins-finding-installing
// CLI plugins: ~/.copilot/installed-plugins/{marketplace}/{plugin}/, manifest at plugin.json or .plugin/plugin.json
//
// VS Code agent plugins: ~/.vscode/agent-plugins/{domain}/{owner}/{repo}/
// Registry: ~/.vscode/agent-plugins/installed.json
// Plugin manifest: plugins/{name}/.github/plugin/plugin.json
//
// Hook reference: https://code.visualstudio.com/docs/copilot/customization/hooks
// Global hooks: ~/.copilot/hooks/*.json
// Project hooks: .github/hooks/*.json
// Format: {"hooks": {"PreToolUse": [{"type": "command", "command": "...", "timeout": 30}]}}
// Events: SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, PreCompact, SubagentStart, SubagentStop, Stop

use super::{AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, PluginEntry};
use std::path::{Path, PathBuf};

/// Read VS Code agent plugin enablement from state.vscdb.
/// Returns the set of plugin URIs that are disabled (enabled = false).
/// Gracefully returns empty set on any error (DB not found, locked, schema change).
fn read_vscode_disabled_plugins(vscode_user_dir: &Path) -> std::collections::HashSet<String> {
    let db_path = vscode_user_dir
        .join("globalStorage")
        .join("state.vscdb");
    let mut disabled = std::collections::HashSet::new();
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return disabled;
    };
    let Ok(value): Result<String, _> = conn.query_row(
        "SELECT value FROM ItemTable WHERE key = 'agentPlugins.enablement'",
        [],
        |row| row.get(0),
    ) else {
        return disabled;
    };
    // Format: [["file:///path/to/plugin", true/false], ...]
    let Ok(entries) = serde_json::from_str::<Vec<(String, bool)>>(&value) else {
        return disabled;
    };
    for (uri, enabled) in entries {
        if !enabled {
            disabled.insert(uri);
        }
    }
    disabled
}

pub struct CopilotAdapter {
    home: PathBuf,
}

impl Default for CopilotAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CopilotAdapter {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
        }
    }
    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    /// VS Code user profile directory where mcp.json lives.
    /// macOS: ~/Library/Application Support/Code/User
    /// Linux: ~/.config/Code/User
    /// Windows: ~/AppData/Roaming/Code/User
    fn vscode_user_dir(&self) -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            self.home.join("Library/Application Support/Code/User")
        }
        #[cfg(target_os = "windows")]
        {
            self.home.join("AppData/Roaming/Code/User")
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            self.home.join(".config/Code/User")
        }
    }
}

impl AgentAdapter for CopilotAdapter {
    fn hook_format(&self) -> HookFormat {
        HookFormat::Copilot
    }
    fn name(&self) -> &str {
        "copilot"
    }
    fn base_dir(&self) -> PathBuf {
        self.home.join(".copilot")
    }
    fn detect(&self) -> bool {
        self.base_dir().exists()
    }
    fn vscode_user_dir(&self) -> Option<PathBuf> {
        Some(self.vscode_user_dir())
    }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ]
    }
    fn mcp_config_path(&self) -> PathBuf {
        self.vscode_user_dir().join("mcp.json")
    }
    fn mcp_format(&self) -> McpFormat {
        McpFormat::Servers
    }
    // Global hooks: ~/.copilot/hooks/*.json
    // Project hooks: .github/hooks/*.json
    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("hooks").join("hooks.json")
    }
    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("plugins"),
            self.home.join(".vscode").join("agent-plugins"),
        ]
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("copilot-instructions.md")]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files = vec![
            self.base_dir().join("config.json"),
            self.vscode_user_dir().join("mcp.json"),
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
        // ~/.copilot/hooks/*.json
        let hooks_dir = self.base_dir().join("hooks");
        if let Ok(entries) = std::fs::read_dir(&hooks_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "json") {
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
            ".github/hooks/*.json".into(),
        ]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".copilotignore".into()]
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        let mut entries = Vec::new();

        // 1. Copilot CLI plugins: ~/.copilot/installed-plugins/{marketplace}/{plugin}/
        //    CLI has no native enable/disable — always enabled if present.
        let cli_base = self.base_dir().join("installed-plugins");
        if let Ok(marketplaces) = std::fs::read_dir(&cli_base) {
            for marketplace in marketplaces.flatten() {
                if !marketplace.path().is_dir() { continue; }
                let mp_name = marketplace.file_name().to_string_lossy().to_string();
                let Ok(plugins) = std::fs::read_dir(marketplace.path()) else { continue };
                for plugin in plugins.flatten() {
                    if !plugin.path().is_dir() { continue; }
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
                        uri: None,
                        installed_at: None,
                        updated_at: None,
                    });
                }
            }
        }

        // 2. VS Code agent plugins: ~/.vscode/agent-plugins/installed.json
        //    Each entry has pluginUri pointing to plugins/{name}/ with .github/plugin/plugin.json
        //    Enable/disable state stored in VS Code's state.vscdb under "agentPlugins.enablement".
        let vscode_base = self.home.join(".vscode").join("agent-plugins");
        let installed_json = vscode_base.join("installed.json");

        // Read VS Code agent plugin enablement state from state.vscdb
        // Format: [["file:///path/to/plugin", true/false], ...]
        let disabled_uris = read_vscode_disabled_plugins(&self.vscode_user_dir());

        if let Ok(content) = std::fs::read_to_string(&installed_json)
            && let Ok(registry) = serde_json::from_str::<serde_json::Value>(&content)
            && let Some(installed) = registry.get("installed").and_then(|v| v.as_array())
        {
            for item in installed {
                let marketplace = item.get("marketplace")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let plugin_uri = item.get("pluginUri")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                // pluginUri is file:///path/to/plugin
                let plugin_path = PathBuf::from(
                    plugin_uri.strip_prefix("file://").unwrap_or(plugin_uri)
                );
                if !plugin_path.is_dir() { continue; }
                // Try .github/plugin/plugin.json for name
                let manifest = plugin_path.join(".github").join("plugin").join("plugin.json");
                let name = std::fs::read_to_string(&manifest).ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                    .unwrap_or_else(|| {
                        plugin_path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| marketplace.to_string())
                    });
                let enabled = !disabled_uris.contains(plugin_uri);
                entries.push(PluginEntry {
                    name,
                    source: marketplace.to_string(),
                    enabled,
                    path: Some(plugin_path),
                    uri: Some(plugin_uri.to_string()),
                    installed_at: None,
                    updated_at: None,
                });
            }
        }

        entries
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let content = std::fs::read_to_string(path).ok();
        let config: Option<serde_json::Value> = content.and_then(|c| serde_json::from_str(&c).ok());
        let Some(config) = config else { return vec![] };
        let Some(servers) = config.get("servers").and_then(|v| v.as_object()) else {
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
        super::hook_events::to_copilot(event)
    }

    /// Copilot's hooks live across multiple JSON files inside
    /// `~/.copilot/hooks/`, so the `_from(path)` abstraction (one path → one
    /// file) doesn't fit. Delegate to the dir-scanning `read_hooks()` to keep
    /// behavior consistent for callers (delete/get_content/install_to_agent)
    /// that switched to `read_hooks_from(hook_config_path_for(scope))`.
    fn read_hooks_from(&self, _path: &std::path::Path) -> Vec<HookEntry> {
        self.read_hooks()
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        // Scan all JSON files in ~/.copilot/hooks/
        let hooks_dir = self.base_dir().join("hooks");
        let Ok(files) = std::fs::read_dir(&hooks_dir) else {
            return vec![];
        };
        let mut entries = Vec::new();
        for file in files.flatten() {
            let path = file.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let config: serde_json::Value = match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Some(hooks) = config.get("hooks").and_then(|v| v.as_object()) else {
                continue;
            };
            for (event, hook_list) in hooks {
                let Some(arr) = hook_list.as_array() else {
                    continue;
                };
                for hook in arr {
                    // Copilot format: {"type": "command", "command": "..."} or platform-specific {"osx": "...", "linux": "...", "windows": "..."}
                    let cmd = hook
                        .get("command")
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            #[cfg(target_os = "macos")]
                            { hook.get("osx").and_then(|v| v.as_str()) }
                            #[cfg(target_os = "windows")]
                            { hook.get("windows").and_then(|v| v.as_str()) }
                            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                            { hook.get("linux").and_then(|v| v.as_str()) }
                        })
                        .or_else(|| hook.get("bash").and_then(|v| v.as_str()));
                    if let Some(cmd_str) = cmd {
                        entries.push(HookEntry {
                            event: event.clone(),
                            matcher: None,
                            command: cmd_str.to_string(),
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
    fn read_hooks_copilot_format() {
        let tmp = tempfile::tempdir().unwrap();
        let hooks_dir = tmp.path().join(".copilot").join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        std::fs::write(hooks_dir.join("notify.json"),
            r#"{"version":1,"hooks":{"PreToolUse":[{"type":"command","command":"./check.sh","timeout":30}]}}"#
        ).unwrap();
        let adapter = CopilotAdapter::with_home(tmp.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].event, "PreToolUse");
        assert_eq!(hooks[0].command, "./check.sh");
    }

    #[test]
    fn read_hooks_multiple_files() {
        let tmp = tempfile::tempdir().unwrap();
        let hooks_dir = tmp.path().join(".copilot").join("hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        std::fs::write(
            hooks_dir.join("a.json"),
            r#"{"version":1,"hooks":{"SessionStart":[{"type":"command","command":"echo start"}]}}"#,
        )
        .unwrap();
        std::fs::write(
            hooks_dir.join("b.json"),
            r#"{"version":1,"hooks":{"Stop":[{"type":"command","command":"echo end"}]}}"#,
        )
        .unwrap();
        let adapter = CopilotAdapter::with_home(tmp.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 2);
    }

    #[test]
    fn read_vscode_agent_plugins() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        // Set up ~/.vscode/agent-plugins structure
        let plugin_dir = home.join(".vscode/agent-plugins/github.com/user/my-repo/plugins/my-plugin");
        let manifest_dir = plugin_dir.join(".github/plugin");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(
            manifest_dir.join("plugin.json"),
            r#"{"name": "my-plugin", "description": "A test plugin", "version": "1.0.0"}"#,
        ).unwrap();

        // Write installed.json
        let installed = home.join(".vscode/agent-plugins/installed.json");
        std::fs::write(
            &installed,
            format!(
                r#"{{"version":1,"installed":[{{"pluginUri":"file://{}","marketplace":"user/my-repo"}}]}}"#,
                plugin_dir.to_string_lossy()
            ),
        ).unwrap();

        let adapter = CopilotAdapter::with_home(home.to_path_buf());
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "my-plugin");
        assert_eq!(plugins[0].source, "user/my-repo");
        assert!(plugins[0].path.is_some());
    }
}
