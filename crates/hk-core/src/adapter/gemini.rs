use super::{AgentAdapter, HookEntry, McpServerEntry, PluginEntry};
use std::path::PathBuf;

pub struct GeminiAdapter { home: PathBuf }

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiAdapter {
    pub fn new() -> Self { Self { home: dirs::home_dir().unwrap_or_default() } }
    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self { Self { home } }
    fn read_settings(&self) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(self.base_dir().join("settings.json")).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for GeminiAdapter {
    fn name(&self) -> &str { "gemini" }
    fn base_dir(&self) -> PathBuf { self.home.join(".gemini") }
    fn detect(&self) -> bool { self.base_dir().exists() }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ]
    }
    fn mcp_config_path(&self) -> PathBuf { self.base_dir().join("settings.json") }
    fn hook_config_path(&self) -> PathBuf { self.base_dir().join("settings.json") }
    fn plugin_dirs(&self) -> Vec<PathBuf> { vec![self.base_dir().join("plugins")] }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("GEMINI.md")]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files = vec![
            self.base_dir().join("settings.json"),
            self.base_dir().join(".env"),
        ];
        // ~/.gemini/commands/*.toml
        let commands_dir = self.base_dir().join("commands");
        if let Ok(entries) = std::fs::read_dir(&commands_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "toml") {
                    files.push(p);
                }
            }
        }
        // ~/.gemini/agents/*.md
        let agents_dir = self.base_dir().join("agents");
        if let Ok(entries) = std::fs::read_dir(&agents_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "md") {
                    files.push(p);
                }
            }
        }
        // ~/.gemini/policies/*.toml
        let policies_dir = self.base_dir().join("policies");
        if let Ok(entries) = std::fs::read_dir(&policies_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "toml") {
                    files.push(p);
                }
            }
        }
        files
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec!["GEMINI.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".gemini/settings.json".into()]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".geminiignore".into()]
    }

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

    fn read_plugins(&self) -> Vec<PluginEntry> {
        // Gemini extensions: ~/.gemini/extensions/{name}/gemini-extension.json
        let ext_dir = self.base_dir().join("extensions");
        let Ok(dirs) = std::fs::read_dir(&ext_dir) else { return vec![] };
        let mut entries = Vec::new();
        for dir in dirs.flatten() {
            if !dir.path().is_dir() { continue; }
            let manifest = dir.path().join("gemini-extension.json");
            let name = if manifest.exists() {
                std::fs::read_to_string(&manifest).ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                    .unwrap_or_else(|| dir.file_name().to_string_lossy().to_string())
            } else {
                continue; // not a valid extension
            };
            entries.push(PluginEntry {
                name,
                source: "gemini".into(),
                enabled: true,
                path: Some(dir.path()),
                installed_at: None,
                updated_at: None,
            });
        }
        entries
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
