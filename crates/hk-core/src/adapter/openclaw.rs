// OpenClaw gateway — data root ~/.openclaw (override: OPENCLAW_STATE_DIR).
// Layout verified against official docs (2026-09):
//
// MCP: `openclaw.json` (JSON5) under the nested `mcp.servers` key. Each entry
// is command-based ({command, args?, env?, cwd?}) or URL-based ({url,
// transport: "streamable-http"|"sse", headers?}). Entries carry a native
// `enabled` flag, so toggling flips it in place (like Hermes).
// Docs: https://docs.openclaw.ai/tools/mcp
//
// Skills: standard `SKILL.md` dirs — `<state>/skills` (managed) plus the
// universal `~/.agents/skills` personal root OpenClaw also loads.
// Docs: https://docs.openclaw.ai/tools/skills
//
// Memory/rules: the agent workspace (`~/.openclaw/workspace`, override
// OPENCLAW_WORKSPACE_DIR) holds AGENTS.md / SOUL.md (instructions → Rules)
// and MEMORY.md + memory/YYYY-MM-DD.md (→ Memory).
// Docs: https://docs.openclaw.ai/concepts/agent-workspace
//
// Hooks (JS handlers + HTTP ingress) are a different model from the
// Claude-style command hooks HarnessKit manages, so they are not scanned or
// installed here (HookFormat::None). Plugins and remote MCP writes are
// follow-ups.
//
// Global-only by design: OpenClaw loads skills from the gateway state dir,
// not from an arbitrary project tree (docs above), so it declares no
// project_skill_dirs — pinned in adapter::tests alongside hermes.

use super::{
    AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, PluginEntry, RemoteMcpSchema,
};
use std::path::{Path, PathBuf};

pub struct OpenClawAdapter {
    home: PathBuf,
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

impl OpenClawAdapter {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
        }
    }
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    fn state_dir_env() -> Option<PathBuf> {
        std::env::var_os("OPENCLAW_STATE_DIR")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    }

    fn workspace_dir(&self) -> PathBuf {
        std::env::var_os("OPENCLAW_WORKSPACE_DIR")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.base_dir().join("workspace"))
    }

    fn parse_json5(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        jsonc_parser::parse_to_serde_value(&content, &json5_parse_options())
            .ok()
            .flatten()
    }
}

impl AgentAdapter for OpenClawAdapter {
    fn name(&self) -> &str {
        "openclaw"
    }
    fn base_dir(&self) -> PathBuf {
        Self::state_dir_env().unwrap_or_else(|| self.home.join(".openclaw"))
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
        self.base_dir().join("openclaw.json")
    }
    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("openclaw.json")
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
        self.read_mcp_servers_from(&self.mcp_config_path())
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
                // OpenClaw spells the transport under `transport`; the
                // shared reader expects `type` and already maps
                // "streamable-http" → Http.
                let mut normalized = val.clone();
                if let (Some(t), Some(obj)) = (val.get("transport"), normalized.as_object_mut()) {
                    if !obj.contains_key("type") {
                        obj.insert("type".into(), t.clone());
                    }
                }
                let (transport, url) = super::parse_type_url(&normalized);
                McpServerEntry {
                    name: name.clone(),
                    command: normalized
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .into(),
                    args: super::json_string_vec(&normalized, "args"),
                    env: super::json_string_map(&normalized, "env"),
                    transport,
                    url,
                    headers: super::json_string_map(&normalized, "headers"),
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
        let ws = self.workspace_dir();
        vec![ws.join("AGENTS.md"), ws.join("SOUL.md")]
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        let ws = self.workspace_dir();
        let mut files = vec![ws.join("MEMORY.md")];
        files.extend(super::files_with_ext(&ws.join("memory"), "md").collect::<Vec<_>>());
        files
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.mcp_config_path()]
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AgentAdapter, McpTransport};
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
        assert_eq!(
            a.skill_dirs(),
            vec![
                tmp.path().join(".openclaw").join("skills"),
                tmp.path().join(".agents").join("skills"),
            ]
        );
        assert_eq!(
            a.mcp_config_path(),
            tmp.path().join(".openclaw").join("openclaw.json")
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
        let rules = a.global_rules_files();
        assert!(rules.contains(&ws.join("AGENTS.md")));
        assert!(rules.contains(&ws.join("SOUL.md")));
        let memory = a.global_memory_files();
        assert!(memory.contains(&ws.join("MEMORY.md")));
    }
}
