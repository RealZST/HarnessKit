// omp (Oh My Pi) config references — read from the in-tree harness docs:
// - Skills:   https://github.com/can1357/oh-my-pi  → docs/skills.md
// - MCP:      docs/mcp-config.md
// - Config:   docs/config-usage.md
// - Context:  docs/context-files.md
// - Hooks:    docs/hooks.md
// - Extensions: docs/extensions.md, docs/extension-loading.md
//
// omp's base dir is ~/.omp; the agent subtree lives under ~/.omp/agent/.
// A named profile (`omp --profile <name>`) relocates the agent dir to
// ~/.omp/profiles/<name>/agent/ — HK scans the default profile only.
//
// Hooks are JS/TS modules (.omp/hooks/pre/*.ts), not shell commands, so this
// adapter reports HookFormat::None — same model as opencode ("hooks are JS
// plugins"). omp's shell-command HookEntry model has no equivalent.

use super::{
    AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, PluginEntry, ProjectMarker,
    RemoteMcpSchema,
};
use std::path::{Path, PathBuf};

pub struct OmpAdapter {
    home: PathBuf,
}

impl Default for OmpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OmpAdapter {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    /// The agent subtree: ~/.omp/agent. All omp-native config (skills, mcp.json,
    /// config.yml, extensions/, commands/, AGENTS.md, RULES.md) lives here.
    fn agent_dir(&self) -> PathBuf {
        self.base_dir().join("agent")
    }

