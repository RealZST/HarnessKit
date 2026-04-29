// MCP config reference:
//   https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md
//   https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/tutorials/mcp-setup.md
// Config file: ~/.gemini/settings.json
// Format: JSON, top-level key "mcpServers", sub-keys: command, args, env, url, httpUrl, headers
//
// Extension (plugin) reference: https://geminicli.com/docs/extensions/
// Extensions: ~/.gemini/extensions/{name}/, manifest at gemini-extension.json

use super::{AgentAdapter, HookEntry, McpServerEntry, PluginEntry};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Read extension-enablement.json and return the set of extension names disabled at user scope.
/// Gemini stores overrides as path-based rules; a rule starting with `!` means disabled.
/// We check for `!{homedir}/*` to determine user-level disabled state.
fn read_disabled_extensions(ext_dir: &Path, home: &Path) -> HashSet<String> {
    let mut disabled = HashSet::new();
    let enablement_path = ext_dir.join("extension-enablement.json");
    let Ok(content) = std::fs::read_to_string(&enablement_path) else {
        return disabled;
    };
    let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) else {
        return disabled;
    };
    let home_prefix = format!("!{}/*", home.to_string_lossy());
    let Some(obj) = config.as_object() else {
        return disabled;
    };
    for (name, val) in obj {
        let Some(overrides) = val.get("overrides").and_then(|v| v.as_array()) else {
            continue;
        };
        // "Last matching rule wins" — check last rule that matches user scope
        let is_disabled = overrides.iter().rev().find_map(|rule| {
            let s = rule.as_str()?;
            if s == home_prefix {
                Some(true) // disabled at user scope
            } else if s == &home_prefix[1..] {
                Some(false) // enabled at user scope (without `!`)
            } else {
                None // not a user-scope rule, skip
            }
        });
        if is_disabled == Some(true) {
            disabled.insert(name.clone());
        }
    }
    disabled
}

