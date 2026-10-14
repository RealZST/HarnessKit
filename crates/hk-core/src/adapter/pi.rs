// Pi (earendil-works/pi coding agent) — verified against upstream docs
// (github.com/earendil-works/pi, packages/coding-agent/docs):
// - Agent dir: `$PI_CODING_AGENT_DIR` or `~/.pi/agent`
//   (configuration.md). Same root serves the pi CLI and pi-web
//   (github.com/agegr/pi-web), the local browser UI — pi-web reuses the
//   pi agent's own configuration, so there is no separate config root.
// - Skills: `~/.pi/agent/skills/` (user) + `.pi/skills/` (project), with
//   recursive SKILL.md discovery. Pi also reads the universal Agent Skills
//   locations `~/.agents/skills` / `.agents/skills` (skills.md), so those
//   are declared here too.
// - MCP: JSON `{"mcpServers": {name: {...}}}` — stdio `{command, args, env,
//   cwd}` or streamable HTTP `{url, headers}` with a native per-entry
//   `enabled` flag (mcp.md). User `~/.pi/agent/mcp.json`, project
//   `.pi/mcp.json`.
// - Extensions: TypeScript/JavaScript modules — `~/.pi/agent/extensions/` +
//   `.pi/extensions/` (extensions.md). Same shape as omp.
// - Context/rules: `AGENTS.override.md` / `AGENTS.md` / `AGENTS.MD` /
//   `CLAUDE.md` / `CLAUDE.MD` from the agent dir and each ancestor of the
//   working directory (configuration.md "Context files").
// - No shell-command hooks → HookFormat::None (extensions are TS/JS modules).

use super::{
    AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, PluginEntry, ProjectMarker,
    RemoteMcpSchema,
};
use std::path::{Path, PathBuf};

/// Context-file fallback list, in Pi's documented precedence order.
const CONTEXT_FILENAMES: &[&str] = &[
    "AGENTS.override.md",
    "AGENTS.md",
    "AGENTS.MD",
    "CLAUDE.md",
    "CLAUDE.MD",
];

pub struct PiAdapter {
    home: PathBuf,
    agent_dir: PathBuf,
}

