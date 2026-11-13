// MCP config reference: https://code.claude.com/docs/en/mcp
// Config file: ~/.claude.json top-level "mcpServers" (user scope), .mcp.json (project scope)
// Format: JSON, top-level key "mcpServers", sub-keys: command, args, env, type, url, headers
//
// Plugin reference:
//   https://code.claude.com/docs/en/discover-plugins
//   https://code.claude.com/docs/en/plugins
// Plugins: ~/.claude/plugins/, registry at installed_plugins.json, manifest at .claude-plugin/plugin.json
// Each plugin key maps to an array of installs: one at `user` scope (no
// projectPath), and one per project at `project` / `local` scope, carrying the
// `projectPath` it belongs to.
// `enabledPlugins` lives in the matching settings file: ~/.claude/settings.json,
// <project>/.claude/settings.json, <project>/.claude/settings.local.json. Inside
// a project the three merge per key, local over project over user.

use super::{AgentAdapter, HookEntry, McpServerEntry, PluginEntry, ProjectMarker, RemoteMcpSchema};
use std::path::{Path, PathBuf};

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

    /// The real working directory a Claude session ran in, read from the
    /// first `*.jsonl` transcript in `project_dir`. Every transcript line
    /// carries a `"cwd"` field; we read lazily (line by line) and return the
    /// first parseable one, so a multi-MB transcript costs one line. Returns
    /// `None` when there is no transcript or no `cwd` — caller treats that as
    /// an unknown owner.
    fn session_cwd(project_dir: &Path) -> Option<PathBuf> {
        use std::io::BufRead;
        let entries = std::fs::read_dir(project_dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                let Ok(file) = std::fs::File::open(&path) else {
                    continue;
                };
                for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
                    if let Some(cwd) = serde_json::from_str::<serde_json::Value>(&line)
                        .ok()
                        .and_then(|v| v.get("cwd").and_then(|c| c.as_str()).map(PathBuf::from))
                    {
                        return Some(cwd);
                    }
                }
            }
        }
        None
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
        self.home.join(".claude.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("settings.json")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("plugins")]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        // MCP servers are in ~/.claude.json (not settings.json)
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
                // Remote entries: {type: "http"|"sse", url, headers} — no command key.
                let (transport, url) = super::parse_type_url(val);
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
                    // Claude's MCP schema has no agent-native disable concept.
                    enabled: true,
                }
            })
            .collect()
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::TypeAndUrl
    }

    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_claude(event)
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        self.read_hooks_from(&self.hook_config_path())
    }

    fn read_hooks_from(&self, path: &Path) -> Vec<HookEntry> {
        super::read_claude_like_hooks(path)
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.base_dir().join("CLAUDE.md")];
        // ~/.claude/rules/**/*.md — Claude Code discovers rules recursively
        // and follows symlinked rule directories.
        files.extend(super::files_with_ext_recursive(
            &self.base_dir().join("rules"),
            "md",
        ));
        files
    }

    fn external_project_memory(&self) -> Vec<(Option<PathBuf>, Vec<PathBuf>)> {
        // ~/.claude/projects/<encoded-cwd>/memory/*.md — one group per project
        // dir, tagged with the real cwd from its session transcript. The
        // encoded dir name is lossy (`/`, space and `~` all collapse to `-`),
        // so we read the cwd instead of decoding the name. Single pass.
        let projects_dir = self.base_dir().join("projects");
        let Ok(entries) = std::fs::read_dir(&projects_dir) else {
            return vec![];
        };
        let mut groups = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            let files: Vec<PathBuf> = super::files_with_ext(&dir.join("memory"), "md").collect();
            if files.is_empty() {
                continue;
            }
            groups.push((Self::session_cwd(&dir), files));
        }
        groups
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files = vec![
            self.home.join(".claude.json"),
            self.base_dir().join("settings.json"),
            self.base_dir().join("settings.local.json"),
            self.base_dir().join("keybindings.json"),
        ];
        // ~/.claude/commands/*.md (legacy, still functional)
        files.extend(super::files_with_ext(
            &self.base_dir().join("commands"),
            "md",
        ));
        // ~/.claude/output-styles/*.md
        files.extend(super::files_with_ext(
            &self.base_dir().join("output-styles"),
            "md",
        ));
        files
    }

    fn global_subagent_files(&self) -> Vec<PathBuf> {
        // ~/.claude/agents/*.md
        super::files_with_ext(&self.base_dir().join("agents"), "md").collect()
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        vec![
            ProjectMarker::Dir(".claude"),
            ProjectMarker::File(".mcp.json"),
        ]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            "CLAUDE.md".into(),
            ".claude/CLAUDE.md".into(),
            ".claude/rules/**/*.md".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![
            ".claude/settings.json".into(),
            ".claude/settings.local.json".into(),
            ".mcp.json".into(),
        ]
    }

    fn project_subagent_patterns(&self) -> Vec<String> {
        vec![".claude/agents/*.md".into()]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![] // Claude Code does NOT have .claudeignore
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        vec![".claude/skills".into()]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        Some(".mcp.json".into())
    }

    fn project_hook_config_relpath(&self) -> Option<String> {
        // Claude project hooks live in `.claude/settings.json` alongside other settings.
        Some(".claude/settings.json".into())
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        self.read_installed_plugins(None)
    }

    fn read_project_plugins(&self, project: &Path) -> Vec<PluginEntry> {
        self.read_installed_plugins(Some(project))
    }

    fn plugins_without_global_install(&self) -> Vec<(String, String)> {
        let registry = Self::parse_json(&installed_plugins_file(&self.base_dir()));
        let plugins = registry
            .as_ref()
            .and_then(|json| json.get("plugins")?.as_object());
        plugins
            .into_iter()
            .flatten()
            .filter(|(_, installs)| {
                installs
                    .as_array()
                    .is_some_and(|all| !all.is_empty() && all.iter().all(is_project_install))
            })
            .map(|(key, _)| split_plugin_key(key))
            .collect()
    }
}

