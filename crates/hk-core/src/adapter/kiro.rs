// Kiro IDE / CLI config references:
// - Hooks:   https://kiro.dev/docs/hooks/
// - MCP:     https://kiro.dev/docs/mcp/configuration/
// - Steering https://kiro.dev/docs/steering/
// - Skills:  https://kiro.dev/docs/cli/skills/
//
// Hooks are IDE Agent Hooks in `.kiro/hooks/*.json`, not CLI custom-agent
// hooks embedded in `.kiro/agents/*.json`.

use super::{
    AgentAdapter, HookEntry, HookFormat, McpServerEntry, ProjectMarker, RemoteMcpSchema,
};
use crate::models::ConfigScope;
use std::path::{Path, PathBuf};

pub struct KiroAdapter {
    home: PathBuf,
}

impl Default for KiroAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KiroAdapter {
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

    fn json_files(dir: &Path) -> Vec<PathBuf> {
        super::files_with_ext(dir, "json")
            .filter(|p| p.is_file())
            .collect()
    }

    fn parse_hook(value: &serde_json::Value) -> Option<HookEntry> {
        let action = value.get("action")?.as_object()?;
        if action.get("type").and_then(|v| v.as_str()) != Some("command") {
            return None;
        }
        let command = action.get("command")?.as_str()?;
        Some(HookEntry {
            event: value.get("trigger")?.as_str()?.to_string(),
            matcher: value
                .get("matcher")
                .and_then(|v| v.as_str())
                .map(String::from),
            command: command.to_string(),
            // Kiro's native per-hook flag: default true, false = skipped
            // without deleting (https://kiro.dev/docs/hooks/).
            enabled: value
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        })
    }
}

impl AgentAdapter for KiroAdapter {
    fn name(&self) -> &str {
        "kiro"
    }

    fn base_dir(&self) -> PathBuf {
        self.home.join(".kiro")
    }

