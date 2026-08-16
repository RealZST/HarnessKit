// DeepSeek Harness (dsh) config references — verified against
// github.com/deepseek-ai/deepseek-harness @ v0.1.0-rc (2026-08-14):
// - Home resolution: packages/util/home-paths (`$DSH_HOME` else `~/.dsh`;
//   the harness keeps all user data under one root).
// - Skills:  docs/subsystems/skills.md — roots (rank order) `<project>/.dsh/skills`,
//   `<project>/.agents/skills`, `<dshHome>/skills` (skips `.system` child),
//   `<agentsHome>/skills` where agentsHome = `$DSH_AGENTS_HOME` else `~/.agents`
//   (packages/skill/skill-filesystem/src/index.ts). Bundle `<name>/SKILL.md` or
//   flat `<name>.md`, one level deep — matches scan_skill_dir exactly, and the
//   `.system` dir is naturally invisible to a one-level scan (its skills nest
//   one level deeper).
// - MCP: packages/mcp/mcp-client — servers are `@deepseek-ai/dsh-mcp-client`
//   plugin rows in cordis patch files. dsh runs one profile at a time
//   (profile patch, then home patch); HK reads the HOME layer only —
//   `<dshHome>/cordis.patch.yml`, the one user layer every profile applies.
//   No project-level MCP config exists.
// - Hooks: packages/hooks — dsh has no own hook format; bridge plugins replay
//   Claude Code / Codex hooks.json. HookFormat::None.
// - Rules: packages/context/agent-instructions — `$DSH_HOME/AGENTS.md` global,
//   project chain reads AGENTS.md / CLAUDE.md + AGENTS.local.md / CLAUDE.local.md.

use super::{
    AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, McpTransport, ProjectMarker,
};
use std::path::{Path, PathBuf};

pub struct DshAdapter {
    /// `$DSH_HOME` else `~/.dsh`.
    dsh_home: PathBuf,
    /// `$DSH_AGENTS_HOME` else `~/.agents` (cross-vendor shared skills root).
    agents_home: PathBuf,
}

impl Default for DshAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve `(dsh_home, agents_home)` from the `DSH_HOME` / `DSH_AGENTS_HOME`
/// overrides, falling back to `<home>/.dsh` / `<home>/.agents`. Pure so it can
/// be tested with explicit inputs (mutating process env in tests is racy
/// under parallel execution).
fn resolve_homes(
    dsh_home: Option<std::ffi::OsString>,
    agents_home: Option<std::ffi::OsString>,
    home: &Path,
) -> (PathBuf, PathBuf) {
    (
        dsh_home
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".dsh")),
        agents_home
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".agents")),
    )
}

impl DshAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let (dsh_home, agents_home) = resolve_homes(
            std::env::var_os("DSH_HOME"),
            std::env::var_os("DSH_AGENTS_HOME"),
            &home,
        );
        Self {
            dsh_home,
            agents_home,
        }
    }

    #[cfg(test)]
    pub fn with_home(home: PathBuf) -> Self {
        Self {
            dsh_home: home.join(".dsh"),
            agents_home: home.join(".agents"),
        }
    }

    /// Existing per-profile patch files (settings listing only — MCP reading
    /// is home-layer-only by design; see module header).
    fn profile_patch_files(&self) -> Vec<PathBuf> {
        let profiles = self.dsh_home.join("profiles");
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&profiles)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        dirs.into_iter()
            .map(|d| d.join("cordis.patch.yml"))
            .filter(|p| p.is_file())
            .collect()
    }
}

/// dsh's `@deepseek-ai/dsh-mcp-client` plugin name — the marker for MCP rows.
const MCP_CLIENT_PLUGIN: &str = "@deepseek-ai/dsh-mcp-client";

/// One entry parsed from a patch file. `from_insert` distinguishes row
/// DEFINITIONS (inside `insert:`) from id-targeted overrides — upstream, an
/// override can never create a row.
struct CordisRow {
    id: Option<String>,
    name: Option<String>,
    /// Literal booleans, plus `null` ≡ `false` (static upstream rule,
    /// vendor/loader/src/config/entry.ts:104-107). Only absent keys and
    /// `!!js` expressions (which arrive as tag-stripped strings) read as
    /// None — upstream evaluates js `disabled` at runtime; HK can't, so it
    /// shows the base state.
    disabled: Option<bool>,
    config: serde_yaml::Value,
    from_insert: bool,
}

