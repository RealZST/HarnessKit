// OpenClaw gateway. Paths follow openclaw/openclaw (2026-10):
//
// - state dir: OPENCLAW_STATE_DIR, else ~/.openclaw (src/config/state-dir.ts)
// - config: OPENCLAW_CONFIG_PATH, else `<state>/openclaw.json` (src/config/paths.ts)
// - managed dir: OPENCLAW_STATE_DIR, else the config file's directory, else
//   ~/.openclaw (src/infra/config-dir.ts); `<managed>/skills` is where
//   `openclaw skills install --global` puts skills for every agent.
//
// Workspaces (src/agents/agent-scope-config.ts `resolveAgentWorkspaceDir`):
// the roster is `agents.entries` (or the older `agents.list`). An entry's own
// `workspace` wins. Without one, the owner agent (`default: true` unless
// `agents.ownership` is "explicit", else the sole entry) gets
// `agents.defaults.workspace`, else OPENCLAW_WORKSPACE_DIR, else
// `<state>/workspace`; every other agent gets `<defaults>/<id>`, else
// `<state>/workspace-<id>`. No roster at all means the default workspace.
// `openclaw onboard` and `openclaw agents add` write an explicit workspace
// for every entry and set explicit ownership, so the fallbacks only serve
// hand-written configs.
// Each workspace holds AGENTS.md / SOUL.md / IDENTITY.md (instructions and
// persona → Rules) and MEMORY.md / USER.md / memory/YYYY-MM-DD.md (→ Memory).
// Docs: https://docs.openclaw.ai/concepts/agent-workspace
//
// Skills (src/skills/loading/workspace-skill-sources.ts), highest precedence
// first: `<workspace>/skills` (the default `openclaw skills install` target),
// `<workspace>/.agents/skills`, `~/.agents/skills` (only with the default
// state dir), `<managed>/skills`. Not scanned yet: `skills.load.extraDirs`
// and the per-agent workshop dir.
// Docs: https://docs.openclaw.ai/tools/skills
//
// MCP: `openclaw.json` (JSON5) under the nested `mcp.servers` key. Each entry
// is command-based ({command, args?, env?, cwd?}) or URL-based ({url,
// transport: "streamable-http"|"sse", headers?}). Entries carry a native
// `enabled` flag, so toggling flips it in place (like Hermes).
// Docs: https://docs.openclaw.ai/tools/mcp
//
// Hooks (JS handlers + HTTP ingress) are a different model from the
// Claude-style command hooks HarnessKit manages, so they are not scanned or
// installed here (HookFormat::None). Plugins and remote MCP writes are
// follow-ups.
//
// Global-only by design: OpenClaw discovers skills from configured
// workspaces, never from the current directory, so there is no project
// scope to map — pinned in adapter::tests alongside hermes.

use super::{
    AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, McpTransport, PluginEntry,
    RemoteMcpSchema,
};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub struct OpenClawAdapter {
    home: PathBuf,
    state_dir: PathBuf,
    config_path: PathBuf,
    managed_dir: PathBuf,
    default_workspace: PathBuf,
}

impl Default for OpenClawAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// ParseOptions enabling the full JSON5 syntax surface: OpenClaw documents
/// its config as JSON5, so hand-written files may use comments (JSONC),
/// trailing commas, single quotes, and unquoted keys.
pub(crate) fn json5_parse_options() -> jsonc_parser::ParseOptions {
    jsonc_parser::ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_single_quoted_strings: true,
        allow_hexadecimal_numbers: true,
        allow_unary_plus_numbers: true,
        allow_missing_commas: true,
        allow_loose_object_property_names: true,
    }
}

/// A user-supplied path the way OpenClaw's `resolveUserPath` reads it:
/// trimmed, `~` expanded. Blank values count as unset, and so do relative
/// ones, which the gateway would resolve against its own working directory.
fn user_path(home: &Path, raw: &str) -> Option<PathBuf> {
    let path = match raw.trim() {
        "" => return None,
        "~" => home.to_path_buf(),
        raw => match raw.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => PathBuf::from(raw),
        },
    };
    path.is_absolute().then_some(path)
}