    fn parse_json(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn plugin_name(path: &Path) -> String {
        // Strip `.disabled` first so a toggled-off `orca-spin.ts.disabled`
        // shares the name of its enabled sibling `orca-spin.ts` — the name
        // must be stable across enable/disable or HK's toggle model breaks.
        // Same approach as the opencode adapter.
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

    /// omp extensions are .ts/.js modules exporting a default factory. The
    /// loader globs only `*.{ts,js}` (discovery/helpers.ts, extension-loading.md)
    /// — no .mjs/.cjs — so matching more here would list files omp never loads.
    ///
    /// A `.disabled` suffix is HK's own in-place disable rename (same model as
    /// opencode plugins). omp's native disable mechanism is the
    /// `disabledExtensions` id list in config.yml, which HK doesn't write; the
    /// rename works because the loader's glob won't match a `.disabled` file.
    fn is_extension_file(path: &Path) -> bool {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let base = file_name.strip_suffix(".disabled").unwrap_or(&file_name);
        matches!(
            Path::new(base).extension().and_then(|ext| ext.to_str()),
            Some("ts" | "js")
        )
    }

    /// Read a string-array field (e.g. `disabledServers`) from the *user*
    /// mcp.json. Both the denylist and the force-enable allowlist live only in
    /// the user file but apply to servers from every source, including project
    /// configs (mcp/config.ts in the omp source).
    fn user_mcp_name_list(&self, key: &str) -> std::collections::HashSet<String> {
        Self::parse_json(&self.mcp_config_path())
            .and_then(|cfg| {
                cfg.get(key).and_then(|v| v.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
            })
            .unwrap_or_default()
    }
}

impl AgentAdapter for OmpAdapter {
    fn name(&self) -> &str {
        "omp"
    }

    fn base_dir(&self) -> PathBuf {
        self.home.join(".omp")
    }

    fn detect(&self) -> bool {
        // omp creates ~/.omp on first run (install-id, agent/, natives/).
        // Presence of the base dir is a reliable signal — matches how every
        // other adapter detects (base_dir().exists()).
        self.base_dir().exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        // Native omp skills live one level under ~/.omp/agent/skills/.
        // (omp also reads .agents/skills and .claude/skills via its own
        // discovery providers, but those are owned by the agents/codex/claude
        // adapters — don't double-claim them here.)
        vec![self.agent_dir().join("skills")]
    }

    fn mcp_config_path(&self) -> PathBuf {
        // ~/.omp/agent/mcp.json is the canonical user-level file. omp also
        // reads ~/.omp/agent/.mcp.json for compat, but writes to mcp.json.
        self.agent_dir().join("mcp.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        // omp has no shell-command hooks file. Reuse the MCP path so the
        // default plugin_config_path() lands on a real file (mirrors opencode).
        self.mcp_config_path()
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        // TypeScript extension modules: ~/.omp/agent/extensions/*.ts
        vec![self.agent_dir().join("extensions")]
    }

    fn hook_format(&self) -> HookFormat {
        HookFormat::None
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::McpServers
    }

    fn supports_native_mcp_toggle(&self) -> bool {
        // Toggle flips the entry's own `enabled` flag in place (see
        // deployer::set_omp_mcp_enabled) — no remove+snapshot, so secrets and
        // extra keys (`type`, `url`, `headers`) are never touched. The
        // user-level disabledServers/enabledServers lists are scrubbed so the
        // flag actually takes effect.
        true
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
        // omp's effective-enabled state is decided by three inputs
        // (mcp/config.ts): the user-level `disabledServers` denylist always
        // wins; the per-entry `enabled` flag (default true) is next; the
        // user-level `enabledServers` allowlist can force a server back on
        // over `enabled: false` — but never over the denylist.
        let denylist = self.user_mcp_name_list("disabledServers");
        let allowlist = self.user_mcp_name_list("enabledServers");
        servers
            .iter()
            .map(|(name, val)| {
                // Remote entries: {type: "http"|"sse", url, headers}.
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
                    // Reported so the scanner reflects effective on-disk state;
                    // the toggle writes the same inputs back in place
                    // (deployer::set_omp_mcp_enabled).
                    enabled: !denylist.contains(name)
                        && (val
                            .get("enabled")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true)
                            || allowlist.contains(name)),
                }
            })
            .collect()
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::TypeAndUrl
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        // omp hooks are JS/TS event-handler modules, not shell commands —
        // there is no shell HookEntry representation. Returns empty, matching
        // opencode.
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
                if path.is_file() {
                    if !Self::is_extension_file(&path) {
                        continue;
                    }
                    let enabled = path.extension().is_none_or(|ext| ext != "disabled");
                    entries.push(PluginEntry {
                        name: Self::plugin_name(&path),
                        source: "local".into(),
                        enabled,
                        path: Some(path),
                        source_url: None,
                        uri: None,
                        installed_at: None,
                        updated_at: None,
                    });
                } else if path.is_dir() {
                    // Directory-form extension: <name>/index.{ts,js}, TypeScript
                    // preferred (extension-loading.md resolution order). Active
                    // entry files first so a `index.ts.disabled` next to a live
                    // `index.js` reports what omp actually loads. The entry's
                    // path is the index file — HK's toggle renames that file,
                    // keeping the directory (and thus the plugin name) stable.
                    // package.json-manifest extensions (`omp.extensions`) are
                    // not modeled.
                    let Some(dir_name) = path.file_name().map(|n| n.to_string_lossy().to_string())
                    else {
                        continue;
                    };
                    let candidates = [
                        ("index.ts", true),
                        ("index.js", true),
                        ("index.ts.disabled", false),
                        ("index.js.disabled", false),
                    ];
                    if let Some((entry_file, enabled)) = candidates
                        .iter()
                        .map(|(f, e)| (path.join(f), *e))
                        .find(|(p, _)| p.is_file())
                    {
                        entries.push(PluginEntry {
                            name: dir_name,
                            source: "local".into(),
                            enabled,
                            path: Some(entry_file),
                            source_url: None,
                            uri: None,
                            installed_at: None,
                            updated_at: None,
                        });
                    }
                }
            }
        }
        entries
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        // AGENTS.md is the native context file (advisory background loaded into
        // the opening prompt). RULES.md is the sticky-rule sibling (loaded as
        // an always-apply rule). rules/*.{md,mdc} are per-file rules loaded by
        // the same provider (builtin.ts loadRules), separate from the sticky
        // RULES.md. All omp-native, read from ~/.omp/agent/ at user scope.
        let agent = self.agent_dir();
        let mut files = vec![agent.join("AGENTS.md"), agent.join("RULES.md")];
        files.extend(super::files_with_ext(&agent.join("rules"), "md"));
        files.extend(super::files_with_ext(&agent.join("rules"), "mdc"));
        files
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        // config.yml is the primary (post-YAML-migration) global settings file.
        // settings.json is the legacy form, kept until migration runs. mcp.json
        // is listed so the servers file is browsable from the agent page
        // (matches the kiro adapter).
        let agent = self.agent_dir();
        vec![
            agent.join("config.yml"),
            agent.join("settings.json"),
            self.mcp_config_path(),
        ]
    }

