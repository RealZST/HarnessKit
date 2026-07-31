// MCP config reference: https://antigravity.google/docs/mcp
// Config file: ~/.gemini/antigravity/mcp_config.json
// Format: JSON, top-level key "mcpServers", sub-keys: command, args, env, serverUrl, headers, etc.
//
// Data directory note: Antigravity has TWO directories on disk:
//   - ~/.antigravity/         → VS Code-fork IDE shell data (extensions/, argv.json) —
//                               undocumented by Google, inferred only from product.json
//                               `dataFolderName: ".antigravity"`. Not used as base_dir.
//   - ~/.gemini/antigravity/  → AI agent runtime data (skills, mcp_config.json,
//                               brain/, knowledge/, conversations/) — the path Google's
//                               docs and codelabs actually reference. Used as base_dir.

use super::{
    AgentAdapter, HookEntry, HookFormat, McpServerEntry, ProjectMarker, RemoteMcpSchema,
};
use std::path::{Path, PathBuf};

pub struct AntigravityAdapter {
    home: PathBuf,
}

impl Default for AntigravityAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AntigravityAdapter {
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

impl AgentAdapter for AntigravityAdapter {
    fn hook_format(&self) -> HookFormat {
        HookFormat::None
    }
    fn name(&self) -> &str {
        "antigravity"
    }
    fn needs_path_injection(&self) -> bool {
        true
    }
    fn base_dir(&self) -> PathBuf {
        self.home.join(".gemini").join("antigravity")
    }
    fn detect(&self) -> bool {
        self.base_dir().exists()
    }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        // Antigravity does NOT scan ~/.gemini/skills/ (Gemini CLI's path) or
        // ~/.agents/skills/ — cross-loading from Gemini CLI requires a manual
        // symlink per Google's own guidance.
        // Source: https://codelabs.developers.google.com/getting-started-with-antigravity-skills
        let mut dirs = vec![self.base_dir().join("skills")];

        // Antigravity supports custom skill paths via skills.txt in its base directory.
        let skills_txt = self.base_dir().join("skills.txt");
        if let Ok(content) = std::fs::read_to_string(&skills_txt) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let path = if let Some(stripped) = line.strip_prefix("~/") {
                    self.home.join(stripped)
                } else {
                    PathBuf::from(line)
                };
                if !dirs.contains(&path) {
                    dirs.push(path);
                }
            }
        }

        dirs
    }
    fn project_skill_dirs(&self) -> Vec<String> {
        // `.agents/skills/` (plural) is the current canonical project path;
        // `.agent/skills/` (singular) is still live for the Antigravity CLI,
        // not merely legacy — both load.
        // Source: https://codelabs.developers.google.com/getting-started-with-antigravity-skills
        // (folder rename context: https://discuss.ai.google.dev/t/new-folder-for-rules/126165)
        vec![".agents/skills".into(), ".agent/skills".into()]
    }
    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("mcp_config.json")
    }
    fn hook_config_path(&self) -> PathBuf {
        // Antigravity has no hook system — `hook_format() = None` makes this
        // a dead-code placeholder, never read or written.
        self.base_dir().join("hooks.unused")
    }
    fn plugin_dirs(&self) -> Vec<PathBuf> {
        // Antigravity has no file-based plugin system. The "plugin" surface
        // is VS Code-style VSIX extensions in ~/.antigravity/extensions/, a
        // different extension class than HK's plugin model.
        vec![]
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.home.join(".gemini").join("GEMINI.md")]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("mcp_config.json")]
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        // `.agents/` is canonical (1.18.4+); `.agent/` kept for backward compat.
        vec![
            ProjectMarker::Dir(".agents/rules"),
            ProjectMarker::Dir(".agents/skills"),
            ProjectMarker::Dir(".agent/rules"),
            ProjectMarker::Dir(".agent/skills"),
        ]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        // `.agents/` is canonical (1.18.4+); `.agent/` kept for backward compat.
        // Source: https://discuss.ai.google.dev/t/new-folder-for-rules/126165
        vec![".agents/rules/*.md".into(), ".agent/rules/*.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![]
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
            .map(|(name, val)| {
                // Remote entries: {serverUrl, headers}. Official docs only
                // show Streamable HTTP examples; SSE support is unverified
                // upstream, so treat serverUrl as HTTP.
                let (transport, url) = super::parse_plain_url(val, "serverUrl");
                McpServerEntry {
                    name: name.clone(),
                    command: val
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .into(),
                    args: super::json_string_vec(val, "args"),
                    env: super::json_string_map(val, "env"),
                    transport,
                    url,
                    headers: super::json_string_map(val, "headers"),
                    // Antigravity's MCP schema has no agent-native disable concept.
                    enabled: true,
                }
            })
            .collect()
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::ServerUrl
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AgentAdapter, McpTransport};
    use super::*;

    #[test]
    fn read_mcp_servers_parses_server_url_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("mcp_config.json");
        std::fs::write(
            &config,
            r#"{"mcpServers":{
                "remote":{"serverUrl":"https://example.com/mcp","headers":{"Authorization":"Bearer t"}},
                "fs":{"command":"npx","args":["-y","srv"]}
            }}"#,
        )
        .unwrap();
        let adapter = AntigravityAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers_from(&config);
        let remote = servers.iter().find(|s| s.name == "remote").unwrap();
        assert_eq!(remote.transport, McpTransport::Http);
        assert_eq!(remote.url.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(remote.command, "");
        assert_eq!(remote.headers["Authorization"], "Bearer t");
        let fs = servers.iter().find(|s| s.name == "fs").unwrap();
        assert_eq!(fs.transport, McpTransport::Stdio);
    }

    #[test]
    fn read_hooks_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        // Even with a hooks-like config in the base_dir, Antigravity should return nothing
        let ag_dir = tmp.path().join(".gemini").join("antigravity");
        std::fs::create_dir_all(&ag_dir).unwrap();
        std::fs::write(
            ag_dir.join("settings.json"),
            r#"{"hooks":{"Stop":[{"hooks":["echo fake"]}]}}"#,
        )
        .unwrap();
        let adapter = AntigravityAdapter::with_home(tmp.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert!(hooks.is_empty(), "Antigravity should not support hooks");
    }
}
