// Hook reference:     https://docs.windsurf.com/windsurf/cascade/hooks
// Config file:        ~/.codeium/windsurf/hooks.json (global), .windsurf/hooks.json (project)
// Format:             JSON, top-level key "hooks", sub-keys: command (or powershell)
//
// Workflow reference: https://docs.windsurf.com/windsurf/cascade/workflows
// Files:              ~/.codeium/windsurf/global_workflows/*.md (global)
//                     .windsurf/workflows/*.md (project)
//
// Ignore reference:   https://docs.windsurf.com/context-awareness/windsurf-ignore
// File:               .codeiumignore (project root)

use super::{AgentAdapter, HookEntry, HookFormat, McpServerEntry};
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
        self.base_dir().exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.base_dir().join("skills"),
            self.home.join(".agents").join("skills"),
        ]
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
            .map(|(name, val)| McpServerEntry {
                name: name.clone(),
                command: val
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
                args: val
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                env: val
                    .get("env")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect()
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
                    });
                }
            }
        }
        entries
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("global_rules.md")]
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
            .collect()
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.mcp_config_path(), self.hook_config_path()]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![".windsurfrules".into(), ".windsurf/rules/*.md".into()]
    }

    fn project_memory_patterns(&self) -> Vec<String> {
        vec![".windsurf/memories/*.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![
            ".windsurf/mcp_config.json".into(),
            ".windsurf/hooks.json".into(),
        ]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".codeiumignore".into()]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        Some(".windsurf/mcp_config.json".into())
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
        vec![".windsurf/workflows/*.md".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    #[test]
    fn detect_requires_base_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());

        std::fs::create_dir_all(tmp.path().join(".codeium/windsurf")).unwrap();
        assert!(adapter.detect());
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
            hook.event == "post_cascade_response"
                && hook.command == "python C:\\hooks\\log.py"
        }));
    }

    #[test]
    fn global_memory_files_reads_markdown_files() {
        let tmp = tempfile::tempdir().unwrap();
        let memories_dir = tmp.path().join(".codeium/windsurf/memories");
        std::fs::create_dir_all(&memories_dir).unwrap();
        std::fs::write(memories_dir.join("one.md"), "# One").unwrap();
        std::fs::write(memories_dir.join("two.txt"), "skip").unwrap();

        let adapter = WindsurfAdapter::with_home(tmp.path().to_path_buf());
        let memories = adapter.global_memory_files();
        assert_eq!(memories.len(), 1);
        assert!(memories[0].ends_with(".codeium/windsurf/memories/one.md"));
    }

    #[test]
    fn project_ignore_patterns_includes_codeiumignore() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let patterns = adapter.project_ignore_patterns();
        assert!(patterns.contains(&".codeiumignore".to_string()));
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
        assert!(!files.iter().any(|p| p.to_string_lossy().contains("global_workflows")));
    }

    #[test]
    fn project_workflow_patterns_includes_workflows_dir() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let patterns = adapter.project_workflow_patterns();
        assert_eq!(patterns, vec![".windsurf/workflows/*.md".to_string()]);
    }

    #[test]
    fn project_settings_patterns_excludes_workflows() {
        let adapter = WindsurfAdapter::with_home(tempfile::tempdir().unwrap().path().to_path_buf());
        let patterns = adapter.project_settings_patterns();
        assert!(!patterns.iter().any(|p| p.contains("workflows")));
    }
}
