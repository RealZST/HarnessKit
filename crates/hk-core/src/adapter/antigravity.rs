// MCP config reference: https://antigravity.google/docs/mcp
// Skills reference: https://antigravity.google/docs/skills
// Config file: ~/.gemini/config/mcp_config.json (or ~/.gemini/antigravity-cli/mcp_config.json)
// Format: JSON, top-level key "mcpServers", sub-keys: command, args, env, serverUrl, headers, etc.
//
// Data directory note: Antigravity has multiple surfaces:
//   - ~/.gemini/antigravity-cli/  → CLI runtime data (skills, conversations, brain, etc.)
//   - ~/.gemini/config/           → App / IDE customization root (skills, mcp_config.json, rules)
//   - ~/.gemini/skills/           → Shared Gemini/Antigravity skills
//   - ~/.gemini/antigravity/      → Legacy AI runtime path kept for backward compat
//   - ~/.antigravity/             → VS Code-fork IDE shell data (extensions/, argv.json) —
//                                   undocumented by Google, inferred only from product.json
//                                   `dataFolderName: ".antigravity"`. Not used as base_dir.

use super::{AgentAdapter, HookEntry, HookFormat, McpServerEntry, ProjectMarker, RemoteMcpSchema};
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
            home: crate::home_dir(),
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

    /// Candidate runtime and config directories in precedence order:
    /// 1. `~/.gemini/antigravity-cli`: Antigravity CLI runtime data
    /// 2. `~/.gemini/config`: Antigravity App / IDE global customization root
    /// 3. `~/.gemini/antigravity`: Legacy path kept for backward compatibility
    fn candidate_base_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.home.join(".gemini").join("antigravity-cli"),
            self.home.join(".gemini").join("config"),
            self.home.join(".gemini").join("antigravity"),
        ]
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
        for dir in self.candidate_base_dirs() {
            if dir.exists() {
                return dir;
            }
        }
        self.home.join(".gemini").join("antigravity-cli")
    }
    fn detect(&self) -> bool {
        self.candidate_base_dirs().iter().any(|d| d.exists())
    }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        // Antigravity skills can reside in:
        // - Active base directory (antigravity-cli/skills or config/skills)
        // - Antigravity CLI global skills: ~/.gemini/antigravity-cli/skills
        // - Antigravity App / IDE global skills: ~/.gemini/config/skills
        // - Shared Gemini/Antigravity skills: ~/.gemini/skills
        // - Legacy path: ~/.gemini/antigravity/skills
        // Source: https://antigravity.google/docs/skills
        let mut dirs = vec![self.base_dir().join("skills")];

        let candidate_skill_dirs = [
            self.home
                .join(".gemini")
                .join("antigravity-cli")
                .join("skills"),
            self.home.join(".gemini").join("config").join("skills"),
            self.home.join(".gemini").join("skills"),
            self.home.join(".gemini").join("antigravity").join("skills"),
        ];

        for path in candidate_skill_dirs {
            if !dirs.contains(&path) {
                dirs.push(path);
            }
        }

        // Antigravity supports custom skill paths via skills.txt in base directories.
        for base in self.candidate_base_dirs() {
            let skills_txt = base.join("skills.txt");
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
        let config_mcp = self
            .home
            .join(".gemini")
            .join("config")
            .join("mcp_config.json");
        if config_mcp.exists() {
            config_mcp
        } else {
            self.base_dir().join("mcp_config.json")
        }
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
        vec![self.mcp_config_path()]
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
        // Recursive: the language server (2.0.4) walks rule dirs with an
        // unbounded recursive traversal, so nested subdirectories load too.
        vec![
            ".agents/rules/**/*.md".into(),
            ".agent/rules/**/*.md".into(),
        ]
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

    #[test]
    fn detect_with_cli_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let cli_dir = tmp.path().join(".gemini").join("antigravity-cli");
        std::fs::create_dir_all(&cli_dir).unwrap();
        let adapter = AntigravityAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.detect());
        assert_eq!(adapter.base_dir(), cli_dir);
    }

    #[test]
    fn detect_with_config_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg_dir = tmp.path().join(".gemini").join("config");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let adapter = AntigravityAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.detect());
        assert_eq!(adapter.base_dir(), cfg_dir);
    }

    #[test]
    fn detect_with_legacy_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy_dir = tmp.path().join(".gemini").join("antigravity");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        let adapter = AntigravityAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.detect());
        assert_eq!(adapter.base_dir(), legacy_dir);
    }

    #[test]
    fn detect_fails_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = AntigravityAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());
        // Default fallback
        assert_eq!(
            adapter.base_dir(),
            tmp.path().join(".gemini").join("antigravity-cli")
        );
    }

    #[test]
    fn skill_dirs_orders_active_base_first_and_includes_all_candidates() {
        let tmp = tempfile::tempdir().unwrap();
        let cli_dir = tmp.path().join(".gemini").join("antigravity-cli");
        std::fs::create_dir_all(&cli_dir).unwrap();
        let skills_txt = cli_dir.join("skills.txt");
        std::fs::write(&skills_txt, "~/custom-skills\n/opt/extra-skills").unwrap();

        let adapter = AntigravityAdapter::with_home(tmp.path().to_path_buf());
        let dirs = adapter.skill_dirs();

        // Active base skills dir must be first (index 0: write target)
        assert_eq!(dirs[0], cli_dir.join("skills"));

        // All candidate dirs must be included
        assert!(dirs.contains(&cli_dir.join("skills")));
        assert!(dirs.contains(&tmp.path().join(".gemini").join("config").join("skills")));
        assert!(dirs.contains(&tmp.path().join(".gemini").join("skills")));
        assert!(
            dirs.contains(
                &tmp.path()
                    .join(".gemini")
                    .join("antigravity")
                    .join("skills")
            )
        );

        // Custom paths from skills.txt included
        assert!(dirs.contains(&tmp.path().join("custom-skills")));
        assert!(dirs.contains(&PathBuf::from("/opt/extra-skills")));
    }

    #[test]
    fn mcp_config_path_prefers_config_dir_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg_dir = tmp.path().join(".gemini").join("config");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let mcp_json = cfg_dir.join("mcp_config.json");
        std::fs::write(&mcp_json, "{}").unwrap();

        let adapter = AntigravityAdapter::with_home(tmp.path().to_path_buf());
        assert_eq!(adapter.mcp_config_path(), mcp_json);
    }
}
