// Qoder CN (desktop bundle com.qodercn.app) — data root ~/.qoder-cn.
// Layout verified against a real install (2026-09).
//
// MCP: `settings.json` under the top-level `mcpServers` key, Claude-style
// entry schema ({command,args,env} plus {type,url,headers} for remote).
// Official docs: https://docs.qoder.cn/cli/mcp-servers
// A bare `mcp.json` also sits at the root; it is an empty placeholder the
// desktop app creates at startup (birth == mtime, never written, absent from
// the docs), so it is NOT a config surface here.
//
// Plugins: `plugins/installed_plugins_v2.json` is the authoritative manifest
// (per-entry `enabled`, installPath, precise timestamps). The sibling
// `installed_plugins-v2.json` (dash) and `installed_plugins.json` are older
// shapes without `enabled`; reading the underscore file covers both.
//
// Hooks: the `hooks` key of `settings.json` (user) and `.qoder/settings.json`
// (project), same shape and event names as Claude Code
// (https://docs.qoder.cn/cli/hooks-reference). No per-hook enabled flag, so
// toggling uses the default remove + DB snapshot path. Project hooks are
// skipped in untrusted folders only if the user turns on
// `security.folderTrust.enabled`, which is off unless set.

use super::{AgentAdapter, HookEntry, McpServerEntry, PluginEntry, ProjectMarker, RemoteMcpSchema};
use std::path::{Path, PathBuf};

pub struct QoderCnAdapter {
    home: PathBuf,
}

impl Default for QoderCnAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl QoderCnAdapter {
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

    /// Real cwd of a `projects/<encoded-cwd>/` dir, read from the `cwd` field
    /// of its session transcripts. The encoded dir name is lossy, so decode
    /// nothing and take the transcript value instead (same approach as Claude).
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

impl AgentAdapter for QoderCnAdapter {
    fn name(&self) -> &str {
        "qoder-cn"
    }
    fn base_dir(&self) -> PathBuf {
        self.home.join(".qoder-cn")
    }
    fn detect(&self) -> bool {
        self.base_dir().exists()
    }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        // `~/.qoder-cn/skills` is the product-native root (85 skills observed
        // loading from a live install); `~/.agents/skills` is the Universal
        // Agent Skills alias Qoder CN also loads (app.asar tags skill sources
        // "user" / ".agents/skills"; e.g. ~/.agents/skills/ego-browser loads).
        // Own dir first: install targets resolve through the canonical root.
        vec![
            self.base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ]
    }
    fn project_skill_dirs(&self) -> Vec<String> {
        vec![".agents/skills".into()]
    }
    fn project_markers(&self) -> Vec<super::ProjectMarker> {
        vec![
            ProjectMarker::Dir(".agents/skills"),
            ProjectMarker::Dir(".qoder/rules"),
        ]
    }
    fn project_rules_patterns(&self) -> Vec<String> {
        // `.qoder/rules/**/*.md` plus the AGENTS.md pair (docs.qoder.cn/cli/memory
        // calls them "static memory"; HK files them under Rules, same as the
        // global AGENTS.md above and Codex's AGENTS.md).
        vec![
            ".qoder/rules/**/*.md".into(),
            "AGENTS.md".into(),
            "AGENTS.local.md".into(),
        ]
    }
    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("settings.json")
    }
    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("settings.json")
    }
    fn project_hook_config_relpath(&self) -> Option<String> {
        Some(".qoder/settings.json".into())
    }
    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_claude(event)
    }
    fn plugin_dirs(&self) -> Vec<PathBuf> {
        // Plugins are manifest rows, not HK-owned directories: the app owns
        // `plugins/cache/<marketplace>/<name>/<version>` and toggles state in
        // the JSON manifests.
        vec![]
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
                    enabled: true,
                }
            })
            .collect()
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::TypeAndUrl
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        self.read_hooks_from(&self.hook_config_path())
    }

    fn read_hooks_from(&self, path: &Path) -> Vec<HookEntry> {
        super::read_claude_like_hooks(path)
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        let manifest_path = self
            .base_dir()
            .join("plugins")
            .join("installed_plugins_v2.json");
        let Some(manifest) = Self::parse_json(&manifest_path) else {
            return vec![];
        };
        let Some(plugins) = manifest.get("plugins").and_then(|v| v.as_object()) else {
            return vec![];
        };
        plugins
            .iter()
            .filter_map(|(key, entries)| {
                let entry = entries.as_array()?.first()?;
                let install_path = entry.get("installPath").and_then(|v| v.as_str())?;
                let ts = |field: &str| {
                    entry
                        .get(field)
                        .and_then(|v| v.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|d| d.with_timezone(&chrono::Utc))
                };
                Some(PluginEntry {
                    name: key.clone(),
                    source: key.split_once('@').map(|(_, m)| m.to_string())?,
                    enabled: entry
                        .get("enabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                    path: Some(PathBuf::from(install_path)),
                    source_url: None,
                    uri: None,
                    base_layers: vec![],
                    pack: None,
                    installed_at: ts("installedAt"),
                    updated_at: ts("lastUpdated"),
                })
            })
            .collect()
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        // AGENTS.md plus ~/.qoder-cn/rules/**/*.md (docs.qoder.cn/cli/memory).
        let mut files = vec![self.base_dir().join("AGENTS.md")];
        files.extend(super::files_with_ext_recursive(
            &self.base_dir().join("rules"),
            "md",
        ));
        files
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.mcp_config_path()]
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        super::files_with_ext(&self.base_dir().join("memory"), "md").collect()
    }

    fn external_project_memory(&self) -> Vec<(Option<PathBuf>, Vec<PathBuf>)> {
        // ~/.qoder-cn/projects/<encoded-cwd>/memory/*.md — per-project memory
        // stored outside the project tree, owner resolved from the dir's
        // session transcripts (Claude's pattern).
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
}