/// Parse one patch file into ordered entries. `{id, insert}` (group append)
/// is out of scope and skipped. A parse failure returns empty WITH a stderr
/// diagnostic — silence here would read as "dsh has no MCP".
fn parse_patch_rows(text: &str, origin: &Path) -> Vec<CordisRow> {
    let doc: serde_yaml::Value = match serde_yaml::from_str(text) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("[hk] warning: cannot parse {}: {err}", origin.display());
            return vec![];
        }
    };
    let Some(items) = doc.as_sequence() else {
        eprintln!(
            "[hk] warning: {} is not a YAML list (cordis patch files are top-level arrays)",
            origin.display()
        );
        return vec![];
    };
    let mut rows = Vec::new();
    for item in items {
        let Some(map) = item.as_mapping() else { continue };
        let has_id = map.get("id").is_some();
        let insert = map.get("insert").and_then(|v| v.as_sequence());
        match (has_id, insert) {
            (false, Some(inserted)) => {
                for row in inserted {
                    let Some(rm) = row.as_mapping() else { continue };
                    rows.push(CordisRow {
                        id: yaml_str(rm, "id"),
                        name: yaml_str(rm, "name"),
                        disabled: yaml_disabled(rm.get("disabled")),
                        config: rm.get("config").cloned().unwrap_or(serde_yaml::Value::Null),
                        from_insert: true,
                    });
                }
            }
            (true, None) => rows.push(CordisRow {
                id: yaml_str(map, "id"),
                name: yaml_str(map, "name"),
                disabled: yaml_disabled(map.get("disabled")),
                config: map.get("config").cloned().unwrap_or(serde_yaml::Value::Null),
                from_insert: false,
            }),
            // {id, insert} = group append; bare junk = neither. Both skipped.
            _ => {}
        }
    }
    rows
}

