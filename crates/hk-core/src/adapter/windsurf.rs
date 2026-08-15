// Windsurf is now Devin Desktop (Cognition acquisition), and the IDE ships two
// agents with separate config systems. This adapter models the Cascade agent
// (the legacy Windsurf one): its global config explicitly did NOT move — the
// FAQ says the per-user paths "remain unchanged" — while the workspace side
// now prefers `.devin/` with `.windsurf/` as fallback. The successor agent
// (Devin Local / Devin CLI, `~/.config/devin` + `.devin/config.json`) is a
// different config system entirely and belongs in its own future adapter.
// FAQ:                https://docs.devin.ai/desktop/devin-desktop-faq
//
// Hook reference:     https://docs.devin.ai/desktop/cascade/hooks
// Config file:        ~/.codeium/windsurf/hooks.json (global), .windsurf/hooks.json (project)
// Format:             JSON, top-level key "hooks", sub-keys: command (or powershell)
//
// Workflow reference: https://docs.devin.ai/desktop/cascade/workflows
// Files:              ~/.codeium/windsurf/global_workflows/*.md (global)
//                     .devin/workflows/*.md, .windsurf/workflows/*.md (project)
//
// Ignore reference:   https://docs.devin.ai/desktop/context-awareness/windsurf-ignore
// Files:              .devinignore, .codeiumignore, .windsurfignore (project root)

use super::{
    AgentAdapter, HookEntry, HookFormat, McpServerEntry, ProjectMarker, RemoteMcpSchema,
};
use std::path::{Path, PathBuf};

pub struct WindsurfAdapter {
    home: PathBuf,
}

impl Default for WindsurfAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WindsurfAdapter {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    fn read_json(&self, path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl AgentAdapter for WindsurfAdapter {
    fn hook_format(&self) -> HookFormat {
        HookFormat::Windsurf
    }

    fn name(&self) -> &str {
        "windsurf"
    }

    fn needs_path_injection(&self) -> bool {
        true
    }

    fn base_dir(&self) -> PathBuf {
        self.home.join(".codeium").join("windsurf")
    }

    fn detect(&self) -> bool {
        // `~/.devin/extensions` is Devin Desktop's new read+write extensions
        // dir (per the FAQ) and marks a fresh install that never had the
        // Codeium-era directory. Deliberately NOT bare `~/.devin`: that dir is
        // also created by Devin CLI sessions (plans, optional global rules),
        // which are a different agent this adapter must not claim.
        self.base_dir().exists() || self.home.join(".devin").join("extensions").exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        // Workspace dirs moved with the Devin Desktop rename: `.devin/` is
        // primary, `.windsurf/` the backward-compat fallback (FAQ table).
        vec![".devin/skills".into(), ".windsurf/skills".into()]
    }

    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("mcp_config.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        self.base_dir().join("hooks.json")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![]
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Some(config) = self.read_json(path) else {
            return vec![];
        };
        let Some(servers) = config.get("mcpServers").and_then(|v| v.as_object()) else {
            return vec![];
        };

        servers
            .iter()
            .map(|(name, val)| {
                // Remote entries: {serverUrl, headers} — protocol auto-detected.
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
                    // Windsurf's MCP schema has no agent-native disable concept.
                    enabled: true,
                }
            })
            .collect()
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::ServerUrl
    }

    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_windsurf(event)
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        self.read_hooks_from(&self.hook_config_path())
    }

