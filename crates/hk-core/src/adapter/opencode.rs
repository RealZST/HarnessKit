use super::{AgentAdapter, HookEntry, HookFormat, McpServerEntry, PluginEntry};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct OpencodeAdapter {
    home: PathBuf,
}

impl Default for OpencodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpencodeAdapter {
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

    fn markdown_files(dir: &Path) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return vec![];
        };
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
            .collect()
    }

    fn plugin_name(path: &Path) -> String {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let base = file_name.strip_suffix(".disabled").unwrap_or(&file_name);
        Path::new(base)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| base.to_string())
    }

    fn is_plugin_file(path: &Path) -> bool {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let base = file_name.strip_suffix(".disabled").unwrap_or(&file_name);
        matches!(
            Path::new(base).extension().and_then(|ext| ext.to_str()),
            Some("js" | "ts" | "mjs" | "cjs")
        )
    }

    fn parse_local_mcp_entry(name: &str, value: &serde_json::Value) -> Option<McpServerEntry> {
        if value.get("type").and_then(|v| v.as_str()) != Some("local") {
            return None;
        }

        let (command, args) = match value.get("command") {
            Some(serde_json::Value::Array(parts)) => {
                let mut parts = parts
                    .iter()
                    .filter_map(|part| part.as_str().map(String::from));
                let command = parts.next()?;
                (command, parts.collect())
            }
            Some(serde_json::Value::String(command)) => (command.clone(), vec![]),
            _ => return None,
        };

        let env = value
            .get("environment")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();

        Some(McpServerEntry {
            name: name.to_string(),
            command,
            args,
            env,
        })
    }
}

impl AgentAdapter for OpencodeAdapter {
    fn hook_format(&self) -> HookFormat {
        HookFormat::None
    }

    fn name(&self) -> &str {
        "opencode"
    }

    fn base_dir(&self) -> PathBuf {
        self.home.join(".config").join("opencode")
    }

    fn detect(&self) -> bool {
        self.base_dir().exists()
            || self.mcp_config_path().is_file()
            || crate::scanner::run_which("opencode").is_some()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        vec![".opencode/skills".into()]
    }

    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("opencode.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        self.mcp_config_path()
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("plugins")]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Some(config) = Self::parse_json(path) else {
            return vec![];
        };
        let Some(servers) = config.get("mcp").and_then(|v| v.as_object()) else {
            return vec![];
        };
        servers
            .iter()
            .filter_map(|(name, value)| Self::parse_local_mcp_entry(name, value))
            .collect()
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        vec![]
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        let mut entries = Vec::new();
        for plugin_dir in self.plugin_dirs() {
            let Ok(files) = std::fs::read_dir(plugin_dir) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                if !path.is_file() || !Self::is_plugin_file(&path) {
                    continue;
                }
                let enabled = path.extension().is_none_or(|ext| ext != "disabled");
                entries.push(PluginEntry {
                    name: Self::plugin_name(&path),
                    source: "local".into(),
                    enabled,
                    path: Some(path),
                    uri: None,
                    installed_at: None,
                    updated_at: None,
                });
            }
        }
        entries
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("AGENTS.md")]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.mcp_config_path()];
        files.extend(Self::markdown_files(&self.base_dir().join("agents")));
        files
    }

    fn global_workflow_files(&self) -> Vec<PathBuf> {
        Self::markdown_files(&self.base_dir().join("commands"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    #[test]
    fn detect_accepts_base_dir_or_config_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".config/opencode")).unwrap();
        let adapter = OpencodeAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.detect());

        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".config/opencode");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("opencode.json"), "{}").unwrap();
        let adapter = OpencodeAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.detect());
    }

    #[test]
    fn read_mcp_servers_keeps_only_local_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".config/opencode");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("opencode.json"),
            r#"{
                "mcp": {
                    "local-server": {
                        "type": "local",
                        "command": ["bun", "x", "tool"],
                        "environment": {"TOKEN": "abc"}
                    },
                    "remote-server": {
                        "type": "remote",
                        "url": "https://example.com/mcp"
                    }
                }
            }"#,
        )
        .unwrap();

        let adapter = OpencodeAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "local-server");
        assert_eq!(servers[0].command, "bun");
        assert_eq!(servers[0].args, vec!["x", "tool"]);
        assert_eq!(servers[0].env.get("TOKEN"), Some(&"abc".to_string()));
    }

    #[test]
    fn read_plugins_scans_enabled_and_disabled_files() {
        let tmp = tempfile::tempdir().unwrap();
        let plugins_dir = tmp.path().join(".config/opencode/plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("alpha.ts"), "export default {};").unwrap();
        std::fs::write(
            plugins_dir.join("beta.js.disabled"),
            "module.exports = {};",
        )
        .unwrap();

        let adapter = OpencodeAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 2);
        assert!(plugins.iter().any(|plugin| {
            plugin.name == "alpha" && plugin.enabled && plugin.source == "local"
        }));
        assert!(plugins.iter().any(|plugin| {
            plugin.name == "beta" && !plugin.enabled && plugin.source == "local"
        }));
    }

    #[test]
    fn global_settings_and_workflows_include_markdown_files() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join(".config/opencode");
        std::fs::create_dir_all(base.join("agents")).unwrap();
        std::fs::create_dir_all(base.join("commands")).unwrap();
        std::fs::write(base.join("agents/reviewer.md"), "# reviewer").unwrap();
        std::fs::write(base.join("commands/deploy.md"), "# deploy").unwrap();

        let adapter = OpencodeAdapter::with_home(tmp.path().to_path_buf());
        let settings = adapter.global_settings_files();
        let workflows = adapter.global_workflow_files();
        assert!(settings.iter().any(|path| path.ends_with("agents/reviewer.md")));
        assert!(workflows.iter().any(|path| path.ends_with("commands/deploy.md")));
    }
}