fn yaml_str(map: &serde_yaml::Mapping, key: &str) -> Option<String> {
    map.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn yaml_disabled(v: Option<&serde_yaml::Value>) -> Option<bool> {
    match v {
        Some(serde_yaml::Value::Null) => Some(false), // upstream: null ≡ false
        Some(serde_yaml::Value::Bool(b)) => Some(*b),
        _ => None, // absent, or !!js expression (arrives as String — HK can't evaluate)
    }
}

fn yaml_config_str(config: &serde_yaml::Value, key: &str) -> Option<String> {
    config.get(key).and_then(|v| v.as_str()).map(String::from)
}

/// Folded final state of one MCP row within one patch file (single ordered
/// apply, later entries win — mirrors upstream applyEntryPatches).
struct McpRowState {
    id: Option<String>,
    disabled: bool,
    config: serde_yaml::Value,
}

impl DshAdapter {
    fn fold_mcp_rows_in_text(text: &str, origin: &Path) -> Vec<McpRowState> {
        let mut order: Vec<String> = Vec::new();
        let mut by_id: std::collections::HashMap<String, McpRowState> =
            std::collections::HashMap::new();
        let mut anon: Vec<McpRowState> = Vec::new();

        for row in parse_patch_rows(text, origin) {
            let CordisRow { id, name, disabled, config, from_insert } = row;
            let is_mcp_def = from_insert && name.as_deref() == Some(MCP_CLIENT_PLUGIN);
            match id {
                Some(id) if is_mcp_def => {
                    order.push(id.clone());
                    by_id.insert(
                        id.clone(),
                        McpRowState { id: Some(id), disabled: disabled.unwrap_or(false), config },
                    );
                }
                Some(id) if !from_insert => {
                    // Override: only mutates an existing row (upstream:
                    // unknown id is warn+skip, never a definition).
                    if let Some(existing) = by_id.get_mut(&id) {
                        if let Some(d) = disabled {
                            existing.disabled = d;
                        }
                        if !config.is_null() {
                            existing.config = config;
                        }
                    }
                }
                // From-insert definition of some other plugin — never an
                // override; skip (even on a malformed id collision).
                Some(_) => {}
                None if is_mcp_def => {
                    anon.push(McpRowState { id: None, disabled: disabled.unwrap_or(false), config })
                }
                None => {}
            }
        }
        let mut out: Vec<McpRowState> =
            order.into_iter().filter_map(|id| by_id.remove(&id)).collect();
        out.extend(anon);
        out
    }

    fn mcp_entries_in_text(text: &str, origin: &Path) -> Vec<McpServerEntry> {
        Self::fold_mcp_rows_in_text(text, origin)
            .into_iter()
            .filter_map(|row| {
                let config = &row.config;
                let server_name = yaml_config_str(config, "serverName")?;
                // Remote MCP: {url, headers?} — stdio MCP: {command, args, env}.
                // `url` decides remote-vs-stdio FIRST (as in hermes.rs): dsh
                // ships only stdio and streamable-http, so a url-bearing row is
                // Streamable HTTP even when `transport` is omitted or carries
                // some other value. Deciding on the transport string instead
                // would emit a contradictory stdio entry with an empty command.
                let url = yaml_config_str(config, "url");
                let (transport, command) = match &url {
                    Some(_) => (McpTransport::Http, String::new()),
                    // Command may be absent on a malformed row; keep the entry
                    // visible (empty command) rather than hiding it.
                    None => (
                        McpTransport::Stdio,
                        yaml_config_str(config, "command").unwrap_or_default(),
                    ),
                };
                Some(McpServerEntry {
                    name: server_name,
                    command,
                    args: config
                        .get("args")
                        .and_then(|v| v.as_sequence())
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    env: super::yaml_string_map(config, "env"),
                    transport,
                    url,
                    headers: super::yaml_string_map(config, "headers"),
                    enabled: !row.disabled,
                })
            })
            .collect()
    }

    /// Row id for a serverName (unique across live instances upstream).
    /// Text-based so the deployer can evaluate the file it already read.
    pub fn mcp_row_id_in_text(text: &str, server_name: &str) -> Option<String> {
        Self::fold_mcp_rows_in_text(text, Path::new("cordis.patch.yml"))
            .into_iter()
            .find(|r| yaml_config_str(&r.config, "serverName").as_deref() == Some(server_name))
            .and_then(|r| r.id)
    }

    /// serverName → enabled for the given home-layer text (deployer uses this
    /// to compute base state with HK's managed block stripped).
    pub fn mcp_enabled_in_text(text: &str) -> std::collections::HashMap<String, bool> {
        Self::fold_mcp_rows_in_text(text, Path::new("cordis.patch.yml"))
            .into_iter()
            .filter_map(|r| Some((yaml_config_str(&r.config, "serverName")?, !r.disabled)))
            .collect()
    }
}

impl AgentAdapter for DshAdapter {
    fn name(&self) -> &str {
        "dsh"
    }

    fn base_dir(&self) -> PathBuf {
        self.dsh_home.clone()
    }

    fn detect(&self) -> bool {
        self.dsh_home.exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.dsh_home.join("skills"),
            self.agents_home.join("skills"),
        ]
    }

    /// Home-level user patch — the highest always-applied user layer and the
    /// canonical write target for HK's managed toggle block. NEVER point this
    /// at `<profileDir>/cordis.yml`: dsh overwrites that file on every boot.
    fn mcp_config_path(&self) -> PathBuf {
        self.dsh_home.join("cordis.patch.yml")
    }

    fn hook_config_path(&self) -> PathBuf {
        // dsh has no own hook config; return the settings doc so the default
        // plugin_config_path() has a sane anchor. Never read for hooks
        // (hook_format is None).
        self.dsh_home.join("settings.yaml")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![]
    }

    fn hook_format(&self) -> HookFormat {
        HookFormat::None
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::DshCordis
    }

    fn supports_native_mcp_toggle(&self) -> bool {
        // Toggle appends id-targeted patch rows via a managed block (deployer::set_dsh_mcp_enabled); never rewrites user YAML.
        true
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Ok(text) = std::fs::read_to_string(path) else {
            return vec![];
        };
        Self::mcp_entries_in_text(&text, path)
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        vec![]
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.dsh_home.join("AGENTS.md")]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files = vec![
            self.dsh_home.join("settings.yaml"),
            self.dsh_home.join("cordis.patch.yml"),
        ];
        files.extend(self.profile_patch_files());
        files
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            "AGENTS.md".into(),
            "CLAUDE.md".into(),
            "AGENTS.local.md".into(),
            "CLAUDE.local.md".into(),
        ]
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        vec![ProjectMarker::Dir(".dsh")]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        vec![".dsh/skills".into()]
    }

    fn project_skill_read_dirs(&self) -> Vec<String> {
        vec![".agents/skills".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    #[test]
    fn resolve_homes_env_overrides_and_fallbacks() {
        let home = Path::new("/home/u");

        // Both env vars set → both override.
        let (dsh, agents) = resolve_homes(
            Some("/custom/dsh".into()),
            Some("/custom/agents".into()),
            home,
        );
        assert_eq!(dsh, PathBuf::from("/custom/dsh"));
        assert_eq!(agents, PathBuf::from("/custom/agents"));

        // Both unset → ~/.dsh and ~/.agents fallbacks.
        let (dsh, agents) = resolve_homes(None, None, home);
        assert_eq!(dsh, home.join(".dsh"));
        assert_eq!(agents, home.join(".agents"));

        // DSH_HOME set, DSH_AGENTS_HOME unset → mixed.
        let (dsh, agents) = resolve_homes(Some("/custom/dsh".into()), None, home);
        assert_eq!(dsh, PathBuf::from("/custom/dsh"));
        assert_eq!(agents, home.join(".agents"));
    }

    #[test]
    fn detect_requires_dsh_home() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());
        std::fs::create_dir_all(tmp.path().join(".dsh")).unwrap();
        assert!(adapter.detect());
    }

    #[test]
    fn skill_dirs_cover_dsh_and_agents_homes() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        assert_eq!(
            adapter.skill_dirs(),
            vec![
                tmp.path().join(".dsh/skills"),
                tmp.path().join(".agents/skills"),
            ]
        );
        // Canonical install target is the dsh-owned dir (skill_dir_for uses first).
        assert_eq!(adapter.project_skill_dirs(), vec![".dsh/skills".to_string()]);
        assert_eq!(
            adapter.project_skill_read_dirs(),
            vec![".agents/skills".to_string()]
        );
    }

    #[test]
    fn config_discovery_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let dsh_home = tmp.path().join(".dsh");
        // settings files include existing per-profile patch files
        std::fs::create_dir_all(dsh_home.join("profiles/web")).unwrap();
        std::fs::write(dsh_home.join("profiles/web/cordis.patch.yml"), "[]\n").unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());

        assert_eq!(adapter.global_rules_files(), vec![dsh_home.join("AGENTS.md")]);
        let settings = adapter.global_settings_files();
        assert!(settings.contains(&dsh_home.join("settings.yaml")));
        assert!(settings.contains(&dsh_home.join("cordis.patch.yml")));
        assert!(settings.contains(&dsh_home.join("profiles/web/cordis.patch.yml")));
        assert_eq!(
            adapter.project_rules_patterns(),
            vec!["AGENTS.md", "CLAUDE.md", "AGENTS.local.md", "CLAUDE.local.md"]
        );
        // HK-side project-discovery marker (dsh's only project-level dir).
        // dsh itself finds project roots by walking to the nearest `.git`.
        assert_eq!(adapter.project_markers(), vec![super::super::ProjectMarker::Dir(".dsh")]);
        // No project-level MCP/hook config exists upstream.
        assert_eq!(adapter.project_mcp_config_relpath(), None);
        assert_eq!(adapter.project_hook_config_relpath(), None);
    }

    /// Pins the load-bearing dependency behavior: serde_yaml parses `!!js`
    /// scalars (plain / quoted / block forms) WITHOUT error and silently
    /// strips the tag, yielding the expression as a plain String. Every
    /// reader below builds on this. If this test ever fails, serde_yaml's
    /// tag handling changed — re-verify the readers before touching them.
    #[test]
    fn serde_yaml_strips_double_bang_tags_to_plain_strings() {
        for (text, expected) in [
            ("k: !!js process.cwd()", "process.cwd()"),
            ("k: !!js '`Bearer ${process.env.T}`'", "`Bearer ${process.env.T}`"),
            (
                "k: !!js >-\n  process.env.X?.trim() ||\n  fallback()",
                "process.env.X?.trim() || fallback()",
            ),
        ] {
            let v: serde_yaml::Value = serde_yaml::from_str(text).unwrap();
            assert_eq!(v.get("k").and_then(|v| v.as_str()), Some(expected), "input: {text}");
        }
    }

    /// Home-layer fixture. The `env` block-scalar entry is copied from dsh's
    /// own examples/mcp-memory/mcp-reference-memory.cordis.yml — its README
    /// tells users to merge exactly this into the patch files HK reads.
    const HOME_PATCH: &str = r#"# user layer — keep this comment