/// `plugin-name@marketplace` as `(name, source)`.
fn split_plugin_key(key: &str) -> (String, String) {
    key.rsplit_once('@')
        .map(|(name, source)| (name.to_string(), source.to_string()))
        .unwrap_or_else(|| (key.to_string(), String::new()))
}

/// `enabledPlugins` of one settings file, as `key → on/off`. Empty when the
/// file is missing or has no such object.
fn read_enabled_plugins(settings_path: &Path) -> std::collections::HashMap<String, bool> {
    ClaudeAdapter::parse_json(settings_path)
        .and_then(|s| s.get("enabledPlugins")?.as_object().cloned())
        .map(|obj| {
            obj.into_iter()
                .filter_map(|(key, value)| Some((key, value.as_bool()?)))
                .collect()
        })
        .unwrap_or_default()
}

/// Claude's own record of installed plugins, under its `base_dir`.
pub(crate) fn installed_plugins_file(base_dir: &Path) -> PathBuf {
    base_dir.join("plugins").join("installed_plugins.json")
}

/// The scope an `installed_plugins.json` record names, if any.
fn install_scope(install: &serde_json::Value) -> Option<&str> {
    install.get("scope").and_then(|v| v.as_str())
}

/// Whether an `installed_plugins.json` record is tied to a project. Records
/// written before scopes existed carry no scope and are user-level.
fn is_project_install(install: &serde_json::Value) -> bool {
    matches!(install_scope(install), Some("project" | "local"))
}

/// Whether an `installed_plugins.json` record is an install for `project`.
/// Claude records `projectPath` resolved (`/private/tmp/x` for `/tmp/x`),
/// hence the symlink-aware comparison.
pub(crate) fn is_install_for_project(install: &serde_json::Value, project: &Path) -> bool {
    is_project_install(install)
        && install
            .get("projectPath")
            .and_then(|v| v.as_str())
            .is_some_and(|path| crate::paths_equal(Path::new(path), project))
}

/// The settings files through which a project switches plugins on or off:
/// the shared one, then the local one that overrides it.
pub(crate) fn project_settings_files(project: &Path) -> [PathBuf; 2] {
    let dir = project.join(".claude");
    [dir.join("settings.json"), dir.join("settings.local.json")]
}

/// What a project's settings files say about plugins.
struct ProjectPluginSettings {
    shared_file: PathBuf,
    local_file: PathBuf,
    shared: std::collections::HashMap<String, bool>,
    local: std::collections::HashMap<String, bool>,
}

