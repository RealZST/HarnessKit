// Config reference: https://hermes-agent.nousresearch.com/docs/user-guide/features/skills
// Skills: ~/.hermes/skills/{category}/{skill-name}/SKILL.md
//   - "local" is the conventional category for user-managed skills (e.g. installed by HK)
//   - Nous ships built-in skills in sibling category dirs (apple/, devops/, etc.)
// MCP:    ~/.hermes/config.yaml — "mcp_servers" YAML mapping
// Hooks:  ~/.hermes/config.yaml root `hooks:` key — list of {matcher?, command, timeout?}
//         per event (pre_tool_call/post_tool_call/on_session_start/...). YAML, McpFormat::HermesYaml-style.
// Plugins: ~/.hermes/hermes-agent/plugins/ contains internal model-provider adapters —
//          not user-installable extensions in the HK sense.

use super::{AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, ProjectMarker};
use std::path::{Path, PathBuf};

pub struct HermesAdapter {
    home: PathBuf,
}

impl Default for HermesAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HermesAdapter {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    /// List all category subdirectory names under `~/.hermes/skills/`.
    /// Returns sorted names, excluding hidden dirs. Used by the UI category picker.
    pub fn list_categories(&self) -> Vec<String> {
        let skills_root = self.base_dir().join("skills");
        let mut cats: Vec<String> = std::fs::read_dir(&skills_root)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if !p.is_dir() {
                    return None;
                }
                let name = p.file_name()?.to_str()?.to_string();
                if name.starts_with('.') {
                    return None;
                }
                Some(name)
            })
            .collect();
        cats.sort();
        cats
    }

    fn parse_yaml(path: &Path) -> Option<serde_yaml::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_yaml::from_str(&content).ok()
    }
}

impl AgentAdapter for HermesAdapter {
    fn name(&self) -> &str {
        "hermes"
    }

    fn base_dir(&self) -> PathBuf {
        self.home.join(".hermes")
    }