/// Agent ids are normalised before they name a directory
/// (packages/normalization-core/src/agent-id.ts, in short).
fn agent_dir_id(id: &str) -> String {
    id.trim().to_ascii_lowercase()
}

fn push_unique(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if !dirs.contains(&dir) {
        dirs.push(dir);
    }
}

impl OpenClawAdapter {
    pub fn new() -> Self {
        let var = |name| std::env::var_os(name).filter(|v| !v.is_empty());
        Self::from_env(
            dirs::home_dir().unwrap_or_default(),
            var("OPENCLAW_STATE_DIR"),
            var("OPENCLAW_CONFIG_PATH"),
            var("OPENCLAW_WORKSPACE_DIR"),
        )
    }

    /// Test/deployer constructor rooting everything under `home`; does not
    /// read the environment.
    pub fn with_home(home: PathBuf) -> Self {
        Self::from_env(home, None, None, None)
    }

    /// The gateway's path precedence for the three environment overrides
    /// (see the module header); `None` means unset.
    fn from_env(
        home: PathBuf,
        state_dir: Option<OsString>,
        config_path: Option<OsString>,
        workspace_dir: Option<OsString>,
    ) -> Self {
        let env = |value: Option<OsString>| user_path(&home, value?.to_str()?);
        let state_override = env(state_dir);
        let state_dir = state_override
            .clone()
            .unwrap_or_else(|| home.join(".openclaw"));
        let config_path = env(config_path).unwrap_or_else(|| state_dir.join("openclaw.json"));
        let managed_dir = state_override.unwrap_or_else(|| {
            config_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default()
        });
        let default_workspace = env(workspace_dir).unwrap_or_else(|| state_dir.join("workspace"));
        Self {
            home,
            state_dir,
            config_path,
            managed_dir,
            default_workspace,
        }
    }

    /// `(id, entry)` pairs from `agents.entries` (object) or the older
    /// `agents.list` (array of entries carrying `id`); non-object entries
    /// are skipped like upstream.
    fn roster(agents: Option<&serde_json::Value>) -> Vec<(String, &serde_json::Value)> {
        let entries = agents
            .and_then(|a| a.get("entries"))
            .and_then(|e| e.as_object());
        if let Some(entries) = entries {
            return entries
                .iter()
                .filter(|(_, e)| e.is_object())
                .map(|(id, e)| (id.clone(), e))
                .collect();
        }
        agents
            .and_then(|a| a.get("list"))
            .and_then(|l| l.as_array())
            .into_iter()
            .flatten()
            .filter(|e| e.is_object())
            .filter_map(|e| Some((e.get("id")?.as_str()?.to_string(), e)))
            .collect()
    }

