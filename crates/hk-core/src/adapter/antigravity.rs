// MCP config reference: https://antigravity.google/docs/mcp
// Skills reference: https://antigravity.google/docs/skills
// Discovery reference: https://antigravity.google/docs/customization-discovery
// Config file: ~/.gemini/config/mcp_config.json
// Format: JSON, top-level key "mcpServers", sub-keys: command, args, env, serverUrl, headers, etc.
//
// Data directory note: since Antigravity 2.0 there are three surfaces (App,
// IDE, CLI `agy`) sharing one customization root:
//   - ~/.gemini/config/           → shared root every surface reads: skills/,
//                                   mcp_config.json, rules/, plugins/, hooks.json.
//                                   The only place a skill or MCP server is
//                                   visible to all three, so it is the base_dir
//                                   and the install target.
//   - ~/.gemini/antigravity-cli/  → the CLI's private app data (settings.json,
//                                   cache, logs). Its skills/ and ~/.gemini/skills/
//                                   (Gemini CLI's dir) are extra read locations
//                                   for the CLI only — "which the Antigravity App
//                                   doesn't read" per the skills docs. agy's own
//                                   first-run migration moves antigravity-cli/skills
//                                   into config/skills.
//   - ~/.gemini/antigravity/      → the App's app data (brain/, conversations/,
//                                   mcp_oauth_tokens.json) and the pre-2.0 global
//                                   root; its skills/ is still read as legacy.
//   - ~/.gemini/antigravity-ide/  → the IDE's app data. Not a skills root.
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

    /// `~/.gemini/config/`: the customization root shared by the App, the IDE
    /// and the CLI. Skills and MCP servers are installed here.
    fn config_dir(&self) -> PathBuf {
        self.home.join(".gemini").join("config")
    }

    /// `~/.gemini/antigravity-cli/`: the CLI's private app-data directory.
    /// Its presence is what tells us the CLI is installed.
    fn cli_dir(&self) -> PathBuf {
        self.home.join(".gemini").join("antigravity-cli")
    }

    /// `~/.gemini/antigravity/`: the App's app-data directory and the global
    /// root before 2.0 (skills/, mcp_config.json).
    fn legacy_dir(&self) -> PathBuf {
        self.home.join(".gemini").join("antigravity")
    }

    /// Every directory whose existence means some Antigravity surface is
    /// installed. `detect()` and the `skills.txt` lookup walk these.
    fn data_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.config_dir(),
            self.cli_dir(),
            self.legacy_dir(),
            self.home.join(".gemini").join("antigravity-ide"),
        ]
    }

    /// Where the global MCP config is read from: the shared file, or the
    /// pre-2.0 file when the shared one does not exist yet. The 2.0 upgrade
    /// turned the legacy file into a symlink to the shared one, so on
    /// migrated machines both names resolve to the same content.
    fn mcp_read_path(&self) -> PathBuf {
        let shared = self.mcp_config_path();
        if shared.exists() {
            return shared;
        }
        let legacy = self.legacy_dir().join("mcp_config.json");
        if legacy.exists() { legacy } else { shared }
    }

    /// Append `path` unless an equivalent directory is already listed. Paths
    /// are compared after `canonicalize` so a legacy `antigravity/skills`
    /// symlinked onto `config/skills` is scanned once, not twice.
    fn push_unique(dirs: &mut Vec<PathBuf>, path: PathBuf) {
        let resolved = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let candidate = resolved(&path);
        if dirs.iter().any(|d| resolved(d) == candidate) {
            return;
        }
        dirs.push(path);
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
        // Always the shared root, whichever surfaces are installed: it is the
        // one directory the App, the IDE and the CLI all read, and the CLI
        // migrates its own antigravity-cli/skills into it on first launch.
        self.config_dir()
    }
    fn detect(&self) -> bool {
        self.data_dirs().iter().any(|d| d.exists())
    }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        // Index 0 is the install target: ~/.gemini/config/skills, the only
        // global skills folder every surface reads. The rest are read-only:
        // - ~/.gemini/antigravity/skills — pre-2.0 global root, still loaded
        //   as legacy.
        // - ~/.gemini/antigravity-cli/skills and ~/.gemini/skills — loaded by
        //   the CLI only ("which the Antigravity App doesn't read"), so they
        //   are listed only when the CLI is installed; otherwise every Gemini
        //   CLI skill would show up under Antigravity for App-only users.
        // Source: https://antigravity.google/docs/skills
        let mut dirs = vec![self.config_dir().join("skills")];
        Self::push_unique(&mut dirs, self.legacy_dir().join("skills"));
        if self.cli_dir().exists() {
            Self::push_unique(&mut dirs, self.cli_dir().join("skills"));
            Self::push_unique(&mut dirs, self.home.join(".gemini").join("skills"));
        }

        // Antigravity supports custom skill paths via skills.txt in its data directories.
        for base in self.data_dirs() {
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
                    Self::push_unique(&mut dirs, path);
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
        // The global MCP file every surface reads; `agy mcp add` writes it too.
        // Never antigravity-cli/mcp_config.json — the CLI stopped reading that.
        self.config_dir().join("mcp_config.json")
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
        vec![self.mcp_read_path()]
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
        self.read_mcp_servers_from(&self.mcp_read_path())
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

    fn home_with(dirs: &[&str]) -> (tempfile::TempDir, AntigravityAdapter) {
        let tmp = tempfile::tempdir().unwrap();
        for d in dirs {
            std::fs::create_dir_all(tmp.path().join(".gemini").join(d)).unwrap();
        }
        let adapter = AntigravityAdapter::with_home(tmp.path().to_path_buf());
        (tmp, adapter)
    }

    #[test]
    fn detects_each_surface_but_always_installs_into_the_shared_root() {
        for surface in [
            "antigravity-cli",
            "config",
            "antigravity",
            "antigravity-ide",
        ] {
            let (tmp, adapter) = home_with(&[surface]);
            assert!(
                adapter.detect(),
                "{surface} alone should count as installed"
            );
            assert_eq!(
                adapter.base_dir(),
                tmp.path().join(".gemini").join("config"),
                "{surface}: the install root is the shared config dir, not the surface's own"
            );
            assert_eq!(adapter.skill_dirs()[0], adapter.base_dir().join("skills"));
        }
    }

    #[test]
    fn detect_fails_when_empty() {
        let (tmp, adapter) = home_with(&[]);
        assert!(!adapter.detect());
        // Default fallback stays the shared root
        assert_eq!(
            adapter.base_dir(),
            tmp.path().join(".gemini").join("config")
        );
    }

    #[test]
    fn app_only_install_does_not_list_the_cli_only_folders() {
        // The App never reads ~/.gemini/skills or antigravity-cli/skills, so
        // without a CLI install they must stay out — otherwise every Gemini
        // CLI skill would be attributed to Antigravity.
        let (tmp, adapter) = home_with(&["config", "antigravity", "skills"]);
        let dirs = adapter.skill_dirs();
        let gemini = tmp.path().join(".gemini");
        assert_eq!(dirs[0], gemini.join("config").join("skills"));
        assert!(dirs.contains(&gemini.join("antigravity").join("skills")));
        assert!(!dirs.contains(&gemini.join("skills")));
        assert!(!dirs.contains(&gemini.join("antigravity-cli").join("skills")));
    }

    #[test]
    fn cli_install_adds_its_two_read_only_folders_after_the_shared_root() {
        let (tmp, adapter) = home_with(&["antigravity-cli"]);
        let gemini = tmp.path().join(".gemini");
        std::fs::write(
            gemini.join("antigravity-cli").join("skills.txt"),
            "~/custom-skills\n/opt/extra-skills",
        )
        .unwrap();

        let dirs = adapter.skill_dirs();
        // Install target first, even though only the CLI is present
        assert_eq!(dirs[0], gemini.join("config").join("skills"));
        assert!(dirs.contains(&gemini.join("antigravity-cli").join("skills")));
        assert!(dirs.contains(&gemini.join("skills")));
        assert!(dirs.contains(&gemini.join("antigravity").join("skills")));
        // Custom paths from skills.txt included
        assert!(dirs.contains(&tmp.path().join("custom-skills")));
        assert!(dirs.contains(&PathBuf::from("/opt/extra-skills")));
    }

    #[cfg(unix)]
    #[test]
    fn legacy_skills_symlinked_onto_the_shared_root_are_listed_once() {
        // The 2.0 upgrade leaves ~/.gemini/antigravity/skills pointing at
        // ~/.gemini/config/skills on many machines; scan it once.
        let (tmp, adapter) = home_with(&["config/skills", "antigravity"]);
        let gemini = tmp.path().join(".gemini");
        std::os::unix::fs::symlink(
            gemini.join("config").join("skills"),
            gemini.join("antigravity").join("skills"),
        )
        .unwrap();

        let dirs = adapter.skill_dirs();
        assert_eq!(dirs, vec![gemini.join("config").join("skills")]);
    }

    #[test]
    fn mcp_config_is_always_written_to_the_shared_root() {
        // Even when only the CLI is installed and no shared file exists yet:
        // antigravity-cli/mcp_config.json is read by no surface anymore.
        let (tmp, adapter) = home_with(&["antigravity-cli"]);
        assert_eq!(
            adapter.mcp_config_path(),
            tmp.path()
                .join(".gemini")
                .join("config")
                .join("mcp_config.json")
        );
    }

    #[test]
    fn mcp_servers_fall_back_to_the_legacy_file_until_the_shared_one_exists() {
        let (tmp, adapter) = home_with(&["antigravity", "config"]);
        let gemini = tmp.path().join(".gemini");
        let legacy = gemini.join("antigravity").join("mcp_config.json");
        std::fs::write(&legacy, r#"{"mcpServers":{"old":{"command":"a"}}}"#).unwrap();
        assert_eq!(adapter.global_settings_files(), vec![legacy.clone()]);
        assert_eq!(adapter.read_mcp_servers()[0].name, "old");

        // Once the shared file exists it wins, whatever the legacy file says.
        let shared = gemini.join("config").join("mcp_config.json");
        std::fs::write(&shared, r#"{"mcpServers":{"new":{"command":"b"}}}"#).unwrap();
        assert_eq!(adapter.global_settings_files(), vec![shared]);
        assert_eq!(adapter.read_mcp_servers()[0].name, "new");
    }
}
