// Config reference: https://hermes-agent.nousresearch.com/docs/user-guide/features/skills
// Skills: every `SKILL.md` at any depth under ~/.hermes/skills/ (plus
//         `skills.external_dirs`), found by `os.walk` in
//         agent/skill_utils.py `iter_skill_index_files`. Category dirs
//         (apple/, devops/, …) are organisation only; `local/` is where HK installs.
//         Off switch: the `skills.disabled` name list in config.yaml, written by
//         `hermes skills` (hermes_cli/skills_config.py `save_disabled_skills`).
// MCP:    ~/.hermes/config.yaml — "mcp_servers" YAML mapping
// Hooks:  ~/.hermes/config.yaml root `hooks:` key — list of {matcher?, command, timeout?}
//         per event (pre_tool_call/post_tool_call/on_session_start/...). YAML, McpFormat::HermesYaml-style.
// Plugins: ~/.hermes/plugins/<name>/plugin.yaml (flat) or .../<category>/<name>/plugin.yaml
//          (one nesting level). Enable-state in config.yaml `plugins.enabled` (disabled by default).

use super::{
    AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, McpTransport, PluginEntry,
    ProjectMarker, RemoteMcpSchema,
};
use std::path::{Path, PathBuf};

/// Never entered by Hermes' skill walk (agent/skill_utils.py `EXCLUDED_SKILL_DIRS`).
const EXCLUDED_SKILL_DIRS: &[&str] = &[
    ".git",
    ".github",
    ".hub",
    ".archive",
    ".curator_backups",
    ".locks",
    ".venv",
    "venv",
    "node_modules",
    "site-packages",
    "__pycache__",
    ".tox",
    ".nox",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
];
/// Not entered when the parent directory is itself a skill
/// (agent/skill_utils.py `SKILL_SUPPORT_DIRS`).
const SKILL_SUPPORT_DIRS: &[&str] = &["references", "templates", "assets", "scripts"];
/// The names a Hermes list-valued config entry holds, mirroring upstream
/// `parse_config_string_list` + `_normalize_string_set`: a sequence, a quoted
/// JSON/Python-literal list (what `hermes config set` stores), or a scalar
/// meaning one name; entries trimmed, empties dropped. Null/other → none.
pub(crate) fn name_list(value: &serde_yaml::Value) -> Vec<String> {
    let items: Vec<serde_yaml::Value> = match value {
        serde_yaml::Value::Sequence(seq) => seq.clone(),
        serde_yaml::Value::String(s) if s.trim_start().starts_with('[') => {
            serde_yaml::from_str::<serde_yaml::Value>(s)
                .ok()
                .and_then(|v| v.as_sequence().cloned())
                .unwrap_or_else(|| vec![value.clone()])
        }
        serde_yaml::Value::String(_) => vec![value.clone()],
        _ => vec![],
    };
    items
        .iter()
        .filter_map(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

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
            home: crate::home_dir(),
        }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self { home }
    }

    /// List all category subdirectory names under `~/.hermes/skills/`.
    /// Returns sorted names, excluding hidden dirs and dirs that are themselves
    /// a skill (a root-level `<name>/SKILL.md` has no category upstream).
    /// Used by the UI category picker.
    pub fn list_categories(&self) -> Vec<String> {
        let skills_root = self.base_dir().join("skills");
        let mut cats: Vec<String> = std::fs::read_dir(&skills_root)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if !p.is_dir() || crate::scanner::is_skill_dir(&p) {
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

    fn config(&self) -> Option<serde_yaml::Value> {
        Self::parse_yaml(&self.base_dir().join("config.yaml"))
    }

    /// The names in the `<section>.<key>` list of config.yaml (empty when
    /// absent) — `plugins.enabled`, `skills.disabled`.
    fn config_strings(&self, section: &str, key: &str) -> std::collections::HashSet<String> {
        self.config()
            .and_then(|cfg| cfg.get(section)?.get(key).map(name_list))
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// Build a `PluginEntry` from a `plugin.yaml` manifest path. The plugin name
    /// comes from the manifest's `name` key, falling back to the parent directory
    /// name. Returns `None` if the manifest can't be parsed or no name is derivable.
    fn plugin_entry_from_manifest(
        manifest: &Path,
        enabled: &std::collections::HashSet<String>,
    ) -> Option<PluginEntry> {
        let parsed = Self::parse_yaml(manifest)?;
        let dir = manifest.parent().map(PathBuf::from);
        let name = parsed
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| {
                dir.as_ref()
                    .and_then(|d| d.file_name())
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })?;
        Some(PluginEntry {
            enabled: enabled.contains(&name),
            name,
            source: "hermes".into(),
            path: dir,
            source_url: None,
            uri: None,
            installed_at: None,
            updated_at: None,
            base_layers: vec![],
            pack: None,
        })
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
        // `local/` first: it is the install target (`skill_dir_for(Global)`).
        // The whole root follows; `expand_skill_roots` walks it (and any
        // `skills.external_dirs`) the way upstream does, so `local/` is reached
        // twice and the scanner dedups.
        let mut dirs = vec![skills_root.join("local"), skills_root];
        if let Some(config) = self.config()
            && let Some(external) = config
                .get("skills")
                .and_then(|s| s.get("external_dirs"))
                .and_then(|v| v.as_sequence())
        {
            for raw in external.iter().filter_map(|v| v.as_str()) {
                let path = match raw.strip_prefix("~/") {
                    Some(stripped) => self.home.join(stripped),
                    None => PathBuf::from(raw),
                };
                if path.is_dir() && !dirs.contains(&path) {
                    dirs.push(path);
                }
            }
        }
        dirs
    }

    fn expand_skill_roots(&self, root: &Path) -> Vec<PathBuf> {
        // Hermes loads every SKILL.md at any depth (`os.walk`), skipping
        // EXCLUDED_SKILL_DIRS everywhere and SKILL_SUPPORT_DIRS under a skill.
        let skip = |child: &Path| {
            let name = child
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            EXCLUDED_SKILL_DIRS.contains(&name)
                || (SKILL_SUPPORT_DIRS.contains(&name)
                    && child
                        .parent()
                        .is_some_and(|parent| parent.join("SKILL.md").is_file()))
        };
        let mut roots = vec![root.to_path_buf()];
        super::collect_nested_skill_parents(
            root,
            1,
            super::MAX_SKILL_WALK_DEPTH,
            &skip,
            &mut roots,
        );
        roots
    }

    fn standalone_md_skills(&self) -> bool {
        // The walk matches the exact filename `SKILL.md`; a bare `notes.md`
        // never reaches the catalog or the prompt index (only `skill_view` has
        // a legacy exact-name fallback). Verified against hermes-agent c419cf7.
        false
    }

    fn natively_disabled_skills(&self) -> std::collections::HashSet<String> {
        self.config_strings("skills", "disabled")
    }

    fn supports_native_skill_toggle(&self) -> bool {
        true
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        // Upstream also loads trusted project skills from <cwd>/.hermes/skills
        // and <cwd>/.agents/skills (agent/skill_utils.py PROJECT_SKILLS_SUBDIRS,
        // gated on project trust); not modelled here yet.
        vec![]
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
        let Some(servers) = config.get("mcp_servers").and_then(|v| v.as_mapping()) else {
            return vec![];
        };

        servers
            .iter()
            .filter_map(|(key, val)| {
                let name = key.as_str()?.to_string();
                // Remote MCP: {url, headers?, transport: sse?} — Streamable
                // HTTP unless `transport: sse` is set.
                // stdio MCP: {command: "...", args: [...], env: {...}}
                let url = val.get("url").and_then(|v| v.as_str()).map(String::from);
                let (transport, command) = match &url {
                    Some(_) => {
                        let sse = val.get("transport").and_then(|v| v.as_str()) == Some("sse");
                        (
                            if sse {
                                McpTransport::Sse
                            } else {
                                McpTransport::Http
                            },
                            String::new(),
                        )
                    }
                    None => (
                        McpTransport::Stdio,
                        val.get("command").and_then(|v| v.as_str())?.to_string(),
                    ),
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

                let enabled = val.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

                Some(McpServerEntry {
                    name,
                    command,
                    args,
                    env: super::yaml_string_map(val, "env"),
                    transport,
                    url,
                    headers: super::yaml_string_map(val, "headers"),
                    enabled,
                })
            })
            .collect()
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::HermesUrl
    }

    fn supports_native_mcp_toggle(&self) -> bool {
        // Hermes MCP servers carry a native `enabled: bool` (default true);
        // disabling = set `enabled: false` in place (config retained), exactly
        // like the `hermes mcp` CLI. No remove, no DB snapshot.
        // Docs: https://hermes-agent.nousresearch.com/docs/reference/mcp-config-reference
        true
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
                    enabled: true,
                });
            }
        }
        out
    }

    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_hermes(event)
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("plugins")]
    }

    fn plugin_config_path(&self) -> PathBuf {
        self.base_dir().join("config.yaml")
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        let root = self.base_dir().join("plugins");
        let enabled = self.config_strings("plugins", "enabled");
        let mut out = Vec::new();
        // Single pass: discover plugin.yaml at depth 1 (flat) and depth 2
        // (category-nested), building each PluginEntry as it is found.
        let Ok(level1) = std::fs::read_dir(&root) else {
            return out;
        };
        for e1 in level1.flatten() {
            let p1 = e1.path();
            if !p1.is_dir() {
                continue;
            }
            // Flat plugin: <plugins>/<name>/plugin.yaml. Wins over any nested
            // manifest, so don't descend into this directory.
            let flat = p1.join("plugin.yaml");
            if flat.is_file() {
                if let Some(entry) = Self::plugin_entry_from_manifest(&flat, &enabled) {
                    out.push(entry);
                }
                continue;
            }
            // Category-nested plugin: <plugins>/<category>/<name>/plugin.yaml.
            let Ok(level2) = std::fs::read_dir(&p1) else {
                continue;
            };
            for e2 in level2.flatten() {
                let nested = e2.path().join("plugin.yaml");
                if nested.is_file() {
                    if let Some(entry) = Self::plugin_entry_from_manifest(&nested, &enabled) {
                        out.push(entry);
                    }
                }
            }
        }
        out
    }

    fn list_skill_categories(&self) -> Vec<String> {
        self.list_categories()
    }

    /// Hermes organises skills into named category subdirectories. Resolve the
    /// install target as `~/.hermes/skills/{category}` (global). Project
    /// scope resolves to `None` (project skills not modelled, see `project_skill_dirs`).
    fn skill_dir_for_category(
        &self,
        scope: &crate::models::ConfigScope,
        category: &str,
    ) -> Option<PathBuf> {
        match scope {
            crate::models::ConfigScope::Global => {
                Some(self.base_dir().join("skills").join(category))
            }
            // Project skills are not modelled (see `project_skill_dirs`).
            crate::models::ConfigScope::Project { .. } => None,
        }
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
        // Hermes has no project-level config HK models; it never marks a
        // project dir (trusted project skills: see `project_skill_dirs`).
        vec![]
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
    fn read_mcp_servers_parses_url_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config.yaml");
        fs::write(
            &config,
            "mcp_servers:\n  stripe:\n    url: https://mcp.stripe.com\n    headers:\n      Authorization: Bearer tok\n  events:\n    url: https://example.com/sse\n    transport: sse\n  local:\n    command: npx\n    args: [-y, srv]\n",
        )
        .unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers_from(&config);
        let by_name: std::collections::HashMap<_, _> =
            servers.iter().map(|s| (s.name.as_str(), s)).collect();
        let stripe = by_name["stripe"];
        assert_eq!(stripe.transport, McpTransport::Http);
        assert_eq!(stripe.url.as_deref(), Some("https://mcp.stripe.com"));
        assert_eq!(stripe.command, "", "url must not leak into command");
        assert_eq!(stripe.headers["Authorization"], "Bearer tok");
        assert_eq!(by_name["events"].transport, McpTransport::Sse);
        assert_eq!(by_name["local"].transport, McpTransport::Stdio);
    }

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
    fn test_project_skill_dirs_empty_global_only() {
        // Project skills are not modelled (see `project_skill_dirs`).
        let adapter = HermesAdapter::new();
        assert!(adapter.project_skill_dirs().is_empty());
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
        let sess = hooks
            .iter()
            .find(|h| h.event == "on_session_start")
            .unwrap();
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
        assert_eq!(servers[0].transport, McpTransport::Http);
        assert_eq!(servers[0].url.as_deref(), Some("http://localhost:8080/mcp"));
        assert_eq!(servers[0].command, "");
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
    fn test_read_plugins_flat_and_nested_with_enabled_state() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".hermes");
        fs::create_dir_all(dir.join("plugins/calculator")).unwrap();
        fs::create_dir_all(dir.join("plugins/devtools/git-helper")).unwrap();
        fs::write(
            dir.join("plugins/calculator/plugin.yaml"),
            "name: calculator\nversion: 1.0.0\n",
        )
        .unwrap();
        fs::write(
            dir.join("plugins/devtools/git-helper/plugin.yaml"),
            "name: git-helper\nversion: 0.1.0\n",
        )
        .unwrap();
        fs::write(
            dir.join("config.yaml"),
            "plugins:\n  enabled:\n    - calculator\n",
        )
        .unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 2);
        let calc = plugins.iter().find(|p| p.name == "calculator").unwrap();
        assert!(calc.enabled, "listed in plugins.enabled");
        let git = plugins.iter().find(|p| p.name == "git-helper").unwrap();
        assert!(!git.enabled, "not listed → disabled by default");
    }

    #[test]
    fn test_read_plugins_falls_back_to_dirname_when_no_name_key() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".hermes");
        fs::create_dir_all(dir.join("plugins/no-name-plugin")).unwrap();
        fs::write(
            dir.join("plugins/no-name-plugin/plugin.yaml"),
            "version: 1.0.0\n",
        )
        .unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(
            plugins[0].name, "no-name-plugin",
            "manifest without a name key falls back to the directory name"
        );
    }

    #[test]
    fn test_read_plugins_flat_wins_does_not_descend() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".hermes");
        // A flat plugin that ALSO contains a nested plugin.yaml underneath it.
        fs::create_dir_all(dir.join("plugins/calculator/sub")).unwrap();
        fs::write(
            dir.join("plugins/calculator/plugin.yaml"),
            "name: calculator\nversion: 1.0.0\n",
        )
        .unwrap();
        fs::write(
            dir.join("plugins/calculator/sub/plugin.yaml"),
            "name: sub\nversion: 2.0.0\n",
        )
        .unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        // Flat plugin wins: calculator appears once, the nested `sub` is not surfaced.
        assert_eq!(plugins.len(), 1);
        assert_eq!(
            plugins.iter().filter(|p| p.name == "calculator").count(),
            1,
            "flat plugin counted exactly once"
        );
        assert!(
            !plugins.iter().any(|p| p.name == "sub"),
            "do not descend into a flat plugin directory"
        );
    }

    #[test]
    fn test_skill_dir_for_category_global_only() {
        use crate::models::ConfigScope;
        let tmp = tempfile::tempdir().unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());

        let global = adapter
            .skill_dir_for_category(&ConfigScope::Global, "devops")
            .expect("hermes resolves a global category dir");
        assert!(global.ends_with(".hermes/skills/devops"));

        // Project scope never resolves a category dir (project skills not
        // modelled).
        assert!(
            adapter
                .skill_dir_for_category(
                    &ConfigScope::Project {
                        name: "demo".into(),
                        path: "/tmp/proj".into(),
                    },
                    "apple",
                )
                .is_none(),
            "hermes is global-only: no project category dir"
        );
    }

    /// `<home>/.hermes/skills/<rel>/SKILL.md` without frontmatter, so the
    /// skill is named after its directory exactly as upstream does.
    fn write_skill(home: &Path, rel: &str) {
        let dir = home.join(".hermes/skills").join(rel);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "# skill\n").unwrap();
    }

    fn scanned(adapter: &HermesAdapter) -> Vec<(String, bool)> {
        let mut out: Vec<(String, bool)> = crate::scanner::scan_skills_for(adapter)
            .into_iter()
            .map(|e| (e.name, e.enabled))
            .collect();
        out.sort();
        out
    }

    #[test]
    fn skill_dirs_install_target_first_then_whole_root() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = HermesAdapter::with_home(tmp.path().to_path_buf());
        let dirs = adapter.skill_dirs();
        assert!(
            dirs[0].ends_with(".hermes/skills/local"),
            "install target first: {dirs:?}"
        );
        assert!(
            dirs[1].ends_with(".hermes/skills"),
            "then the whole root: {dirs:?}"
        );
    }

    #[test]
    fn skills_are_discovered_at_any_depth_like_upstream() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        write_skill(home, "local/alpha"); // category/skill
        write_skill(home, "beta"); // skill directly under the root
        write_skill(home, "local/team/gamma"); // two levels under a category
        write_skill(home, "beta/sub"); // nested inside a skill: still a skill upstream
        write_skill(home, "beta/scripts/helper"); // support dir of a skill: pruned
        write_skill(home, ".git/hook"); // EXCLUDED_SKILL_DIRS
        write_skill(home, "node_modules/pkg"); // EXCLUDED_SKILL_DIRS
        let adapter = HermesAdapter::with_home(home.to_path_buf());
        let names: Vec<String> = scanned(&adapter).into_iter().map(|(n, _)| n).collect();
        assert_eq!(
            names,
            ["alpha", "beta", "gamma", "sub"],
            "each exactly once"
        );
    }

    #[test]
    fn bare_markdown_is_not_a_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        write_skill(home, "local/alpha");
        fs::write(
            home.join(".hermes/skills/local/notes.md"),
            "---\nname: notes\n---\n",
        )
        .unwrap();
        let adapter = HermesAdapter::with_home(home.to_path_buf());
        assert_eq!(scanned(&adapter), [("alpha".to_string(), true)]);
    }

    #[test]
    fn skills_listed_in_config_disabled_scan_as_disabled() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        write_skill(home, "local/alpha");
        write_skill(home, "local/beta");
        fs::write(
            home.join(".hermes/config.yaml"),
            "skills:\n  disabled:\n    - ' alpha '\n    - ghost\n",
        )
        .unwrap();
        let adapter = HermesAdapter::with_home(home.to_path_buf());
        assert_eq!(
            scanned(&adapter),
            [("alpha".to_string(), false), ("beta".to_string(), true)]
        );
        // A scalar is one name (upstream parse_config_string_list).
        fs::write(
            home.join(".hermes/config.yaml"),
            "skills:\n  disabled: beta\n",
        )
        .unwrap();
        assert_eq!(
            scanned(&adapter),
            [("alpha".to_string(), true), ("beta".to_string(), false)]
        );
    }

    #[test]
    fn list_categories_skips_root_level_skill_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        write_skill(home, "devops/deploy"); // category
        write_skill(home, "beta"); // a skill, not a category
        fs::create_dir_all(home.join(".hermes/skills/.hub")).unwrap();
        let adapter = HermesAdapter::with_home(home.to_path_buf());
        assert_eq!(adapter.list_categories(), ["devops"]);
    }
}