    fn detect(&self) -> bool {
        self.base_dir().exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        let skills_root = self.base_dir().join("skills");
        // "local" is the user-managed category: written first so skill_dir_for(Global)
        // returns it as the install target. Built-in Nous categories follow so the
        // scanner picks up all skills across all categories.
        let mut dirs = vec![skills_root.join("local")];

        if let Ok(entries) = std::fs::read_dir(&skills_root) {
            let mut extra: Vec<PathBuf> = entries
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.is_dir()
                        && p.file_name().and_then(|n| n.to_str()) != Some("local")
                    {
                        Some(p)
                    } else {
                        None
                    }
                })
                .collect();
            extra.sort();
            dirs.extend(extra);
        }

        // Also include any external dirs configured in config.yaml
        let config_path = self.base_dir().join("config.yaml");
        if let Some(config) = Self::parse_yaml(&config_path) {
            if let Some(external) = config
                .get("skills")
                .and_then(|s| s.get("external_dirs"))
                .and_then(|v| v.as_sequence())
            {
                for item in external {
                    if let Some(raw) = item.as_str() {
                        let path = if let Some(stripped) = raw.strip_prefix("~/") {
                            self.home.join(stripped)
                        } else {
                            PathBuf::from(raw)
                        };
                        if path.is_dir() && !dirs.contains(&path) {
                            dirs.push(path);
                        }
                    }
                }
            }
        }

        dirs
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        // "local" sub-category mirrors the global convention: user-managed skills
        // land in .hermes/skills/local/, keeping them separate from any Nous-shipped
        // category dirs that might appear in a future project-level skills feature.
        vec![".hermes/skills/local".into()]
    }

    fn mcp_config_path(&self) -> PathBuf {
        self.base_dir().join("config.yaml")
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::HermesYaml
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Some(config) = Self::parse_yaml(path) else {
            return vec![];
        };
        let Some(servers) = config
            .get("mcp_servers")
            .and_then(|v| v.as_mapping())
        else {
            return vec![];
        };

        servers
            .iter()
            .filter_map(|(key, val)| {
                let name = key.as_str()?.to_string();
                // HTTP MCP: {url: "http://..."} — store URL in command field
                // stdio MCP: {command: "...", args: [...], env: {...}}
                let command = if let Some(url) = val.get("url").and_then(|v| v.as_str()) {
                    url.to_string()
                } else {
                    val.get("command").and_then(|v| v.as_str())?.to_string()
                };

                let args: Vec<String> = val
                    .get("args")
                    .and_then(|v| v.as_sequence())
                    .map(|seq| {
                        seq.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let env: std::collections::HashMap<String, String> = val
                    .get("env")
                    .and_then(|v| v.as_mapping())
                    .map(|m| {
                        m.iter()
                            .filter_map(|(k, v)| {
                                Some((k.as_str()?.to_string(), v.as_str()?.to_string()))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let enabled = val
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                Some(McpServerEntry {
                    name,
                    command,
                    args,
                    env,
                    enabled,
                })
            })
            .collect()
    }

    fn hook_format(&self) -> HookFormat {
        HookFormat::HermesYaml
    }

    fn hook_config_path(&self) -> PathBuf {
        // Hermes hooks live at the root `hooks:` key of config.yaml.
        self.base_dir().join("config.yaml")
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        self.read_hooks_from(&self.hook_config_path())
    }

    fn read_hooks_from(&self, path: &Path) -> Vec<HookEntry> {
        let Some(config) = Self::parse_yaml(path) else {
            return vec![];
        };
        let Some(hooks) = config.get("hooks").and_then(|v| v.as_mapping()) else {
            return vec![];
        };
        let mut out = Vec::new();
        for (event_key, list) in hooks {
            let Some(event) = event_key.as_str() else {
                continue;
            };
            let Some(items) = list.as_sequence() else {
                continue;
            };
            for item in items {
                let Some(command) = item.get("command").and_then(|v| v.as_str()) else {
                    continue;
                };
                let matcher = item
                    .get("matcher")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                out.push(HookEntry {
                    event: event.to_string(),
                    matcher,
                    command: command.to_string(),
                });
            }
        }
        out
    }

    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_hermes(event)
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![]
    }

    fn list_skill_categories(&self) -> Vec<String> {
        self.list_categories()
    }

    /// Hermes organises skills into named category subdirectories. Resolve the
    /// install target as `~/.hermes/skills/{category}` (global) or
    /// `<project>/.hermes/skills/{category}` (project).
    fn skill_dir_for_category(
        &self,
        scope: &crate::models::ConfigScope,
        category: &str,
    ) -> Option<PathBuf> {
        Some(match scope {
            crate::models::ConfigScope::Global => {
                self.base_dir().join("skills").join(category)
            }
            crate::models::ConfigScope::Project { path, .. } => {
                Path::new(path).join(".hermes").join("skills").join(category)
            }
        })
    }

    // --- Config file discovery (Agents page) ---

    fn global_rules_files(&self) -> Vec<PathBuf> {
        // SOUL.md defines the agent's personality / system prompt baseline
        vec![self.base_dir().join("SOUL.md")]
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        let memories_dir = self.base_dir().join("memories");
        std::fs::read_dir(&memories_dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect()
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("config.yaml")]
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        // Hermes has no native project config convention. The marker is the
        // directory HarnessKit itself creates when installing project skills,
        // so existing HK-managed projects are recognized on re-scan.
        vec![ProjectMarker::Dir(".hermes/skills/local")]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;
    use std::fs;

    #[test]
    fn test_name() {
        let adapter = HermesAdapter::new();
        assert_eq!(adapter.name(), "hermes");
    }

    #[test]
    fn test_detect_without_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());
    }

    #[test]
    fn test_detect_with_dir() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".hermes")).unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.detect());
    }

    #[test]
    fn test_skill_dirs_local_first() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        let dirs = adapter.skill_dirs();
        assert!(!dirs.is_empty());
        assert!(
            dirs[0].ends_with(".hermes/skills/local"),
            "first skill dir should be local category, got {:?}",
            dirs[0]
        );
    }

    #[test]
    fn test_skill_dirs_includes_category_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let skills = tmp.path().join(".hermes").join("skills");
        fs::create_dir_all(skills.join("local")).unwrap();
        fs::create_dir_all(skills.join("devops")).unwrap();
        fs::create_dir_all(skills.join("apple")).unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        let dirs = adapter.skill_dirs();
        // local is first
        assert!(dirs[0].ends_with("local"));
        // other categories are included
        let names: Vec<&str> = dirs
            .iter()
            .filter_map(|p| p.file_name()?.to_str())
            .collect();
        assert!(names.contains(&"devops"));
        assert!(names.contains(&"apple"));
    }

    #[test]
    fn test_project_skill_dirs() {
        let adapter = HermesAdapter::new();
        assert_eq!(adapter.project_skill_dirs(), vec![".hermes/skills/local"]);
    }

    #[test]
    fn test_read_hooks_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.read_hooks().is_empty());
    }

    #[test]
    fn test_read_hooks_parses_config_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".hermes");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("config.yaml"),
            "hooks:\n  pre_tool_call:\n    - matcher: terminal\n      command: ~/.hermes/agent-hooks/block.sh\n      timeout: 5\n  on_session_start:\n    - command: ~/.hermes/agent-hooks/log.sh\n",
        )
        .unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        let hooks = adapter.read_hooks();
        assert_eq!(hooks.len(), 2);
        let pre = hooks.iter().find(|h| h.event == "pre_tool_call").unwrap();
        assert_eq!(pre.matcher.as_deref(), Some("terminal"));
        assert_eq!(pre.command, "~/.hermes/agent-hooks/block.sh");
        let sess = hooks.iter().find(|h| h.event == "on_session_start").unwrap();
        assert_eq!(sess.matcher, None);
    }

    #[test]
    fn test_read_mcp_servers_url_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let hermes_dir = tmp.path().join(".hermes");
        fs::create_dir_all(&hermes_dir).unwrap();
        fs::write(
            hermes_dir.join("config.yaml"),
            "mcp_servers:\n  proxy:\n    url: http://localhost:8080/mcp\n    enabled: false\n",
        )
        .unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "proxy");
        assert_eq!(servers[0].command, "http://localhost:8080/mcp");
        assert!(!servers[0].enabled);
    }

    #[test]
    fn test_read_mcp_servers_command_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let hermes_dir = tmp.path().join(".hermes");
        fs::create_dir_all(&hermes_dir).unwrap();
        fs::write(
            hermes_dir.join("config.yaml"),
            "mcp_servers:\n  fs:\n    command: /usr/local/bin/mcp-fs\n    args:\n      - --root\n      - /tmp\n    env:\n      DEBUG: \"1\"\n",
        )
        .unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "fs");
        assert_eq!(servers[0].command, "/usr/local/bin/mcp-fs");
        assert_eq!(servers[0].args, vec!["--root", "/tmp"]);
        assert_eq!(servers[0].env.get("DEBUG").map(|s| s.as_str()), Some("1"));
        assert!(servers[0].enabled);
    }

    #[test]
    fn test_read_mcp_servers_empty_when_no_config() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.read_mcp_servers().is_empty());
    }

    #[test]
    fn test_skill_dir_for_category_resolves_global_and_project() {
        use crate::models::ConfigScope;
        let tmp = tempfile::tempdir().unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());

        let global = adapter
            .skill_dir_for_category(&ConfigScope::Global, "devops")
            .expect("hermes resolves a global category dir");
        assert!(global.ends_with(".hermes/skills/devops"));

        let project = adapter
            .skill_dir_for_category(
                &ConfigScope::Project {
                    name: "demo".into(),
                    path: "/tmp/proj".into(),
                },
                "apple",
            )
            .expect("hermes resolves a project category dir");
        assert_eq!(
            project,
            std::path::Path::new("/tmp/proj/.hermes/skills/apple")
        );
    }
}