impl ProjectPluginSettings {
    /// `user_file` is the user-level settings file. When the project is the
    /// home folder its shared file is that same file, which already speaks
    /// for Global and says nothing of its own for the project.
    fn read(project: &Path, user_file: &Path) -> Self {
        let [shared_file, local_file] = project_settings_files(project);
        let shared = if crate::paths_equal(&shared_file, user_file) {
            Default::default()
        } else {
            read_enabled_plugins(&shared_file)
        };
        Self {
            shared,
            local: read_enabled_plugins(&local_file),
            shared_file,
            local_file,
        }
    }

    /// The project's own verdict on `key`, local over shared. `None` when
    /// neither file names the plugin.
    fn enabled(&self, key: &str) -> Option<bool> {
        self.local
            .get(key)
            .or_else(|| self.shared.get(key))
            .copied()
    }

    /// The file a toggle must write so that its value is the one that wins:
    /// the local file once it names the plugin, or when the plugin is only
    /// installed at local scope.
    fn toggle_file(&self, key: &str, local_only: bool) -> &Path {
        if self.local.contains_key(key) || local_only {
            &self.local_file
        } else {
            &self.shared_file
        }
    }
}

impl ClaudeAdapter {
    /// Plugins from `installed_plugins.json` as one scope sees them.
    ///
    /// `None` is the user level: plugins with a user-scope install, switched
    /// by `~/.claude/settings.json`. `Some(project)` lists the plugins that
    /// project configures: those installed for it, plus user-scope installs
    /// its settings files override. Their `uri` is the settings file a toggle
    /// writes to.
    fn read_installed_plugins(&self, project: Option<&Path>) -> Vec<PluginEntry> {
        let plugins_dir = self.base_dir().join("plugins");
        let Some(json) = Self::parse_json(&installed_plugins_file(&self.base_dir())) else {
            return vec![];
        };
        let Some(plugins) = json.get("plugins").and_then(|v| v.as_object()) else {
            return vec![];
        };

        // Map marketplace name → upstream "owner/repo" from the agent's own
        // catalog, so each plugin is attributed to its real source instead of
        // the `.git` its cache dir happens to sit under.
        let marketplace_repo = read_marketplace_repos(&plugins_dir);

        let user_file = self.plugin_config_path();
        let user_settings = read_enabled_plugins(&user_file);
        let project = project.map(|path| (path, ProjectPluginSettings::read(path, &user_file)));

        let mut entries = Vec::new();
        for (key, installs) in plugins {
            let (name, source) = split_plugin_key(key);

            let installs = installs.as_array().map(Vec::as_slice).unwrap_or_default();
            let user_install = installs.iter().find(|i| !is_project_install(i));
            let user_enabled = user_settings.get(key).copied().unwrap_or(false);

            let (install, enabled, toggle_file) = match &project {
                None => (user_install, user_enabled, None),
                Some((path, settings)) => {
                    let own: Vec<&serde_json::Value> = installs
                        .iter()
                        .filter(|i| is_install_for_project(i, path))
                        .collect();
                    let verdict = settings.enabled(key);
                    let install = own
                        .first()
                        .copied()
                        .or(user_install.filter(|_| verdict.is_some()));
                    let local_only =
                        !own.is_empty() && own.iter().all(|i| install_scope(i) == Some("local"));
                    (
                        install,
                        verdict.unwrap_or(user_enabled),
                        Some(settings.toggle_file(key, local_only)),
                    )
                }
            };
            let Some(install) = install else {
                continue;
            };

            let install_path = install
                .get("installPath")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .and_then(|p| p.parent().map(PathBuf::from)); // strip version component

            let installed_at = install
                .get("installedAt")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            let updated_at = install
                .get("lastUpdated")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            let source_url = marketplace_repo
                .get(&source)
                .map(|repo| format!("https://github.com/{repo}"));

            entries.push(PluginEntry {
                name,
                source: source.clone(),
                enabled,
                path: install_path,
                source_url,
                uri: toggle_file.map(|f| f.to_string_lossy().to_string()),
                installed_at,
                updated_at,
                base_layers: vec![],
                pack: None,
            });
        }
        entries
    }
}

