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

use super::{AgentAdapter, HookEntry, HookFormat, McpServerEntry, ProjectMarker};
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

    // mcp_format + supports_native_mcp_toggle overridden in Task 3
    // (McpFormat::DshCordis doesn't exist yet; trait defaults — McpServers /
    // false — apply until then).

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        // Implemented in Task 2.
        vec![]
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
}