- insert:
    - id: mcp-github
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: github
        transport: stdio
        command: npx
        args: ['-y', '@modelcontextprotocol/server-github']
        cwd: !!js process.cwd()
        env:
          GITHUB_TOKEN: !!js process.env.GITHUB_TOKEN
          MEMORY_FILE_PATH: !!js >-
            process.env.MEMORY_FILE_PATH?.trim() ||
            process.getBuiltinModule('node:path').join(process.cwd(), 'memory.json')
    - id: mcp-web
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: web
        transport: streamable-http
        url: http://localhost:3000/mcp
        headers:
          Authorization: !!js '`Bearer ${process.env.MCP_TOKEN}`'
- id: mcp-github
  disabled: true
"#;

    fn write_home_patch(home: &Path, text: &str) {
        std::fs::create_dir_all(home.join(".dsh")).unwrap();
        std::fs::write(home.join(".dsh/cordis.patch.yml"), text).unwrap();
    }

    #[test]
    fn read_mcp_servers_parses_home_layer_with_js_tags() {
        let tmp = tempfile::tempdir().unwrap();
        write_home_patch(tmp.path(), HOME_PATCH);
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 2);

        let gh = servers.iter().find(|s| s.name == "github").unwrap();
        assert!(!gh.enabled, "later same-file override disabled mcp-github");
        assert_eq!(gh.command, "npx");
        assert_eq!(gh.args, vec!["-y", "@modelcontextprotocol/server-github"]);
        // !!js values arrive tag-stripped as bare expression text — shown
        // as-is, never evaluated (see the probe test above).
        assert_eq!(gh.env["GITHUB_TOKEN"], "process.env.GITHUB_TOKEN");
        assert!(gh.env["MEMORY_FILE_PATH"].starts_with("process.env.MEMORY_FILE_PATH?.trim()"));

        let web = servers.iter().find(|s| s.name == "web").unwrap();
        assert_eq!(web.transport, McpTransport::Http);
        assert_eq!(web.url.as_deref(), Some("http://localhost:3000/mcp"));
        assert_eq!(web.headers["Authorization"], "`Bearer ${process.env.MCP_TOKEN}`");
        assert!(web.enabled);
    }

    #[test]
    fn disabled_false_later_in_file_reenables() {
        let tmp = tempfile::tempdir().unwrap();
        let text = format!("{HOME_PATCH}- id: mcp-github\n  disabled: false\n");
        write_home_patch(tmp.path(), &text);
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let gh = adapter
            .read_mcp_servers()
            .into_iter()
            .find(|s| s.name == "github")
            .unwrap();
        assert!(gh.enabled, "later entry wins (single ordered apply upstream)");
    }

    #[test]
    fn bare_override_never_creates_a_row() {
        // Upstream: a patch targeting an unknown id is warn+skip, never a
        // definition (vendor/include/src/index.ts:107-112).
        let tmp = tempfile::tempdir().unwrap();
        write_home_patch(
            tmp.path(),
            "- id: ghost\n  name: '@deepseek-ai/dsh-mcp-client'\n  config:\n    serverName: ghost\n",
        );
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.read_mcp_servers().is_empty());
    }

    #[test]
    fn mcp_row_id_lookup_by_server_name() {
        assert_eq!(
            DshAdapter::mcp_row_id_in_text(HOME_PATCH, "github").as_deref(),
            Some("mcp-github")
        );
        assert_eq!(DshAdapter::mcp_row_id_in_text(HOME_PATCH, "nope"), None);
    }

    #[test]
    fn mcp_enabled_map_reflects_folded_state() {
        let map = DshAdapter::mcp_enabled_in_text(HOME_PATCH);
        assert_eq!(map.len(), 2);
        assert_eq!(map["github"], false);
        assert_eq!(map["web"], true);

        // `disabled: null` ≡ false upstream (static rule, entry.ts:104-107) —
        // a null override re-enables a previously disabled row.
        let text = format!("{HOME_PATCH}- id: mcp-github\n  disabled: null\n");
        let map = DshAdapter::mcp_enabled_in_text(&text);
        assert_eq!(map["github"], true, "disabled: null re-enables");
    }

    #[test]
    fn url_decides_remote_even_without_a_transport_key() {
        // A url-bearing row with `transport` omitted (or set to anything other
        // than streamable-http) is still remote — never a stdio entry with an
        // empty command.
        let tmp = tempfile::tempdir().unwrap();
        write_home_patch(
            tmp.path(),
            r#"- insert:
    - id: mcp-a
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: a
        url: https://a.example/mcp
    - id: mcp-b
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: b
        transport: sse
        url: https://b.example/mcp
"#,
        );
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 2);
        for s in &servers {
            assert_eq!(s.transport, McpTransport::Http, "{} should be remote", s.name);
            assert!(s.command.is_empty(), "{} should carry no command", s.name);
            assert!(s.url.is_some(), "{} should keep its url", s.name);
        }
    }

    #[test]
    fn read_mcp_servers_from_reads_the_given_file() {
        // The service delete path locates entries via read_mcp_servers_from;
        // returning them (instead of the trait's empty default) turns dsh MCP
        // deletion into a loud DshCordis error instead of a silent no-op.
        let tmp = tempfile::tempdir().unwrap();
        write_home_patch(tmp.path(), HOME_PATCH);
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers_from(&tmp.path().join(".dsh/cordis.patch.yml"));
        assert_eq!(servers.len(), 2);
    }
}
