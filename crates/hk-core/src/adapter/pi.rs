// Pi (earendil-works/pi coding agent) — verified against upstream docs
// (github.com/earendil-works/pi, packages/coding-agent/docs):
// - Agent dir: `$PI_CODING_AGENT_DIR` or `~/.pi/agent`
//   (configuration.md). Same root serves the pi CLI and pi-web
//   (github.com/agegr/pi-web), the local browser UI — pi-web reuses the
//   pi agent's own configuration, so there is no separate config root.
// - Skills: `~/.pi/agent/skills/` (user) + `.pi/skills/` (project). Pi also
//   reads the universal Agent Skills locations `~/.agents/skills` /
//   `.agents/skills` (skills.md), so those are declared here too. Pi walks
//   nested directories for SKILL.md; HarnessKit lists the top level only,
//   as for omp.
// - MCP: JSON `{"mcpServers": {name: {...}}}` — stdio `{command, args, env,
//   cwd}` or streamable HTTP `{url, headers}` with a native per-entry
//   `enabled` flag (mcp.md). User `~/.pi/agent/mcp.json`, project
//   `.pi/mcp.json`.
// - Extensions: TypeScript/JavaScript modules — `~/.pi/agent/extensions/` +
//   `.pi/extensions/` (extensions.md). Same shape as omp.
// - Context/rules: `AGENTS.override.md` / `AGENTS.md` / `AGENTS.MD` /
//   `CLAUDE.md` / `CLAUDE.MD` from the agent dir and each ancestor of the
//   working directory (configuration.md "Context files").
// - No shell-command hooks → HookFormat::None (extensions are TS/JS modules).

use super::{
    AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, PluginEntry, ProjectMarker,
    RemoteMcpSchema,
};
use std::path::{Path, PathBuf};

/// Context-file fallback list, in Pi's documented precedence order.
const CONTEXT_FILENAMES: &[&str] = &[
    "AGENTS.override.md",
    "AGENTS.md",
    "AGENTS.MD",
    "CLAUDE.md",
    "CLAUDE.MD",
];

pub struct PiAdapter {
    home: PathBuf,
    agent_dir: PathBuf,
}

impl Default for PiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PiAdapter {
    pub fn new() -> Self {
        let home = crate::home_dir();
        let agent_dir = std::env::var_os("PI_CODING_AGENT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".pi").join("agent"));
        Self { home, agent_dir }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self {
            agent_dir: home.join(".pi").join("agent"),
            home,
        }
    }

    fn parse_json(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for PiAdapter {
    fn name(&self) -> &str {
        "pi"
    }

    fn base_dir(&self) -> PathBuf {
        self.home.join(".pi")
    }

    fn detect(&self) -> bool {
        // Pi creates the agent dir on first run (`~/.pi/agent`), including
        // `~/.pi` for pi-web session state. Presence of the agent dir is the
        // reliable install signal.
        self.agent_dir.exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        // Native Pi user skills: ~/.pi/agent/skills, plus the universal
        // Agent Skills location ~/.agents/skills (skills.md: "Pi also
        // supports the Agent Skills locations ~/.agents/skills/ and
        // .agents/skills/").
        vec![
            self.agent_dir.join("skills"),
            self.home.join(".agents").join("skills"),
        ]
    }

    fn mcp_config_path(&self) -> PathBuf {
        // ~/.pi/agent/mcp.json is the canonical user-level file.
        self.agent_dir.join("mcp.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        // Pi has no shell-command hooks file; the trait requires a path, so
        // point at mcp.json like omp does.
        self.mcp_config_path()
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        // TypeScript/JavaScript extension modules: ~/.pi/agent/extensions/*.ts
        vec![self.agent_dir.join("extensions")]
    }

    fn hook_format(&self) -> HookFormat {
        HookFormat::None
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::McpServers
    }

    fn supports_native_mcp_toggle(&self) -> bool {
        // Toggle flips the entry's own `enabled` flag in place
        // (deployer::set_mcp_server_enabled_flag), keeping `description`,
        // `timeout`, `exposure`, `cwd`, `oauth`, `env`, `headers`, and all
        // other keys untouched — exactly how `pi mcp` / `/mcp` behaves.
        true
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        // Remote entries are `{url, headers}` and always Streamable HTTP:
        // Pi rejects `type: "sse"` outright (docs/mcp.md).
        RemoteMcpSchema::PlainUrlHttpOnly
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Some(config) = Self::parse_json(path) else {
            return vec![];
        };
        let Some(servers) = config.get("mcpServers").and_then(|v| v.as_object()) else {
            return vec![];
        };
        servers
            .iter()
            // A project entry with none of `command`, `url`, `type` is not a
            // server: it only overrides `enabled` / `exposure` /
            // `toolExposure` of the user-level server of the same name
            // (mcp.md). Reading it as a server would show an empty stdio
            // entry; the override itself is not modeled yet.
            .filter(|(_, val)| {
                ["command", "url", "type"]
                    .iter()
                    .any(|k| val.get(k).is_some())
            })
            .map(|(name, val)| {
                // `command` selects stdio, `url` selects streamable HTTP;
                // an optional `type` (stdio/http/streamable-http) also counts.
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
                    // Native per-server flag: absent means enabled.
                    enabled: val.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                }
            })
            .collect()
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        // Pi hooks are TS/JS event-handler extensions, not shell commands.
        vec![]
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        // Local .ts/.js extension modules (extensions.md). Packages installed
        // with `pi install` (settings.json `packages`) are not modeled yet.
        self.plugin_dirs()
            .iter()
            .flat_map(|dir| super::read_ts_extensions(dir))
            .collect()
    }

    fn read_plugins_from(&self, dir: &Path) -> Vec<PluginEntry> {
        super::read_ts_extensions(dir)
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        // Context files live directly in the agent dir (configuration.md).
        CONTEXT_FILENAMES
            .iter()
            .map(|name| self.agent_dir.join(name))
            .collect()
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        // settings.json is the primary user-level settings file; models.json
        // holds compatible endpoints; mcp.json is listed so the servers file
        // is browsable from the agent page (matches the omp/kiro adapters).
        vec![
            self.agent_dir.join("settings.json"),
            self.agent_dir.join("models.json"),
            self.mcp_config_path(),
        ]
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        // Dir(".pi") alone suffices: every project-level Pi file lives inside
        // `.pi/`.
        vec![ProjectMarker::Dir(".pi")]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        // Context files are discovered from the working directory and its
        // ancestors (configuration.md "Context files"), not from `.pi/`.
        CONTEXT_FILENAMES.iter().map(|s| s.to_string()).collect()
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".pi/settings.json".into()]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        // Pi-native project skills plus the universal Agent Skills location
        // (skills.md).
        vec![".pi/skills".into(), ".agents/skills".into()]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        // Pi reads `.pi/mcp.json`; project config is loaded after project
        // trust is granted.
        Some(".pi/mcp.json".into())
    }

    fn project_plugin_dirs(&self) -> Vec<String> {
        // Project-level extension modules: <repo>/.pi/extensions/*.ts
        vec![".pi/extensions".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    #[test]
    fn detect_requires_agent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = PiAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());
        std::fs::create_dir_all(tmp.path().join(".pi/agent")).unwrap();
        assert!(adapter.detect());
    }

    #[test]
    fn read_mcp_servers_handles_stdio_and_remote() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = PiAdapter::with_home(tmp.path().to_path_buf());
        let cfg = tmp.path().join(".pi/agent/mcp.json");
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(
            &cfg,
            r#"{
              "mcpServers": {
                "filesystem": {
                  "command": "npx",
                  "args": ["-y", "@modelcontextprotocol/server-filesystem"]
                },
                "docs": {
                  "url": "https://example.com/mcp",
                  "headers": { "Authorization": "Bearer t" }
                },
                "off": { "command": "echo", "enabled": false },
                "internal-tools": { "enabled": false }
              }
            }"#,
        )
        .unwrap();

        // `internal-tools` has no command/url/type: a project-style override
        // of a server defined elsewhere, not a server of its own.
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 3);
        assert!(servers.iter().all(|s| s.name != "internal-tools"));
        let by_name: std::collections::HashMap<&str, &McpServerEntry> =
            servers.iter().map(|s| (s.name.as_str(), s)).collect();