    fn global_workflow_files(&self) -> Vec<PathBuf> {
        // Slash commands: ~/.omp/agent/commands/*.md
        super::files_with_ext(&self.agent_dir().join("commands"), "md").collect()
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        // Dir(".omp") alone suffices: every project-level omp file lives inside
        // .omp/, so any narrower marker would be redundant.
        vec![ProjectMarker::Dir(".omp")]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        // Native project context: <ancestor>/.omp/AGENTS.md (nearest walk-up to
        // repo root), the sticky <ancestor>/.omp/RULES.md, and per-file rules
        // under .omp/rules/ (md + mdc, builtin.ts loadRules). All omp-native
        // and only read from the .omp config directory.
        vec![
            ".omp/AGENTS.md".into(),
            ".omp/RULES.md".into(),
            ".omp/rules/*.md".into(),
            ".omp/rules/*.mdc".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".omp/config.yml".into(), ".omp/settings.json".into()]
    }

    fn project_workflow_patterns(&self) -> Vec<String> {
        vec![".omp/commands/*.md".into()]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        vec![".omp/skills".into()]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        // omp reads .omp/mcp.json (and the compat .omp/.mcp.json); writes go to
        // .omp/mcp.json. See docs/mcp-config.md "Preferred config locations".
        Some(".omp/mcp.json".into())
    }

    fn project_plugin_dirs(&self) -> Vec<String> {
        // Project-level extension modules: <repo>/.omp/extensions/*.ts
        vec![".omp/extensions".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    #[test]
    fn detect_requires_omp_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = OmpAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());

        std::fs::create_dir_all(tmp.path().join(".omp")).unwrap();
        assert!(adapter.detect());
    }