/// Parse `<plugins_dir>/known_marketplaces.json` into a `marketplace name →
/// "owner/repo"` map. Empty when the catalog is missing or unreadable.
fn read_marketplace_repos(plugins_dir: &Path) -> std::collections::HashMap<String, String> {
    let path = plugins_dir.join("known_marketplaces.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return std::collections::HashMap::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return std::collections::HashMap::new();
    };
    let Some(obj) = json.as_object() else {
        return std::collections::HashMap::new();
    };
    obj.iter()
        .filter_map(|(name, entry)| {
            let src = entry.get("source")?;
            // Only github marketplaces map cleanly to a github.com/<repo> URL;
            // skip others (gitlab, local, …) so we don't fabricate a wrong URL.
            if src.get("source").and_then(|v| v.as_str()) != Some("github") {
                return None;
            }
            let repo = src.get("repo")?.as_str()?.to_string();
            Some((name.clone(), repo))
        })
        .collect()
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
    fn read_mcp_servers_parses_remote_transports() {
        // Shapes written by `claude mcp add --transport http|sse` (issue #105).
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join(".claude.json");
        std::fs::write(
            &config,
            r#"{"mcpServers":{
                "linear":{"type":"http","url":"https://mcp.linear.app/mcp",
                          "headers":{"Authorization":"Bearer tok"}},
                "events":{"type":"sse","url":"https://example.com/sse"},
                "spec-alias":{"type":"streamable-http","url":"https://example.com/mcp"},
                "bare-url":{"url":"https://example.com/mcp"},
                "fs":{"command":"npx","args":["-y","server-fs"]}
            }}"#,
        )
        .unwrap();
        let adapter = ClaudeAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers_from(&config);
        let by_name: std::collections::HashMap<_, _> =
            servers.iter().map(|s| (s.name.as_str(), s)).collect();

        let linear = by_name["linear"];
        assert_eq!(linear.transport, super::super::McpTransport::Http);
        assert_eq!(linear.url.as_deref(), Some("https://mcp.linear.app/mcp"));
        assert_eq!(linear.command, "", "remote entries must not fill command");
        assert_eq!(linear.headers["Authorization"], "Bearer tok");

        assert_eq!(by_name["events"].transport, super::super::McpTransport::Sse);
        // `streamable-http` is the MCP spec's accepted alias for `http`.
        assert_eq!(
            by_name["spec-alias"].transport,
            super::super::McpTransport::Http
        );
        // url without type tolerated as Streamable HTTP (laxer than Claude
        // itself, which refuses to load such an entry — see parse_type_url).
        assert_eq!(
            by_name["bare-url"].transport,
            super::super::McpTransport::Http
        );

        let fs = by_name["fs"];
        assert_eq!(fs.transport, super::super::McpTransport::Stdio);
        assert_eq!(fs.command, "npx");
        assert_eq!(fs.url, None);
    }

    #[test]
    fn external_project_memory_groups_files_with_session_cwd() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        // Project A: has a transcript → owner cwd is known.
        let a = home.join(".claude/projects/-Users-zoe-Demo-Proj");
        fs::create_dir_all(a.join("memory")).unwrap();
        fs::write(a.join("s.jsonl"), "{\"cwd\":\"/Users/zoe/Demo/Proj\"}\n").unwrap();
        fs::write(a.join("memory/note.md"), "x").unwrap();

        // Project B: memory but NO transcript → owner unknown (None).
        let b = home.join(".claude/projects/-orphan");
        fs::create_dir_all(b.join("memory")).unwrap();
        fs::write(b.join("memory/orphan.md"), "y").unwrap();

        let adapter = ClaudeAdapter::with_home(home.to_path_buf());
        let groups = adapter.external_project_memory();

        // One group per project dir that actually has memory.
        assert_eq!(groups.len(), 2);

        let known = groups
            .iter()
            .find(|(_, f)| f.iter().any(|p| p.ends_with("note.md")))
            .unwrap();
        assert_eq!(
            known.0.as_deref(),
            Some(std::path::Path::new("/Users/zoe/Demo/Proj"))
        );

        let orphan = groups
            .iter()
            .find(|(_, f)| f.iter().any(|p| p.ends_with("orphan.md")))
            .unwrap();
        assert_eq!(orphan.0, None);
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
        // MCP config lives at ~/.claude.json (not ~/.claude/settings.json)
        std::fs::write(
            dir.path().join(".claude.json"),
            r#"{"mcpServers":{"github":{"command":"npx","args":["-y","@modelcontextprotocol/server-github"],"env":{"GITHUB_TOKEN":"ghp_test"}}}}"#,
        ).unwrap();
        let adapter = ClaudeAdapter::with_home(dir.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "github");
        assert_eq!(servers[0].command, "npx");
    }

    #[test]
    fn test_claude_read_hooks_string_format() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":["echo test"]}]}}"#,
        )
        .unwrap();
        let adapter = ClaudeAdapter::with_home(dir.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].event, "PreToolUse");
        assert_eq!(hooks[0].command, "echo test");
    }

    #[test]
    fn test_claude_read_hooks_object_format() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"afplay /System/Library/Sounds/Glass.aiff"}]}]}}"#,
        ).unwrap();
        let adapter = ClaudeAdapter::with_home(dir.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].event, "Stop");
        assert_eq!(hooks[0].command, "afplay /System/Library/Sounds/Glass.aiff");
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
        assert!(global_settings.len() >= 4);
        assert!(global_settings[0].ends_with(".claude.json"));
        assert!(global_settings[1].ends_with("settings.json"));
        assert!(global_settings[2].ends_with("settings.local.json"));
        assert!(global_settings[3].ends_with("keybindings.json"));

        let project_rules = adapter.project_rules_patterns();
        assert!(project_rules.contains(&"CLAUDE.md".to_string()));
        assert!(project_rules.contains(&".claude/CLAUDE.md".to_string()));
        assert!(project_rules.contains(&".claude/rules/**/*.md".to_string()));

        let project_settings = adapter.project_settings_patterns();
        assert!(project_settings.contains(&".claude/settings.json".to_string()));
        assert!(project_settings.contains(&".claude/settings.local.json".to_string()));
        assert!(project_settings.contains(&".mcp.json".to_string()));

        let project_ignore = adapter.project_ignore_patterns();
        assert!(project_ignore.is_empty());
    }

    #[test]
    fn test_claude_subagent_methods() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = ClaudeAdapter::with_home(tmp.path().to_path_buf());

        // Missing agents/ dir → empty.
        assert!(adapter.global_subagent_files().is_empty());

        // Populate agents/ with one .md and one non-.md (must be filtered).
        let agents_dir = adapter.base_dir().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("reviewer.md"), "# reviewer").unwrap();
        std::fs::write(agents_dir.join("notes.txt"), "ignore me").unwrap();

        let subagents = adapter.global_subagent_files();
        assert!(subagents.iter().any(|p| p.ends_with("agents/reviewer.md")));
        assert!(
            !subagents.iter().any(|p| p.ends_with("notes.txt")),
            "non-.md files in agents/ must be filtered"
        );

        // Subagent dir contents must NOT leak into settings anymore.
        let settings = adapter.global_settings_files();
        assert!(
            !settings.iter().any(|p| p.ends_with("agents/reviewer.md")),
            "agents/ moved to global_subagent_files; must not appear in settings"
        );

        // Project pattern is the canonical .claude/agents/*.md.
        assert_eq!(
            adapter.project_subagent_patterns(),
            vec![".claude/agents/*.md".to_string()]
        );
    }

    /// A home with `demo` installed for `project` only and `two` installed
    /// for the user, the shapes `claude plugin install --scope` writes.
    fn plugin_home(project_scope: &str) -> (TempDir, ClaudeAdapter, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join(".claude")).unwrap();
        let plugins = tmp.path().join(".claude/plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let registry = serde_json::json!({"version": 2, "plugins": {
            "demo@mkt": [{"scope": project_scope, "projectPath": project,
                          "installPath": "/cache/mkt/demo/1.0.0"}],
            "two@mkt": [{"scope": "user", "installPath": "/cache/mkt/two/1.0.0"}],
        }});
        std::fs::write(plugins.join("installed_plugins.json"), registry.to_string()).unwrap();
        let adapter = ClaudeAdapter::with_home(tmp.path().to_path_buf());
        (tmp, adapter, project)
    }

    fn enable_plugins(settings: &Path, value: serde_json::Value) {
        std::fs::write(
            settings,
            serde_json::json!({ "enabledPlugins": value }).to_string(),
        )
        .unwrap();
    }

    fn names(plugins: &[PluginEntry]) -> Vec<&str> {
        plugins.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn project_installed_plugin_is_listed_only_in_its_project() {
        let (tmp, adapter, project) = plugin_home("project");
        let elsewhere = tmp.path().join("elsewhere");

        assert_eq!(names(&adapter.read_plugins()), ["two"]);
        assert_eq!(names(&adapter.read_project_plugins(&project)), ["demo"]);
        assert!(adapter.read_project_plugins(&elsewhere).is_empty());
    }

    #[test]
    fn project_settings_enable_a_plugin_the_user_settings_disable() {
        // Issue #189: Claude loads the plugin in the project regardless of
        // the user-level `false`.
        let (tmp, adapter, project) = plugin_home("project");
        enable_plugins(
            &tmp.path().join(".claude/settings.json"),
            serde_json::json!({"demo@mkt": false}),
        );
        let project_settings = project.join(".claude/settings.json");
        enable_plugins(&project_settings, serde_json::json!({"demo@mkt": true}));

        let plugins = adapter.read_project_plugins(&project);

        assert!(plugins[0].enabled);
        assert_eq!(
            plugins[0].uri.as_deref(),
            Some(project_settings.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn local_settings_decide_over_project_settings() {
        let (_tmp, adapter, project) = plugin_home("project");
        enable_plugins(
            &project.join(".claude/settings.json"),
            serde_json::json!({"demo@mkt": true}),
        );
        let local_settings = project.join(".claude/settings.local.json");
        enable_plugins(&local_settings, serde_json::json!({"demo@mkt": false}));

        let plugins = adapter.read_project_plugins(&project);

        assert!(!plugins[0].enabled);
        assert_eq!(
            plugins[0].uri.as_deref(),
            Some(local_settings.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn local_scope_install_is_toggled_in_local_settings() {
        let (_tmp, adapter, project) = plugin_home("local");

        let plugins = adapter.read_project_plugins(&project);

        assert_eq!(
            plugins[0].uri.as_deref(),
            Some(
                project
                    .join(".claude/settings.local.json")
                    .to_string_lossy()
                    .as_ref()
            )
        );
    }

    #[test]
    fn project_overriding_a_user_install_lists_it_with_the_project_verdict() {
        // `two` is installed for the user only; the project switches it off.
        let (tmp, adapter, project) = plugin_home("project");
        enable_plugins(
            &tmp.path().join(".claude/settings.json"),
            serde_json::json!({"two@mkt": true}),
        );
        assert_eq!(names(&adapter.read_project_plugins(&project)), ["demo"]);
        enable_plugins(
            &project.join(".claude/settings.json"),
            serde_json::json!({"two@mkt": false}),
        );

        let plugins = adapter.read_project_plugins(&project);

        let two = plugins.iter().find(|p| p.name == "two").unwrap();
        assert!(!two.enabled);
        assert!(adapter.read_plugins()[0].enabled);
    }

    #[test]
    fn plugins_installed_for_projects_only_have_no_global_install() {
        // `demo` is installed for the project only, `two` for the user.
        let (_tmp, adapter, _project) = plugin_home("project");

        assert_eq!(
            adapter.plugins_without_global_install(),
            [("demo".to_string(), "mkt".to_string())]
        );
    }

    #[test]
    fn home_folder_as_a_project_does_not_repeat_the_user_plugins() {
        let (tmp, adapter, _project) = plugin_home("project");
        enable_plugins(
            &tmp.path().join(".claude/settings.json"),
            serde_json::json!({"two@mkt": true}),
        );

        assert!(adapter.read_project_plugins(tmp.path()).is_empty());
    }

    #[test]
    fn plugin_installed_for_user_and_project_is_listed_at_both_levels() {
        let (tmp, adapter, project) = plugin_home("project");
        let registry = serde_json::json!({"plugins": {"demo@mkt": [
            {"scope": "project", "projectPath": project, "installPath": "/cache/mkt/demo/1.0.0"},
            {"scope": "user", "installPath": "/cache/mkt/demo/1.0.0"},
        ]}});
        std::fs::write(
            tmp.path().join(".claude/plugins/installed_plugins.json"),
            registry.to_string(),
        )
        .unwrap();

        assert_eq!(names(&adapter.read_plugins()), ["demo"]);
        assert_eq!(names(&adapter.read_project_plugins(&project)), ["demo"]);
    }

    #[test]
    #[cfg(unix)]
    fn project_reached_through_a_symlink_still_matches_its_installs() {
        // Claude records the resolved path (`/private/tmp/x` for `/tmp/x`).
        let (tmp, adapter, project) = plugin_home("project");
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&project, &link).unwrap();

        assert_eq!(names(&adapter.read_project_plugins(&link)), ["demo"]);
    }
}