        assert_eq!(by_name["filesystem"].command, "npx");
        assert!(by_name["filesystem"].enabled);

        let docs = by_name["docs"];
        assert_eq!(docs.transport, super::super::McpTransport::Http);
        assert_eq!(docs.url.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(docs.headers["Authorization"], "Bearer t");

        assert!(!by_name["off"].enabled);
    }

    #[test]
    fn read_plugins_lists_ts_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = PiAdapter::with_home(tmp.path().to_path_buf());
        let ext_dir = tmp.path().join(".pi/agent/extensions");
        std::fs::create_dir_all(&ext_dir).unwrap();
        std::fs::write(ext_dir.join("hello.ts"), "// ext\n").unwrap();
        std::fs::write(ext_dir.join("off.ts.disabled"), "// ext\n").unwrap();
        std::fs::write(ext_dir.join("README.md"), "# readme\n").unwrap();

        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 2);
        let by_name: std::collections::HashMap<&str, &PluginEntry> =
            plugins.iter().map(|p| (p.name.as_str(), p)).collect();
        assert!(by_name["hello"].enabled);
        assert!(!by_name["off"].enabled);
    }

    #[test]
    fn read_plugins_from_reads_project_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = PiAdapter::with_home(tmp.path().to_path_buf());
        let ext_dir = tmp.path().join("repo/.pi/extensions");
        std::fs::create_dir_all(ext_dir.join("repo-panel")).unwrap();
        std::fs::write(ext_dir.join("repo-panel/index.ts"), "// ext\n").unwrap();

        let plugins = adapter.read_plugins_from(&ext_dir);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "repo-panel");
        assert_eq!(plugins[0].path, Some(ext_dir.join("repo-panel/index.ts")));
        assert!(adapter.read_plugins().is_empty());
    }

    #[test]
    fn paths_match_pi_native_conventions() {
        let adapter = PiAdapter::with_home(PathBuf::from("/h"));
        assert_eq!(adapter.base_dir(), PathBuf::from("/h/.pi"));
        assert_eq!(
            adapter.skill_dirs(),
            vec![
                PathBuf::from("/h/.pi/agent/skills"),
                PathBuf::from("/h/.agents/skills"),
            ]
        );
        assert_eq!(
            adapter.mcp_config_path(),
            PathBuf::from("/h/.pi/agent/mcp.json")
        );
        assert_eq!(
            adapter.plugin_dirs(),
            vec![PathBuf::from("/h/.pi/agent/extensions")]
        );
        assert_eq!(
            adapter.project_skill_dirs(),
            vec![".pi/skills".to_string(), ".agents/skills".to_string()]
        );
        assert_eq!(
            adapter.project_mcp_config_relpath(),
            Some(".pi/mcp.json".into())
        );
        assert_eq!(adapter.hook_format(), HookFormat::None);
        assert_eq!(adapter.mcp_format(), McpFormat::McpServers);
    }
}