    #[test]
    fn read_mcp_servers_handles_stdio_and_remote() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = OmpAdapter::with_home(tmp.path().to_path_buf());
        let cfg = tmp.path().join(".omp/agent/mcp.json");
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(
            &cfg,
            r#"{
              "mcpServers": {
                "filesystem": {
                  "command": "npx",
                  "args": ["-y", "@modelcontextprotocol/server-filesystem"]
                },
                "github": {
                  "type": "http",
                  "url": "https://api.githubcopilot.com/mcp/"
                },
                "disabled-one": {
                  "command": "echo",
                  "enabled": false
                },
                "denylisted": {
                  "command": "echo"
                },
                "force-enabled": {
                  "command": "echo",
                  "enabled": false
                }
              },
              "disabledServers": ["denylisted"],
              "enabledServers": ["force-enabled"]
            }"#,
        )
        .unwrap();

        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 5);

        let by_name: std::collections::HashMap<&str, &McpServerEntry> =
            servers.iter().map(|s| (s.name.as_str(), s)).collect();

        let fs = by_name["filesystem"];
        assert_eq!(fs.command, "npx");
        assert_eq!(fs.args, vec!["-y", "@modelcontextprotocol/server-filesystem"]);
        assert!(fs.enabled);

        // Remote server: transport + url fields, command stays empty.
        let gh = by_name["github"];
        assert_eq!(gh.transport, super::super::McpTransport::Http);
        assert_eq!(gh.url.as_deref(), Some("https://api.githubcopilot.com/mcp/"));
        assert_eq!(gh.command, "");
        assert!(gh.args.is_empty());

        // Native per-server enabled flag is read back.
        let off = by_name["disabled-one"];
        assert_eq!(off.command, "echo");
        assert!(!off.enabled);

        // The user-level `disabledServers` denylist always wins, even over an
        // entry with no `enabled: false` of its own.
        assert!(!by_name["denylisted"].enabled);

        // The `enabledServers` allowlist forces a server back on over its own
        // `enabled: false` (but never over the denylist).
        assert!(by_name["force-enabled"].enabled);
    }

    #[test]
    fn user_denylist_applies_to_project_servers() {
        // Both name lists live in the *user* mcp.json but gate servers from
        // every source, including project configs (mcp/config.ts).
        let tmp = tempfile::tempdir().unwrap();
        let adapter = OmpAdapter::with_home(tmp.path().to_path_buf());
        let user_cfg = tmp.path().join(".omp/agent/mcp.json");
        std::fs::create_dir_all(user_cfg.parent().unwrap()).unwrap();
        std::fs::write(
            &user_cfg,
            r#"{ "mcpServers": {}, "disabledServers": ["project-server"] }"#,
        )
        .unwrap();

        let project_cfg = tmp.path().join("repo/.omp/mcp.json");
        std::fs::create_dir_all(project_cfg.parent().unwrap()).unwrap();
        std::fs::write(
            &project_cfg,
            r#"{ "mcpServers": { "project-server": { "command": "echo" } } }"#,
        )
        .unwrap();

        let servers = adapter.read_mcp_servers_from(&project_cfg);
        assert_eq!(servers.len(), 1);
        assert!(!servers[0].enabled);
    }

    #[test]
    fn read_plugins_lists_ts_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = OmpAdapter::with_home(tmp.path().to_path_buf());
        let ext_dir = tmp.path().join(".omp/agent/extensions");
        std::fs::create_dir_all(&ext_dir).unwrap();
        std::fs::write(ext_dir.join("orca-status.ts"), "// ext\n").unwrap();
        std::fs::write(ext_dir.join("orca-spin.ts.disabled"), "// ext\n").unwrap();
        // Non-extension files are ignored — including .mjs/.cjs, which omp's
        // loader glob (*.{ts,js}) never picks up.
        std::fs::write(ext_dir.join("README.md"), "# readme\n").unwrap();
        std::fs::write(ext_dir.join("not-loaded.mjs"), "// ext\n").unwrap();

        // Directory-form extension: <name>/index.ts.
        let dir_ext = ext_dir.join("orca-panel");
        std::fs::create_dir_all(&dir_ext).unwrap();
        std::fs::write(dir_ext.join("index.ts"), "// ext\n").unwrap();

        // Disabled directory-form extension: only index.ts.disabled inside.
        let dir_off = ext_dir.join("orca-dock");
        std::fs::create_dir_all(&dir_off).unwrap();
        std::fs::write(dir_off.join("index.ts.disabled"), "// ext\n").unwrap();

        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 4);
        let by_name: std::collections::HashMap<&str, &PluginEntry> =
            plugins.iter().map(|p| (p.name.as_str(), p)).collect();
        assert!(by_name.contains_key("orca-status"));
        assert!(by_name["orca-status"].enabled);
        assert!(by_name.contains_key("orca-spin"));
        assert!(!by_name["orca-spin"].enabled);

        // Directory-form entries are named after the directory; the path points
        // at the index file so HK's rename toggle keeps the name stable.
        let panel = by_name["orca-panel"];
        assert!(panel.enabled);
        assert_eq!(panel.path, Some(dir_ext.join("index.ts")));
        assert!(!by_name["orca-dock"].enabled);
    }

    #[test]
    fn paths_match_omp_native_conventions() {
        let adapter = OmpAdapter::with_home(PathBuf::from("/h"));
        assert_eq!(adapter.base_dir(), PathBuf::from("/h/.omp"));
        assert_eq!(adapter.skill_dirs(), vec![PathBuf::from("/h/.omp/agent/skills")]);
        assert_eq!(
            adapter.mcp_config_path(),
            PathBuf::from("/h/.omp/agent/mcp.json")
        );
        assert_eq!(adapter.plugin_dirs(), vec![PathBuf::from("/h/.omp/agent/extensions")]);
        assert_eq!(
            adapter.project_skill_dirs(),
            vec![".omp/skills".to_string()]
        );
        assert_eq!(
            adapter.project_mcp_config_relpath(),
            Some(".omp/mcp.json".into())
        );
        assert_eq!(adapter.hook_format(), HookFormat::None);
        assert_eq!(adapter.mcp_format(), McpFormat::McpServers);
    }
}
