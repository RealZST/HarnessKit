// MCP config reference: https://antigravity.google/docs/mcp
// Config file: ~/.gemini/antigravity/mcp_config.json
// Format: JSON, top-level key "mcpServers", sub-keys: command, args, env, serverUrl, headers, etc.
//
// Data directory note: Antigravity has TWO directories on disk:
//   - ~/.antigravity/         → VS Code-fork IDE shell data (extensions/, argv.json) —
//                               undocumented by Google, inferred only from product.json
//                               `dataFolderName: ".antigravity"`. Not used as base_dir.
//   - ~/.gemini/antigravity/  → AI agent runtime data (skills, mcp_config.json,
//                               brain/, knowledge/, conversations/) — the path Google's
//                               docs and codelabs actually reference. Used as base_dir.

use super::{AgentAdapter, HookEntry, HookFormat, McpServerEntry, ProjectMarker};
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
        self.home.join(".gemini").join("antigravity")
    }
    fn detect(&self) -> bool {
        self.base_dir().exists()
    }
    fn skill_dirs(&self) -> Vec<PathBuf> {
        // Antigravity does NOT scan ~/.gemini/skills/ (Gemini CLI's path) or
        // ~/.agents/skills/ — cross-loading from Gemini CLI requires a manual
        // symlink per Google's own guidance.
        // Source: https://codelabs.developers.google.com/getting-started-with-antigravity-skills
        vec![self.base_dir().join("skills")]
    }
    fn project_skill_dirs(&self) -> Vec<String> {
        // Antigravity 1.18.4+ migrated from `.agent/` (singular) to `.agents/`
        // (plural). Both still load; `.agents/` is canonical going forward.
        // Source: https://discuss.ai.google.dev/t/new-folder-for-rules/126165
        vec![".agents/skills".into(), ".agent/skills".into()]
    }
    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("mcp_config.json")
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
        vec![self.base_dir().join("mcp_config.json")]
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
        vec![
            ".agents/rules/*.md".into(),
            ".agent/rules/*.md".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".geminiignore".into()]
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
                // Antigravity's MCP schema has no agent-native disable concept.
                enabled: true,
            })
            .collect()
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

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
}