    fn read_hooks_from(&self, path: &Path) -> Vec<HookEntry> {
        let Some(config) = self.read_json(path) else {
            return vec![];
        };
        let Some(hooks) = config.get("hooks").and_then(|v| v.as_object()) else {
            return vec![];
        };

        let mut entries = Vec::new();
        for (event, hook_list) in hooks {
            let Some(arr) = hook_list.as_array() else {
                continue;
            };
            for hook in arr {
                let command = hook
                    .get("command")
                    .and_then(|v| v.as_str())
                    .or_else(|| hook.get("powershell").and_then(|v| v.as_str()));
                if let Some(command) = command {
                    entries.push(HookEntry {
                        event: event.clone(),
                        matcher: None,
                        command: command.to_string(),
                        enabled: true,
                    });
                }
            }
        }
        entries
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        // The documented location is memories/global_rules.md. The bare
        // base-dir path was what this adapter scanned before v1.8.3 (no
        // upstream doc ever placed it there); it stays listed so a file a
        // user created through HK at the old location remains visible.
        // Non-existent candidates are filtered by the scanner.
        vec![
            self.base_dir().join("memories").join("global_rules.md"),
            self.base_dir().join("global_rules.md"),
        ]
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        let memory_dir = self.base_dir().join("memories");
        let Ok(entries) = std::fs::read_dir(memory_dir) else {
            return vec![];
        };

        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
            // global_rules.md lives inside memories/ but is a rules file
            // (listed by global_rules_files), not an autogenerated memory.
            .filter(|path| path.file_name().is_none_or(|n| n != "global_rules.md"))
            .collect()
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.mcp_config_path(), self.hook_config_path()]
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        vec![
            ProjectMarker::Dir(".devin"),
            ProjectMarker::Dir(".windsurf"),
            ProjectMarker::File(".windsurfrules"),
        ]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        // `.devin/rules` is the preferred location ("When you create a new
        // rule, it will be saved in the .devin/rules directory"), `.windsurf`
        // the fallback. Rules discovery in the official binary (Devin Desktop
        // 3.6.27) uses the doublestar glob `**/.windsurf/rules/**/*.md`, so
        // nested subdirectories inside rules/ are loaded.
        vec![
            ".devin/rules/**/*.md".into(),
            ".windsurfrules".into(),
            ".windsurf/rules/**/*.md".into(),
        ]
    }

    fn project_memory_patterns(&self) -> Vec<String> {
        vec![".windsurf/memories/*.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        // No `.windsurf/mcp_config.json` here: Windsurf MCP is global-only
        // (see `project_mcp_config_relpath`), so a workspace copy would be a
        // file Windsurf never reads.
        vec![".windsurf/hooks.json".into()]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        // `.devinignore` is the primary name post-rename; the other two are
        // documented as still supported, and all can coexist.
        vec![
            ".devinignore".into(),
            ".codeiumignore".into(),
            ".windsurfignore".into(),
        ]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        // Windsurf MCP is global-only: the official MCP doc documents a
        // single config at `~/.codeium/windsurf/mcp_config.json` and never
        // mentions a workspace path — unlike the skills and hooks docs on
        // the same site, which explicitly scope `.windsurf/skills/` and
        // `.windsurf/hooks.json` to the workspace. Third-party guides
        // confirm ("Windsurf doesn't load a project-scoped copy").
        // Source: https://docs.devin.ai/desktop/cascade/mcp
        None
    }

    fn project_hook_config_relpath(&self) -> Option<String> {
        Some(".windsurf/hooks.json".into())
    }

    fn global_workflow_files(&self) -> Vec<PathBuf> {
        let workflows_dir = self.base_dir().join("global_workflows");
        let Ok(entries) = std::fs::read_dir(&workflows_dir) else {
            return vec![];
        };
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
            .collect()
    }

    fn project_workflow_patterns(&self) -> Vec<String> {
        vec![
            ".devin/workflows/*.md".into(),
            ".windsurf/workflows/*.md".into(),
        ]
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
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
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
    fn detect_accepts_codeium_era_and_devin_era_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());

        std::fs::create_dir_all(tmp.path().join(".codeium/windsurf")).unwrap();
        assert!(adapter.detect());

        // A fresh Devin Desktop install has ~/.devin/extensions but no
        // ~/.codeium.
        let tmp = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join(".devin/extensions")).unwrap();
        assert!(adapter.detect());

        // Bare ~/.devin without extensions/ is Devin CLI territory (plans,
        // optional global rules) — a different agent; must NOT detect.
        let tmp = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join(".devin/rules")).unwrap();
        assert!(!adapter.detect());
    }

    #[test]
    fn global_rules_prefer_the_documented_memories_location() {
        // Docs place global rules at memories/global_rules.md; the bare
        // base-dir path stays listed only because this adapter scanned it
        // before v1.8.3 and a user may have created a file there through HK.
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let rules = adapter.global_rules_files();
        assert!(rules[0].ends_with(".codeium/windsurf/memories/global_rules.md"));
        assert!(rules[1].ends_with(".codeium/windsurf/global_rules.md"));
    }

    #[test]
    fn read_mcp_servers_reads_json_config() {
        let tmp = tempfile::tempdir().unwrap();
        let base_dir = tmp.path().join(".codeium/windsurf");
        std::fs::create_dir_all(&base_dir).unwrap();
        std::fs::write(
            base_dir.join("mcp_config.json"),
            r#"{"mcpServers":{"github":{"command":"npx","args":["-y","server"],"env":{"TOKEN":"abc"}}}}"#,
        )
        .unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "github");
        assert_eq!(servers[0].command, "npx");
        assert_eq!(servers[0].args, vec!["-y", "server"]);
        assert_eq!(servers[0].env.get("TOKEN"), Some(&"abc".to_string()));
    }

    #[test]
    fn read_hooks_reads_hooks_json() {
        let tmp = tempfile::tempdir().unwrap();
        let base_dir = tmp.path().join(".codeium/windsurf");
        std::fs::create_dir_all(&base_dir).unwrap();
        std::fs::write(
            base_dir.join("hooks.json"),
            r#"{"hooks":{"pre_user_prompt":[{"command":"python3 /tmp/check.py"}],"post_cascade_response":[{"powershell":"python C:\\hooks\\log.py"}]}}"#,
        )
        .unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 2);
        assert!(hooks.iter().any(|hook| {
            hook.event == "pre_user_prompt" && hook.command == "python3 /tmp/check.py"
        }));
        assert!(hooks.iter().any(|hook| {
            hook.event == "post_cascade_response" && hook.command == "python C:\\hooks\\log.py"
        }));
    }

    #[test]
    fn global_memory_files_reads_markdown_files() {
        let tmp = tempfile::tempdir().unwrap();
        let memories_dir = tmp.path().join(".codeium/windsurf/memories");
        std::fs::create_dir_all(&memories_dir).unwrap();
        std::fs::write(memories_dir.join("one.md"), "# One").unwrap();
        std::fs::write(memories_dir.join("two.txt"), "skip").unwrap();
        // Lives in memories/ but belongs to global_rules_files — listing it
        // here too would show the same file under both Rules and Memory.
        std::fs::write(memories_dir.join("global_rules.md"), "# Rules").unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let memories = adapter.global_memory_files();
        assert_eq!(memories.len(), 1);
        assert!(memories[0].ends_with(".codeium/windsurf/memories/one.md"));
    }

    #[test]
    fn project_ignore_patterns_cover_all_three_supported_names() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let patterns = adapter.project_ignore_patterns();
        assert!(patterns.contains(&".devinignore".to_string()));
        assert!(patterns.contains(&".codeiumignore".to_string()));
        assert!(patterns.contains(&".windsurfignore".to_string()));
    }

    #[test]
    fn project_paths_prefer_devin_and_keep_windsurf_fallbacks() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        assert_eq!(
            adapter.project_skill_dirs(),
            vec![".devin/skills".to_string(), ".windsurf/skills".to_string()]
        );
        let rules = adapter.project_rules_patterns();
        assert_eq!(rules[0], ".devin/rules/**/*.md");
        assert!(rules.contains(&".windsurfrules".to_string()));
        assert!(rules.contains(&".windsurf/rules/**/*.md".to_string()));
        // Hooks did NOT move: Cascade's workspace hooks stay at
        // .windsurf/hooks.json (a `.devin/hooks.json` exists for neither
        // agent — the successor agent uses .devin/hooks.v1.json instead).
        assert_eq!(
            adapter.project_hook_config_relpath().as_deref(),
            Some(".windsurf/hooks.json")
        );
    }

    #[test]
    fn global_workflow_files_reads_markdown_files() {
        let tmp = tempfile::tempdir().unwrap();
        let workflows_dir = tmp.path().join(".codeium/windsurf/global_workflows");
        std::fs::create_dir_all(&workflows_dir).unwrap();
        std::fs::write(workflows_dir.join("deploy.md"), "# deploy").unwrap();
        std::fs::write(workflows_dir.join("notes.txt"), "skip").unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let files = adapter.global_workflow_files();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with(".codeium/windsurf/global_workflows/deploy.md"));
    }

    #[test]
    fn global_settings_files_excludes_workflows() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let files = adapter.global_settings_files();
        assert!(
            !files
                .iter()
                .any(|p| p.to_string_lossy().contains("global_workflows"))
        );
    }

    #[test]
    fn project_workflow_patterns_includes_workflows_dir() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let patterns = adapter.project_workflow_patterns();
        assert_eq!(
            patterns,
            vec![
                ".devin/workflows/*.md".to_string(),
                ".windsurf/workflows/*.md".to_string()
            ]
        );
    }

    #[test]
    fn project_settings_patterns_excludes_workflows() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let patterns = adapter.project_settings_patterns();
        assert!(!patterns.iter().any(|p| p.contains("workflows")));
    }

    #[test]
    fn project_mcp_is_none_global_only() {
        // Windsurf MCP has no workspace config (official docs document only
        // `~/.codeium/windsurf/mcp_config.json`); pin the deliberate None so
        // it isn't "fixed" back by symmetry with other adapters.
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        assert!(adapter.project_mcp_config_relpath().is_none());
        assert!(
            !adapter
                .project_settings_patterns()
                .iter()
                .any(|p| p.contains("mcp_config"))
        );
    }
}