    /// The agent that owns the default workspace: the one marked
    /// `default: true` (unless `agents.ownership` is "explicit"), else the
    /// sole entry.
    fn owner<'a>(
        agents: Option<&serde_json::Value>,
        roster: &'a [(String, &serde_json::Value)],
    ) -> Option<&'a str> {
        let explicit = agents
            .and_then(|a| a.get("ownership"))
            .and_then(|v| v.as_str())
            == Some("explicit");
        let mut marked = roster
            .iter()
            .filter(|(_, e)| !explicit && e.get("default") == Some(&serde_json::Value::Bool(true)));
        match (marked.next(), marked.next()) {
            (Some((id, _)), None) => Some(id),
            (None, _) if roster.len() == 1 => Some(&roster[0].0),
            _ => None,
        }
    }

    /// Every agent workspace, sorted and deduplicated so scans are stable.
    fn workspace_dirs(&self) -> Vec<PathBuf> {
        let config = Self::parse_json5(&self.config_path);
        let agents = config.as_ref().and_then(|c| c.get("agents"));
        let defaults = agents
            .and_then(|a| a.pointer("/defaults/workspace"))
            .and_then(|v| user_path(&self.home, v.as_str()?));
        let roster = Self::roster(agents);
        if roster.is_empty() {
            return vec![defaults.unwrap_or_else(|| self.default_workspace.clone())];
        }
        let owner = Self::owner(agents, &roster);
        let mut dirs: Vec<PathBuf> = roster
            .iter()
            .map(|(id, entry)| {
                entry
                    .get("workspace")
                    .and_then(|v| user_path(&self.home, v.as_str()?))
                    .unwrap_or_else(|| match (&defaults, owner == Some(id.as_str())) {
                        (Some(d), true) => d.clone(),
                        (None, true) => self.default_workspace.clone(),
                        (Some(d), false) => d.join(agent_dir_id(id)),
                        (None, false) => self
                            .state_dir
                            .join(format!("workspace-{}", agent_dir_id(id))),
                    })
            })
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    }

    fn parse_json5(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        jsonc_parser::parse_to_serde_value(&content, &json5_parse_options())
            .ok()
            .flatten()
    }

    /// Transport precedence of openclaw/openclaw: a non-blank `command` is
    /// stdio whatever else the entry says and a bare `url` connects over SSE
    /// (src/agents/mcp-transport-config.ts); the canonical `transport` wins
    /// over the legacy `type` alias (src/config/mcp-config-normalize.ts).
    fn transport_and_url(server: &serde_json::Value) -> (McpTransport, Option<String>) {
        let field = |key| server.get(key).and_then(|v| v.as_str());
        if field("command").is_some_and(|c| !c.trim().is_empty()) {
            return (McpTransport::Stdio, None);
        }
        let url = field("url")
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(String::from);
        let transport = field("transport").or_else(|| field("type"));
        let transport = match transport.map(str::to_ascii_lowercase).as_deref() {
            Some("streamable-http" | "http") => McpTransport::Http,
            Some("sse") => McpTransport::Sse,
            _ if url.is_some() => McpTransport::Sse,
            _ => McpTransport::Stdio,
        };
        (transport, url)
    }
}