impl Default for PiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PiAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let agent_dir = std::env::var_os("PI_CODING_AGENT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".pi").join("agent"));
        Self { home, agent_dir }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self {
            agent_dir: home.join(".pi").join("agent"),
            home,
        }
    }

    fn parse_json(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// `npm:<name>[@version]` declarations from `settings.json` → package
    /// directory under `~/.pi/agent/npm/node_modules`. Only npm sources are
    /// returned (git/local packages are not modeled as plugins yet).
    fn npm_package_dirs(&self) -> Vec<(String, String, PathBuf)> {
        let Some(settings) = Self::parse_json(&self.agent_dir.join("settings.json")) else {
            return vec![];
        };
        let Some(packages) = settings.get("packages").and_then(|v| v.as_array()) else {
            return vec![];
        };
        let npm_root = self.agent_dir.join("npm").join("node_modules");
        let mut out = vec![];
        for pkg in packages {
            let source = match pkg {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Object(o) => o
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                _ => continue,
            };
            let Some(spec) = source.strip_prefix("npm:") else {
                continue;
            };
            // Strip a trailing `@version` only when it is not the scope of a
            // scoped package (`@scope/name`): the version separator never
            // contains a `/`.
            let pkg_name = match spec.rsplit_once('@') {
                Some((name, ver)) if !ver.contains('/') && !name.is_empty() => name.to_string(),
                _ => spec.to_string(),
            };
            let dir = npm_root.join(&pkg_name);
            if dir.is_dir() {
                out.push((source, pkg_name, dir));
            }
        }
        out
    }

    /// Read an installed npm package's `repository.url` and derive its
    /// lowercased `owner/repo` pack. `None` for packages without a repository
    /// (e.g. unpublished local packages).
    fn npm_package_repo(pkg_dir: &Path) -> Option<(String, String)> {
        let content = std::fs::read_to_string(pkg_dir.join("package.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        crate::scanner::repository_pack_from_package_json(&json)
    }

    /// Strip a `.disabled` suffix so a toggled-off `hello.ts.disabled` keeps
    /// the stable name of its enabled sibling `hello.ts` (same as omp).
    fn plugin_name(path: &Path) -> String {
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

    /// Pi loads direct `.ts`/`.js` files and `index.ts`/`index.js` entry
    /// points (extensions.md). A `.disabled` suffix is HarnessKit's own
    /// in-place rename, which hides the file from the loader.
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
}

impl AgentAdapter for PiAdapter {
    fn name(&self) -> &str {
        "pi"
    }

    fn base_dir(&self) -> PathBuf {
        self.home.join(".pi")
    }

    fn detect(&self) -> bool {
        // Pi creates the agent dir on first run (`~/.pi/agent`), including
        // `~/.pi` for pi-web session state. Presence of the agent dir is the
        // reliable install signal.
        self.agent_dir.exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        // Native Pi user skills: ~/.pi/agent/skills, plus the universal
        // Agent Skills location ~/.agents/skills (skills.md: "Pi also
        // supports the Agent Skills locations ~/.agents/skills/ and
        // .agents/skills/"). npm packages can ship skills under their own
        // `skills/` directory (packages.md: conventional resource dir).
        let mut dirs = vec![
            self.agent_dir.join("skills"),
            self.home.join(".agents").join("skills"),
        ];
        for (_, _, pkg_dir) in self.npm_package_dirs() {
            let skills = pkg_dir.join("skills");
            if skills.is_dir() {
                dirs.push(skills);
            }
        }
        dirs
    }

    fn mcp_config_path(&self) -> PathBuf {
        // ~/.pi/agent/mcp.json is the canonical user-level file.
        self.agent_dir.join("mcp.json")
    }

    fn hook_config_path(&self) -> PathBuf {
        // Pi has no shell-command hooks file. Reuse the MCP path so the
        // default plugin_config_path() lands on a real file (mirrors omp).
        self.mcp_config_path()
    }

    fn plugin_config_path(&self) -> PathBuf {
        // npm package plugins are enabled/disabled by editing the `packages`
        // array in settings.json (`pi install` / `pi remove`).
        self.agent_dir.join("settings.json")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        // TypeScript/JavaScript extension modules: ~/.pi/agent/extensions/*.ts
        vec![self.agent_dir.join("extensions")]
    }

    fn hook_format(&self) -> HookFormat {
        HookFormat::None
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::McpServers
    }

    fn supports_native_mcp_toggle(&self) -> bool {
        // Toggle flips the entry's own `enabled` flag in place (see
        // deployer::set_pi_mcp_enabled), keeping `description`, `timeout`,
        // `exposure`, `cwd`, `oauth`, `env`, `headers`, and all other keys
        // untouched — exactly how `pi mcp` / `/mcp` behaves.
        true
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        // HTTP entries are `{url, headers}` with the transport auto-detected;
        // the legacy SSE transport is rejected upstream (docs/mcp.md), so
        // remote entries are Streamable HTTP only.
        RemoteMcpSchema::PlainUrlHttpOnly
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
                // `command` selects stdio, `url` selects streamable HTTP;
                // an optional `type` (stdio/http/streamable-http) also counts.
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
                    // Native per-server flag: absent means enabled.
                    enabled: val
                        .get("enabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true),
                }
            })
            .collect()
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        // Pi hooks are TS/JS event-handler extensions, not shell commands.
        vec![]
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        let mut entries = Vec::new();
        // npm packages are Pi's primary plugin distribution (settings.json
        // `packages` → ~/.pi/agent/npm/node_modules). Each installed package
        // is one plugin; the `uri` carries the original `npm:…` declaration
        // so the toggle can remove/re-add it in settings.json.
        for (source, pkg_name, pkg_dir) in self.npm_package_dirs() {
            let repo = Self::npm_package_repo(&pkg_dir);
            entries.push(PluginEntry {
                name: pkg_name,
                source: "npm".into(),
                enabled: true,
                path: Some(pkg_dir),
                source_url: repo.as_ref().map(|(url, _)| url.clone()),
                uri: Some(source),
                installed_at: None,
                updated_at: None,
                base_layers: vec![],
                pack: repo.map(|(_, pack)| pack),
            });
        }
        // Local single-file / directory TypeScript extensions.
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
                        base_layers: vec![],
                        pack: None,
                    });
                } else if path.is_dir() {
                    let Some(dir_name) =
                        path.file_name().map(|n| n.to_string_lossy().to_string())
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
                            base_layers: vec![],
                            pack: None,
                        });
                    }
                }
            }
        }
        entries
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        // Context files live directly in the agent dir (configuration.md).
        CONTEXT_FILENAMES
            .iter()
            .map(|name| self.agent_dir.join(name))
            .collect()
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        // settings.json is the primary user-level settings file; models.json
        // holds compatible endpoints; mcp.json is listed so the servers file
        // is browsable from the agent page (matches the omp/kiro adapters).
        vec![
            self.agent_dir.join("settings.json"),
            self.agent_dir.join("models.json"),
            self.mcp_config_path(),
        ]
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        // Dir(".pi") alone suffices: every project-level Pi file lives inside
        // `.pi/`.
        vec![ProjectMarker::Dir(".pi")]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        // Context files are discovered from the working directory and its
        // ancestors (configuration.md "Context files"), not from `.pi/`.
        CONTEXT_FILENAMES.iter().map(|s| s.to_string()).collect()
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".pi/settings.json".into()]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        // Pi-native project skills plus the universal Agent Skills location
        // (skills.md).
        vec![".pi/skills".into(), ".agents/skills".into()]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        // Pi reads `.pi/mcp.json`; project config is loaded after project
        // trust is granted.
        Some(".pi/mcp.json".into())
    }

    fn project_plugin_dirs(&self) -> Vec<String> {
        // Project-level extension modules: <repo>/.pi/extensions/*.ts
        vec![".pi/extensions".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    #[test]
    fn detect_requires_agent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = PiAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());
        std::fs::create_dir_all(tmp.path().join(".pi/agent")).unwrap();
        assert!(adapter.detect());
    }

    #[test]
    fn read_mcp_servers_handles_stdio_and_remote() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = PiAdapter::with_home(tmp.path().to_path_buf());
        let cfg = tmp.path().join(".pi/agent/mcp.json");
        std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        std::fs::write(
            &cfg,
            r#"{
              "mcpServers": {
                "filesystem": {
                  "command": "npx",
                  "args": ["-y", "@modelcontextprotocol/server-filesystem"]
                },
                "docs": {
                  "url": "https://example.com/mcp",
                  "headers": { "Authorization": "Bearer t" }
                },
                "off": { "command": "echo", "enabled": false }
              }
            }"#,
        )
        .unwrap();

        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 3);
        let by_name: std::collections::HashMap<&str, &McpServerEntry> =
            servers.iter().map(|s| (s.name.as_str(), s)).collect();

        assert_eq!(by_name["filesystem"].command, "npx");
        assert!(by_name["filesystem"].enabled);

        let docs = by_name["docs"];
        assert_eq!(docs.transport, super::super::McpTransport::Http);
        assert_eq!(docs.url.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(docs.headers["Authorization"], "Bearer t");

        assert!(!by_name["off"].enabled);
    }

    #[test]
    fn read_plugins_lists_ts_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = PiAdapter::with_home(tmp.path().to_path_buf());
        let ext_dir = tmp.path().join(".pi/agent/extensions");
        std::fs::create_dir_all(&ext_dir).unwrap();
        std::fs::write(ext_dir.join("hello.ts"), "// ext\n").unwrap();
        std::fs::write(ext_dir.join("off.ts.disabled"), "// ext\n").unwrap();
        std::fs::write(ext_dir.join("README.md"), "# readme\n").unwrap();

        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 2);
        let by_name: std::collections::HashMap<&str, &PluginEntry> =
            plugins.iter().map(|p| (p.name.as_str(), p)).collect();
        assert!(by_name["hello"].enabled);
        assert!(!by_name["off"].enabled);
    }

    #[test]
    fn read_plugins_lists_npm_packages() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = PiAdapter::with_home(tmp.path().to_path_buf());
        let agent = tmp.path().join(".pi/agent");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::write(
            agent.join("settings.json"),
            r#"{"packages": ["npm:@scope/rpiv-todo", "npm:pi-lens", {"source":"npm:ponytail","skills":[]}]}"#,
        )
        .unwrap();
        let npm = agent.join("npm").join("node_modules");
        std::fs::create_dir_all(npm.join("@scope/rpiv-todo")).unwrap();
        std::fs::create_dir_all(npm.join("pi-lens")).unwrap();
        std::fs::create_dir_all(npm.join("ponytail")).unwrap();
        // Each declared package carries its upstream repo; a package without
        // a repository reports no pack/source_url.
        std::fs::write(
            npm.join("@scope/rpiv-todo/package.json"),
            r#"{"repository":{"url":"git+https://github.com/juicesharp/rpiv-mono.git"}}"#,
        )
        .unwrap();
        std::fs::write(
            npm.join("pi-lens/package.json"),
            r#"{"repository":{"url":"git+https://github.com/apmantza/pi-lens.git"}}"#,
        )
        .unwrap();
        std::fs::write(
            npm.join("ponytail/package.json"),
            r#"{"repository":{"url":"git+https://github.com/DietrichGebert/ponytail.git"}}"#,
        )
        .unwrap();
        // A transitive dependency that is NOT declared in `packages` must
        // not be surfaced as a plugin.
        std::fs::create_dir_all(npm.join("express")).unwrap();

        let plugins = adapter.read_plugins();
        let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["@scope/rpiv-todo", "pi-lens", "ponytail"]);
        assert!(plugins.iter().all(|p| p.source == "npm"));
        assert!(plugins.iter().all(|p| p.uri.is_some()));
        let by_name: std::collections::HashMap<&str, &PluginEntry> =
            plugins.iter().map(|p| (p.name.as_str(), p)).collect();
        assert_eq!(
            by_name["pi-lens"].pack.as_deref(),
            Some("apmantza/pi-lens")
        );
        assert_eq!(
            by_name["ponytail"].pack.as_deref(),
            Some("dietrichgebert/ponytail"),
            "GitHub owner/repo case must be normalized to lowercase"
        );
        assert_eq!(
            by_name["@scope/rpiv-todo"].pack.as_deref(),
            Some("juicesharp/rpiv-mono")
        );
        assert!(by_name["@scope/rpiv-todo"].source_url.is_some());
    }

    #[test]
    fn skill_dirs_include_npm_package_skills() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = PiAdapter::with_home(tmp.path().to_path_buf());
        let agent = tmp.path().join(".pi/agent");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::write(
            agent.join("settings.json"),
            r#"{"packages": ["npm:with-skills", "npm:no-skills"]}"#,
        )
        .unwrap();
        std::fs::create_dir_all(agent.join("npm/node_modules/with-skills/skills")).unwrap();
        std::fs::create_dir_all(agent.join("npm/node_modules/no-skills")).unwrap();

        let dirs = adapter.skill_dirs();
        assert_eq!(dirs.len(), 3);
        assert_eq!(dirs[0], agent.join("skills"));
        assert_eq!(dirs[1], tmp.path().join(".agents/skills"));
        assert_eq!(dirs[2], agent.join("npm/node_modules/with-skills/skills"));
    }

    #[test]
    fn paths_match_pi_native_conventions() {
        let adapter = PiAdapter::with_home(PathBuf::from("/h"));
        assert_eq!(adapter.base_dir(), PathBuf::from("/h/.pi"));
        assert_eq!(
            adapter.skill_dirs(),
            vec![
                PathBuf::from("/h/.pi/agent/skills"),
                PathBuf::from("/h/.agents/skills"),
            ]
        );
        assert_eq!(
            adapter.mcp_config_path(),
            PathBuf::from("/h/.pi/agent/mcp.json")
        );
        assert_eq!(
            adapter.plugin_dirs(),
            vec![PathBuf::from("/h/.pi/agent/extensions")]
        );
        assert_eq!(
            adapter.project_skill_dirs(),
            vec![".pi/skills".to_string(), ".agents/skills".to_string()]
        );
        assert_eq!(
            adapter.project_mcp_config_relpath(),
            Some(".pi/mcp.json".into())
        );
        assert_eq!(adapter.hook_format(), HookFormat::None);
        assert_eq!(adapter.mcp_format(), McpFormat::McpServers);
    }
}