#[cfg(test)]
mod tests {
    use super::super::{AgentAdapter, McpTransport};
    use super::*;

    fn adapter_in(tmp: &Path) -> QoderCnAdapter {
        QoderCnAdapter::with_home(tmp.to_path_buf())
    }

    #[test]
    fn read_mcp_servers_parses_stdio_and_remote_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("settings.json");
        std::fs::write(
            &config,
            r#"{"mcpServers":{
                "stdio-srv":{"command":"npx","args":["-y","srv"],"env":{"K":"V"}},
                "remote":{"type":"sse","url":"https://example.com/sse","headers":{"Authorization":"Bearer t"}}
            }}"#,
        )
        .unwrap();
        let servers = adapter_in(tmp.path()).read_mcp_servers_from(&config);
        let stdio = servers.iter().find(|s| s.name == "stdio-srv").unwrap();
        assert_eq!(stdio.transport, McpTransport::Stdio);
        assert_eq!(stdio.env["K"], "V");
        let remote = servers.iter().find(|s| s.name == "remote").unwrap();
        assert_eq!(remote.transport, McpTransport::Sse);
        assert_eq!(remote.url.as_deref(), Some("https://example.com/sse"));
        assert_eq!(remote.headers["Authorization"], "Bearer t");
    }

    #[test]
    fn read_hooks_parses_claude_shaped_hooks_next_to_mcp_servers() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".qoder-cn");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"mcpServers":{"srv":{"command":"npx"}},"hooks":{
                "PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo pre"}]}],
                "Stop":[{"hooks":[{"type":"prompt","prompt":"check the work"}]}]
            }}"#,
        )
        .unwrap();
        let hooks = adapter_in(tmp.path()).read_hooks();
        assert_eq!(hooks.len(), 2);
        let pre = hooks.iter().find(|h| h.event == "PreToolUse").unwrap();
        assert_eq!(pre.matcher.as_deref(), Some("Bash"));
        assert_eq!(pre.command, "echo pre");
        let stop = hooks.iter().find(|h| h.event == "Stop").unwrap();
        assert_eq!(stop.command, "check the work");
    }

    #[test]
    fn read_plugins_parses_enabled_and_timestamps() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".qoder-cn").join("plugins");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("installed_plugins_v2.json"),
            r#"{"plugins":{
                "demo@qoder-marketplace":[{"enabled":false,"installPath":"/x/demo/1.0.0","version":"1.0.0","source":"marketplace","installedAt":"2026-08-25T07:27:57.000Z","lastUpdated":"2026-08-26T08:00:00.000Z"}]
            },"version":2}"#,
        )
        .unwrap();
        let plugins = adapter_in(tmp.path()).read_plugins();
        assert_eq!(plugins.len(), 1);
        let p = &plugins[0];
        assert_eq!(p.name, "demo@qoder-marketplace");
        assert_eq!(p.source, "qoder-marketplace");
        assert!(!p.enabled);
        assert_eq!(
            p.path.as_deref(),
            Some(std::path::Path::new("/x/demo/1.0.0"))
        );
        assert!(p.installed_at.is_some());
        assert!(p.updated_at.is_some());
    }

    #[test]
    fn external_project_memory_groups_files_with_session_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path().join(".qoder-cn").join("projects").join("-x-proj");
        std::fs::create_dir_all(proj.join("memory")).unwrap();
        std::fs::write(proj.join("memory").join("note.md"), "# note").unwrap();
        std::fs::write(
            proj.join("session.jsonl"),
            "{\"type\":\"user\"}\n{\"cwd\":\"/real/proj\"}\n",
        )
        .unwrap();
        let groups = adapter_in(tmp.path()).external_project_memory();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, Some(PathBuf::from("/real/proj")));
        assert_eq!(groups[0].1.len(), 1);
    }

    #[test]
    fn detect_and_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let a = adapter_in(tmp.path());
        assert!(!a.detect());
        std::fs::create_dir_all(tmp.path().join(".qoder-cn")).unwrap();
        assert!(a.detect());
        assert_eq!(
            a.skill_dirs(),
            vec![
                tmp.path().join(".qoder-cn").join("skills"),
                tmp.path().join(".agents").join("skills"),
            ]
        );
        assert_eq!(
            a.mcp_config_path(),
            tmp.path().join(".qoder-cn").join("settings.json")
        );
    }
}