impl AgentAdapter for OpenClawAdapter {
    fn name(&self) -> &str {
        "openclaw"
    }
    fn base_dir(&self) -> PathBuf {
        self.state_dir.clone()
    }
    fn detect(&self) -> bool {
        self.state_dir.exists()
    }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        // Managed dir first: it is the cross-agent install target.
        let mut dirs = vec![self.managed_dir.join("skills")];
        for ws in self.workspace_dirs() {
            push_unique(&mut dirs, ws.join("skills"));
            push_unique(&mut dirs, ws.join(".agents").join("skills"));
        }
        if self.state_dir == self.home.join(".openclaw") {
            push_unique(&mut dirs, self.home.join(".agents").join("skills"));
        }
        dirs
    }
    fn mcp_config_path(&self) -> PathBuf {
        self.config_path.clone()
    }
    fn hook_config_path(&self) -> PathBuf {
        self.config_path.clone()
    }
    fn hook_format(&self) -> HookFormat {
        HookFormat::None
    }
    fn mcp_format(&self) -> McpFormat {
        McpFormat::OpenClawJson5
    }
    fn supports_native_mcp_toggle(&self) -> bool {
        true
    }
    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![]
    }
    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        // Follow-up PR: entries can carry {url, transport} remotes; the
        // generic Claude-style remote writer does not spell OpenClaw's
        // `transport` key, and install-gating must match the real writer.
        RemoteMcpSchema::Unsupported
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.config_path)
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Some(config) = Self::parse_json5(path) else {
            return vec![];
        };
        let Some(servers) = config
            .get("mcp")
            .and_then(|m| m.get("servers"))
            .and_then(|v| v.as_object())
        else {
            return vec![];
        };
        servers
            .iter()
            .map(|(name, val)| {
                let (transport, url) = Self::transport_and_url(val);
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
                    enabled: val.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                }
            })
            .collect()
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        vec![]
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        vec![]
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        self.workspace_dirs()
            .iter()
            .flat_map(|ws| ["AGENTS.md", "SOUL.md", "IDENTITY.md"].map(|f| ws.join(f)))
            .collect()
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        self.workspace_dirs()
            .iter()
            .flat_map(|ws| {
                ["MEMORY.md", "USER.md"]
                    .map(|f| ws.join(f))
                    .into_iter()
                    .chain(super::files_with_ext(&ws.join("memory"), "md").collect::<Vec<_>>())
            })
            .collect()
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.config_path.clone()]
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    fn write_config(dir: &Path, content: &str) -> PathBuf {
        let openclaw = dir.join(".openclaw");
        std::fs::create_dir_all(&openclaw).unwrap();
        let path = openclaw.join("openclaw.json");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn detect_and_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let a = OpenClawAdapter::with_home(tmp.path().to_path_buf());
        assert!(!a.detect());
        std::fs::create_dir_all(tmp.path().join(".openclaw")).unwrap();
        assert!(a.detect());
        let ws = tmp.path().join(".openclaw").join("workspace");
        assert_eq!(
            a.skill_dirs(),
            vec![
                tmp.path().join(".openclaw").join("skills"),
                ws.join("skills"),
                ws.join(".agents").join("skills"),
                tmp.path().join(".agents").join("skills"),
            ]
        );
        assert_eq!(
            a.mcp_config_path(),
            tmp.path().join(".openclaw").join("openclaw.json")
        );
    }

    #[test]
    fn env_overrides_follow_the_gateway() {
        let home = PathBuf::from("/home/u");
        let state = OpenClawAdapter::from_env(
            home.clone(),
            Some("/srv/claw".into()),
            None,
            Some("/srv/ws".into()),
        );
        assert_eq!(state.base_dir(), PathBuf::from("/srv/claw"));
        assert_eq!(
            state.mcp_config_path(),
            PathBuf::from("/srv/claw/openclaw.json")
        );
        let skills = state.skill_dirs();
        assert_eq!(skills[0], PathBuf::from("/srv/claw/skills"));
        assert!(skills.contains(&PathBuf::from("/srv/ws/skills")));
        // a non-default state dir drops the personal ~/.agents/skills root
        assert!(!skills.contains(&home.join(".agents").join("skills")));

        // only the config path: the managed dir follows the config file,
        // while the state dir (and so the personal root) stays default
        let cfg =
            OpenClawAdapter::from_env(home.clone(), None, Some("/etc/claw/oc.json".into()), None);
        assert_eq!(cfg.base_dir(), home.join(".openclaw"));
        assert_eq!(cfg.mcp_config_path(), PathBuf::from("/etc/claw/oc.json"));
        let skills = cfg.skill_dirs();
        assert_eq!(skills[0], PathBuf::from("/etc/claw/skills"));
        assert!(skills.contains(&home.join(".agents").join("skills")));

        // both set: the state dir owns the managed skills, not the config's
        // folder; env values are trimmed and `~`-expanded like config values
        let both = OpenClawAdapter::from_env(
            home.clone(),
            Some(" ~/claw ".into()),
            Some("/etc/claw/oc.json".into()),
            None,
        );
        assert_eq!(both.base_dir(), home.join("claw"));
        assert_eq!(both.mcp_config_path(), PathBuf::from("/etc/claw/oc.json"));
        assert_eq!(both.skill_dirs()[0], home.join("claw").join("skills"));
    }

    #[test]
    fn workspaces_follow_the_agent_roster() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            r#"{
  agents: {
    defaults: { workspace: '~/claw' },
    entries: {
      main: { default: true },
      research: { workspace: ' /srv/research-ws ' },
      Ops: { workspace: 'relative/ignored' },
      stale: null,
    },
  },
}"#,
        );
        let a = OpenClawAdapter::with_home(tmp.path().to_path_buf());
        let claw = tmp.path().join("claw");
        let mut expected = vec![
            PathBuf::from("/srv/research-ws"),
            claw.clone(),
            claw.join("ops"),
        ];
        expected.sort();
        assert_eq!(a.workspace_dirs(), expected);
    }

    #[test]
    fn roster_without_defaults_uses_state_dir_workspaces() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            // older `agents.list` roster; two entries, so nobody is the owner
            r#"{ agents: { list: [ { id: 'a', workspace: '~' }, { id: 'b' } ] } }"#,
        );
        let a = OpenClawAdapter::with_home(tmp.path().to_path_buf());
        assert_eq!(
            a.workspace_dirs(),
            vec![
                tmp.path().to_path_buf(),
                tmp.path().join(".openclaw").join("workspace-b"),
            ]
        );
    }

    #[test]
    fn skill_roots_are_not_repeated() {
        let tmp = tempfile::tempdir().unwrap();
        // a workspace at the state dir would alias the managed skills dir
        write_config(
            tmp.path(),
            r#"{ agents: { defaults: { workspace: '~/.openclaw' } } }"#,
        );
        let a = OpenClawAdapter::with_home(tmp.path().to_path_buf());
        let state = tmp.path().join(".openclaw");
        assert_eq!(
            a.skill_dirs(),
            vec![
                state.join("skills"),
                state.join(".agents").join("skills"),
                tmp.path().join(".agents").join("skills"),
            ]
        );
    }

    #[test]
    fn read_mcp_servers_parses_json5_with_comments_and_shorthand() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            r#"{
  // hand-written JSON5: comments, unquoted keys, single quotes, trailing commas
  mcp: {
    servers: {
      'fs': { command: 'npx', args: ['-y', 'srv'], env: { K: 'V' } },
      web: { url: 'https://x/mcp', transport: 'streamable-http' },
      off: { command: 'c', enabled: false },
    },
  },
}"#,
        );
        let a = OpenClawAdapter::with_home(tmp.path().to_path_buf());
        let servers = a.read_mcp_servers();
        assert_eq!(servers.len(), 3);
        let fs = servers.iter().find(|s| s.name == "fs").unwrap();
        assert_eq!(fs.transport, McpTransport::Stdio);
        assert_eq!(fs.command, "npx");
        assert_eq!(fs.args, vec!["-y".to_string(), "srv".to_string()]);
        assert_eq!(fs.env["K"], "V");
        assert!(fs.enabled);
        let web = servers.iter().find(|s| s.name == "web").unwrap();
        assert_eq!(web.transport, McpTransport::Http);
        assert_eq!(web.url.as_deref(), Some("https://x/mcp"));
        let off = servers.iter().find(|s| s.name == "off").unwrap();
        assert!(!off.enabled);
    }

    #[test]
    fn transport_follows_gateway_precedence() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            r#"{
  mcp: {
    servers: {
      bare_url: { url: 'https://x/sse' },
      command_and_url: { command: 'c', url: 'https://x/u', transport: 'streamable-http' },
      legacy_alias: { url: 'https://x/l', type: 'http' },
      transport_beats_type: { url: 'https://x/t', type: 'http', transport: 'SSE' },
      blank_url: { url: '  ' },
    },
  },
}"#,
        );
        let a = OpenClawAdapter::with_home(tmp.path().to_path_buf());
        let servers = a.read_mcp_servers();
        let by_name = |n: &str| servers.iter().find(|s| s.name == n).unwrap();
        let bare = by_name("bare_url");
        assert_eq!(bare.transport, McpTransport::Sse);
        assert_eq!(bare.url.as_deref(), Some("https://x/sse"));
        let both = by_name("command_and_url");
        assert_eq!(both.transport, McpTransport::Stdio);
        assert_eq!(both.command, "c");
        assert_eq!(both.url, None);
        assert_eq!(by_name("legacy_alias").transport, McpTransport::Http);
        assert_eq!(by_name("transport_beats_type").transport, McpTransport::Sse);
        let blank = by_name("blank_url");
        assert_eq!(
            (blank.transport, blank.url.as_deref()),
            (McpTransport::Stdio, None)
        );
    }

    #[test]
    fn read_mcp_servers_tolerates_missing_or_non_object_config() {
        let tmp = tempfile::tempdir().unwrap();
        let a = OpenClawAdapter::with_home(tmp.path().to_path_buf());
        assert!(a.read_mcp_servers().is_empty());
        write_config(tmp.path(), "[1,2,3]");
        assert!(a.read_mcp_servers().is_empty());
    }

    #[test]
    fn rules_and_memory_map_to_workspace_bootstrap_files() {
        let tmp = tempfile::tempdir().unwrap();
        let a = OpenClawAdapter::with_home(tmp.path().to_path_buf());
        let ws = tmp.path().join(".openclaw").join("workspace");
        assert_eq!(
            a.global_rules_files(),
            vec![
                ws.join("AGENTS.md"),
                ws.join("SOUL.md"),
                ws.join("IDENTITY.md")
            ]
        );
        let memory = a.global_memory_files();
        assert!(memory.contains(&ws.join("MEMORY.md")));
        assert!(memory.contains(&ws.join("USER.md")));
    }
}