    fn detect(&self) -> bool {
        self.base_dir().exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("skills")]
    }

    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("settings").join("mcp.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("hooks").join("harnesskit.json")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![]
    }

    fn hook_format(&self) -> HookFormat {
        HookFormat::KiroIde
    }

    fn supports_native_mcp_toggle(&self) -> bool {
        true
    }

    /// Kiro's docs describe `~/.kiro/hooks/` (user-level), but no released
    /// version loads it — verified on-device with 1.0.89 (macOS): the IDE
    /// only counts `.kiro/hooks/` workspace hooks. Upstream:
    /// kirodotdev/Kiro#5440 (open feature request, Feb 2026) and
    /// kirodotdev/Kiro#9857 (open bug, "not loaded in 1.0.52"). Flip this
    /// back to `true` once upstream ships user-level loading.
    fn supports_global_hook_install(&self) -> bool {
        false
    }

    fn hook_config_paths_for(&self, scope: &ConfigScope) -> Vec<PathBuf> {
        match scope {
            ConfigScope::Global => Self::json_files(&self.base_dir().join("hooks")),
            ConfigScope::Project { path, .. } => {
                Self::json_files(&Path::new(path).join(".kiro").join("hooks"))
            }
        }
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
            .map(|(name, val)| {
                // Remote entries: {url, headers} — protocol auto-detected by Kiro.
                let (transport, url) = super::parse_plain_url(val, "url");
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
                    enabled: !val
                        .get("disabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                }
            })
            .collect()
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::PlainUrl
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        self.hook_config_paths_for(&ConfigScope::Global)
            .into_iter()
            .flat_map(|path| self.read_hooks_from(&path))
            .collect()
    }

    fn read_hooks_from(&self, path: &Path) -> Vec<HookEntry> {
        let Some(config) = Self::parse_json(path) else {
            return vec![];
        };
        let Some(hooks) = config.get("hooks").and_then(|v| v.as_array()) else {
            return vec![];
        };
        hooks.iter().filter_map(Self::parse_hook).collect()
    }

    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_kiro(event)
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        super::files_with_ext(&self.base_dir().join("steering"), "md").collect()
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.mcp_config_path()];
        files.extend(Self::json_files(&self.base_dir().join("hooks")));
        files
    }

    fn global_subagent_files(&self) -> Vec<PathBuf> {
        // Two coexisting formats in the same dir: Kiro CLI agents are *.json,
        // IDE custom subagents are *.md with YAML front matter
        // (https://kiro.dev/docs/chat/subagents/).
        let agents_dir = self.base_dir().join("agents");
        let mut files = Self::json_files(&agents_dir);
        files.extend(super::files_with_ext(&agents_dir, "md"));
        files
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        vec![ProjectMarker::Dir(".kiro")]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![".kiro/steering/*.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![
            ".kiro/settings/mcp.json".into(),
            ".kiro/hooks/*.json".into(),
        ]
    }

    fn project_subagent_patterns(&self) -> Vec<String> {
        vec![".kiro/agents/*.json".into(), ".kiro/agents/*.md".into()]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        vec![".kiro/skills".into()]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        Some(".kiro/settings/mcp.json".into())
    }

    fn project_hook_config_relpath(&self) -> Option<String> {
        Some(".kiro/hooks/harnesskit.json".into())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AgentAdapter, McpTransport};
    use super::*;

    #[test]
    fn read_mcp_servers_parses_url_entries() {
        // Remote schema per kiro.dev/docs/mcp/configuration — the url no
        // longer round-trips through the command field.
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("mcp.json");
        std::fs::write(
            &config,
            r#"{"mcpServers":{
                "gh":{"url":"https://api.github.com/mcp","headers":{"X-K":"v"}},
                "fs":{"command":"npx","args":["-y","srv"]}
            }}"#,
        )
        .unwrap();
        let adapter = KiroAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers_from(&config);
        let gh = servers.iter().find(|s| s.name == "gh").unwrap();
        assert_eq!(gh.transport, McpTransport::Http);
        assert_eq!(gh.url.as_deref(), Some("https://api.github.com/mcp"));
        assert_eq!(gh.command, "", "url must not leak into command");
        assert_eq!(gh.headers["X-K"], "v");
    }

    #[test]
    fn detect_requires_kiro_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = KiroAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());

        std::fs::create_dir_all(tmp.path().join(".kiro")).unwrap();
        assert!(adapter.detect());
    }

    #[test]
    fn read_mcp_servers_reads_disabled_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = KiroAdapter::with_home(tmp.path().to_path_buf());
        let cfg = tmp.path().join(".kiro/settings/mcp.json");
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(
            &cfg,
            r#"{"mcpServers":{"lint":{"command":"npm","args":["run","lint"],"disabled":true}}}"#,
        )
        .unwrap();

        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "lint");
        assert!(!servers[0].enabled);
    }

    #[test]
    fn read_hooks_reads_command_actions_only() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = KiroAdapter::with_home(tmp.path().to_path_buf());
        let hooks_dir = tmp.path().join(".kiro/hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        std::fs::write(
            hooks_dir.join("lint.json"),
            r#"{
              "version": "v1",
              "hooks": [
                {
                  "name": "lint-on-save",
                  "trigger": "PostFileSave",
                  "matcher": "\\.ts$",
                  "action": { "type": "command", "command": "npm run lint" },
                  "enabled": false
                },
                {
                  "name": "ask",
                  "trigger": "Stop",
                  "action": { "type": "agent", "prompt": "summarize" }
                }
              ]
            }"#,
        )
        .unwrap();

        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].event, "PostFileSave");
        assert_eq!(hooks[0].matcher.as_deref(), Some("\\.ts$"));
        assert_eq!(hooks[0].command, "npm run lint");
        assert!(
            !hooks[0].enabled,
            "natively disabled hook must be listed as disabled, not dropped"
        );
    }

    #[test]
    fn subagents_cover_cli_json_and_ide_markdown() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = KiroAdapter::with_home(tmp.path().to_path_buf());
        let agents_dir = tmp.path().join(".kiro/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("cli-agent.json"), r#"{"name":"cli"}"#).unwrap();
        std::fs::write(
            agents_dir.join("reviewer.md"),
            "---\nname: reviewer\n---\nYou are a code reviewer.\n",
        )
        .unwrap();

        let files = adapter.global_subagent_files();
        let names: Vec<String> = files
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect();
        assert!(names.contains(&"cli-agent.json".to_string()), "{names:?}");
        assert!(names.contains(&"reviewer.md".to_string()), "{names:?}");

        assert_eq!(
            adapter.project_subagent_patterns(),
            vec![".kiro/agents/*.json", ".kiro/agents/*.md"]
        );
    }
}