pub struct GeminiAdapter {
    home: PathBuf,
}

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiAdapter {
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

impl AgentAdapter for GeminiAdapter {
    fn name(&self) -> &str {
        "gemini"
    }
    fn base_dir(&self) -> PathBuf {
        self.home.join(".gemini")
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
        self.base_dir().join("settings.json")
    }
    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("settings.json")
    }
    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("extensions")]
    }

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
        vec![
            "GEMINI.md".into(),
            "*/GEMINI.md".into(),
            "*/*/GEMINI.md".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".gemini/settings.json".into()]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".geminiignore".into()]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Some(settings) = Self::parse_json(path) else {
            return vec![];
        };
        let Some(servers) = settings.get("mcpServers").and_then(|v| v.as_object()) else {
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

    fn read_plugins(&self) -> Vec<PluginEntry> {
        // Gemini extensions: ~/.gemini/extensions/{name}/gemini-extension.json
        let ext_dir = self.base_dir().join("extensions");
        let Ok(dirs) = std::fs::read_dir(&ext_dir) else {
            return vec![];
        };
        let disabled_set = read_disabled_extensions(&ext_dir, &self.home);
        let mut entries = Vec::new();
        for dir in dirs.flatten() {
            if !dir.path().is_dir() {
                continue;
            }
            let manifest = dir.path().join("gemini-extension.json");
            let name = if manifest.exists() {
                std::fs::read_to_string(&manifest)
                    .ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                    .unwrap_or_else(|| dir.file_name().to_string_lossy().to_string())
            } else {
                continue; // not a valid extension
            };
            let enabled = !disabled_set.contains(&name);
            entries.push(PluginEntry {
                name,
                source: "gemini".into(),
                enabled,
                path: Some(dir.path()),
                uri: None,
                installed_at: None,
                updated_at: None,
            });
        }
        entries
    }

    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_gemini(event)
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        self.read_hooks_from(&self.hook_config_path())
    }

    fn read_hooks_from(&self, path: &Path) -> Vec<HookEntry> {
        let Some(settings) = Self::parse_json(path) else {
            return vec![];
        };
        let Some(hooks) = settings.get("hooks").and_then(|v| v.as_object()) else {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_extension(tmp: &std::path::Path, name: &str) {
        let ext_dir = tmp.join(".gemini").join("extensions").join(name);
        std::fs::create_dir_all(&ext_dir).unwrap();
        let manifest = serde_json::json!({ "name": name, "version": "1.0.0" });
        std::fs::write(ext_dir.join("gemini-extension.json"), manifest.to_string()).unwrap();
    }

    #[test]
    fn read_plugins_finds_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        setup_extension(tmp.path(), "my-ext");
        let adapter = GeminiAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "my-ext");
        assert!(plugins[0].enabled);
    }

    #[test]
    fn read_plugins_skips_dirs_without_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let no_manifest = tmp.path().join(".gemini").join("extensions").join("stray-dir");
        std::fs::create_dir_all(&no_manifest).unwrap();
        let adapter = GeminiAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.read_plugins().is_empty());
    }

    #[test]
    fn read_plugins_detects_disabled_extension() {
        let tmp = tempfile::tempdir().unwrap();
        setup_extension(tmp.path(), "disabled-ext");
        setup_extension(tmp.path(), "enabled-ext");

        // Write enablement file with disabled-ext disabled at user scope
        let home_str = tmp.path().to_string_lossy();
        let enablement = serde_json::json!({
            "disabled-ext": { "overrides": [format!("!{}/*", home_str)] },
            "enabled-ext": { "overrides": [format!("{}/*", home_str)] },
        });
        let ext_dir = tmp.path().join(".gemini").join("extensions");
        std::fs::write(
            ext_dir.join("extension-enablement.json"),
            enablement.to_string(),
        ).unwrap();

        let adapter = GeminiAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 2);

        let disabled = plugins.iter().find(|p| p.name == "disabled-ext").unwrap();
        let enabled = plugins.iter().find(|p| p.name == "enabled-ext").unwrap();
        assert!(!disabled.enabled);
        assert!(enabled.enabled);
    }

    #[test]
    fn read_plugins_defaults_enabled_when_no_enablement_file() {
        let tmp = tempfile::tempdir().unwrap();
        setup_extension(tmp.path(), "some-ext");
        // No extension-enablement.json
        let adapter = GeminiAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].enabled);
    }

    #[test]
    fn read_plugins_defaults_enabled_when_not_in_enablement_file() {
        let tmp = tempfile::tempdir().unwrap();
        setup_extension(tmp.path(), "my-ext");

        // Enablement file exists but doesn't mention this extension
        let ext_dir = tmp.path().join(".gemini").join("extensions");
        std::fs::write(
            ext_dir.join("extension-enablement.json"),
            r#"{"other-ext": {"overrides": ["!/some/path/*"]}}"#,
        ).unwrap();

        let adapter = GeminiAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert!(plugins[0].enabled);
    }

    #[test]
    fn plugin_dirs_points_to_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = GeminiAdapter::with_home(tmp.path().to_path_buf());
        let dirs = adapter.plugin_dirs();
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with(".gemini/extensions"));
    }

    #[test]
    fn read_disabled_extensions_last_matching_rule_wins() {
        let tmp = tempfile::tempdir().unwrap();
        setup_extension(tmp.path(), "toggled-ext");

        let home_str = tmp.path().to_string_lossy();
        // First disabled, then re-enabled at user scope — last rule wins
        let enablement = serde_json::json!({
            "toggled-ext": { "overrides": [
                format!("!{}/*", home_str),
                format!("{}/*", home_str),
            ]}
        });
        let ext_dir = tmp.path().join(".gemini").join("extensions");
        std::fs::write(
            ext_dir.join("extension-enablement.json"),
            enablement.to_string(),
        ).unwrap();

        let adapter = GeminiAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert!(plugins[0].enabled, "last rule is enable, should be enabled");
    }
}
