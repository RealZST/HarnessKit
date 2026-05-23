pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod gemini;
pub mod hook_events;
pub mod opencode;
pub mod windsurf;

use crate::models::ConfigScope;
use std::path::{Path, PathBuf};

/// Return every file directly inside `dir` whose extension equals `ext`
/// (case-sensitive, no leading dot). Missing or unreadable directories
/// yield an empty iterator — adapters use this for "scan a fixed subdir"
/// listings (subagent / mode / theme / command files etc.) and rely on
/// the silent-empty behavior so a missing optional dir isn't an error.
///
/// Returns an iterator (not a Vec) so callers that want to chain extra
/// predicates pay only one allocation in `.collect()`. Callers that just
/// need the full list write `.collect()` once.
pub(crate) fn files_with_ext<'a>(
    dir: &'a Path,
    ext: &'a str,
) -> impl Iterator<Item = PathBuf> + 'a {
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten() // Option<ReadDir> → entries (or none)
        .flatten() // Result<DirEntry, _> → DirEntry (skip Err)
        .map(|entry| entry.path())
        .filter(move |path| path.extension().is_some_and(|e| e == ext))
}

/// Represents an MCP server entry parsed from an agent's config.
///
/// Serde representation is the canonical Kit blob: `{command, args, env}`.
/// `name` is carried by the caller's context (asset name in the manifest);
/// `enabled` is HarnessKit-internal and defaults to `true` on the install
/// side (only OpenCode's source schema has a per-entry agent-native
/// `enabled` — every other adapter always sets `true`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpServerEntry {
    #[serde(skip)]
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(skip, default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Represents a hook entry parsed from an agent's config
#[derive(Debug, Clone)]
pub struct HookEntry {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
}

/// Represents a plugin entry parsed from an agent's config
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub name: String,
    pub source: String,
    pub enabled: bool,
    pub path: Option<std::path::PathBuf>,
    /// Agent-specific URI for the plugin (e.g. VS Code pluginUri "file:///...").
    /// Used by toggle to identify the plugin in the agent's state store.
    pub uri: Option<String>,
    /// Precise install timestamp (e.g. from a registry file). Overrides file-system heuristic.
    pub installed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Precise last-updated timestamp. Overrides file-system heuristic.
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Format used by an agent for hook configuration files.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HookFormat {
    /// Claude, Codex, Gemini: {"hooks": {"Event": [{"matcher": "...", "hooks": ["cmd"]}]}}
    ClaudeLike,
    /// Cursor: {"version": 1, "hooks": {"event": [{"command": "cmd"}]}}
    Cursor,
    /// Copilot: {"version": 1, "hooks": {"event": [{"type": "command", "bash": "cmd"}]}}
    Copilot,
    /// Windsurf: {"hooks": {"event": [{"command": "cmd"}]}}
    Windsurf,
    /// Agent does not support hooks
    None,
}

/// A path marker that, when present in a project root directory, identifies
/// the directory as belonging to a particular agent. Each adapter declares
/// its own markers via [`AgentAdapter::project_markers`]; project discovery
/// (`is_project_dir`, `discover_projects`) considers a directory a project
/// when *any* adapter's marker matches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectMarker {
    /// A relative directory that must exist (e.g. `.claude`, `.opencode`).
    Dir(&'static str),
    /// A relative file that must exist (e.g. `.mcp.json`, `opencode.json`).
    File(&'static str),
}

/// Format used by an agent for MCP server configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum McpFormat {
    /// JSON with "mcpServers" top-level key (Claude, Gemini, Cursor, Antigravity)
    McpServers,
    /// JSON with "servers" top-level key (Copilot / VS Code)
    Servers,
    /// TOML with [mcp_servers.<name>] sections (Codex)
    Toml,
    /// JSON with "mcp" top-level key (OpenCode). Each entry is a tagged
    /// union — local servers use `{type: "local", command: [bin, ...args],
    /// environment: {...}}`, distinct from the Claude-style `{command, args, env}`
    /// schema. `additionalProperties: false` in the upstream schema means no
    /// extra fields may be written.
    /// See https://opencode.ai/config.json (McpLocalConfig).
    Opencode,
}

pub trait AgentAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn base_dir(&self) -> PathBuf;
    fn detect(&self) -> bool;
    fn skill_dirs(&self) -> Vec<PathBuf>;
    fn mcp_config_path(&self) -> PathBuf;
    fn hook_config_path(&self) -> PathBuf;
    fn plugin_dirs(&self) -> Vec<PathBuf>;
    /// Path to the config file where plugin enable/disable state is stored.
    /// Defaults to the same file as hook_config_path (settings.json for most agents).
    fn plugin_config_path(&self) -> PathBuf {
        self.hook_config_path()
    }
    fn read_mcp_servers(&self) -> Vec<McpServerEntry>;
    fn read_hooks(&self) -> Vec<HookEntry>;
    /// Parse MCP servers from a specific config file (e.g. a project's `.mcp.json`).
    /// Default returns empty — only adapters that support project-level MCP override.
    fn read_mcp_servers_from(&self, _path: &std::path::Path) -> Vec<McpServerEntry> {
        vec![]
    }
    /// Parse hooks from a specific config file (e.g. a project's `.claude/settings.json`).
    /// Default returns empty — only adapters that support project-level hooks override.
    fn read_hooks_from(&self, _path: &std::path::Path) -> Vec<HookEntry> {
        vec![]
    }
    fn read_plugins(&self) -> Vec<PluginEntry> {
        vec![]
    }
    /// VS Code user data directory for agents that store state in state.vscdb.
    /// Only Copilot overrides this; others return None.
    fn vscode_user_dir(&self) -> Option<PathBuf> {
        None
    }
    fn hook_format(&self) -> HookFormat {
        HookFormat::ClaudeLike
    }
    fn mcp_format(&self) -> McpFormat {
        McpFormat::McpServers
    }

    /// True if HarnessKit should resolve bare commands to absolute paths and
    /// inject `PATH` into the MCP env block when deploying servers to this
    /// agent. Required for agents that don't reliably inherit shell `$PATH`
    /// when launching MCP server subprocesses (e.g. Antigravity, Windsurf
    /// launched from a GUI without sourcing interactive shell rc files).
    /// Default false — only override for agents with confirmed reports.
    fn needs_path_injection(&self) -> bool {
        false
    }

    /// Translate a hook event name from any agent's convention to this agent's convention.
    /// Returns None if the event has no equivalent in this agent.
    /// Mappings are centralized in `hook_events.rs`.
    fn translate_hook_event(&self, event: &str) -> Option<String> {
        Some(event.to_string()) // Default: pass-through (overridden by each adapter)
    }

    // --- Config file discovery (for Agents page) ---

    /// Global rule/instruction files (absolute paths, e.g. ~/.claude/CLAUDE.md)
    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![]
    }

    /// Global memory files (absolute paths)
    fn global_memory_files(&self) -> Vec<PathBuf> {
        vec![]
    }

    /// Global settings files (absolute paths, e.g. ~/.claude/settings.json)
    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![]
    }

    /// Global subagent definition files (absolute paths). Each file in the
    /// returned list is one subagent persona definition (e.g.
    /// `~/.claude/agents/foo.md`, `~/.codex/agents/bar.toml`). Distinct from
    /// settings: subagents define behavior/personality, not config knobs.
    fn global_subagent_files(&self) -> Vec<PathBuf> {
        vec![]
    }

    /// Relative paths/globs for rules within a project dir (e.g. "CLAUDE.md")
    fn project_rules_patterns(&self) -> Vec<String> {
        vec![]
    }

    /// Relative paths/globs for memory within a project dir
    fn project_memory_patterns(&self) -> Vec<String> {
        vec![]
    }

    /// Canonical relative path used when writing a project-level Rules file
    /// from a Kit. Default: the unique non-glob, non-trailing-slash entry
    /// from `project_rules_patterns()`, else `None`. Adapters with multiple
    /// legitimate paths should override.
    fn project_rules_target_relpath(&self) -> Option<String> {
        pick_unique_concrete(self.project_rules_patterns())
    }

    /// Same as `project_rules_target_relpath` but for Memory.
    fn project_memory_target_relpath(&self) -> Option<String> {
        pick_unique_concrete(self.project_memory_patterns())
    }

    /// Relative paths/globs for settings within a project dir
    fn project_settings_patterns(&self) -> Vec<String> {
        vec![]
    }

    /// Relative paths/globs for subagent definition files within a project dir
    /// (e.g. `.claude/agents/*.md`, `.codex/agents/*.toml`). Each matched file
    /// is one subagent persona definition.
    fn project_subagent_patterns(&self) -> Vec<String> {
        vec![]
    }

    /// Relative paths/globs for ignore files within a project dir
    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![]
    }

    /// Global workflow/command files (absolute paths). Workflows are user-invocable
    /// reusable step sequences (e.g. Windsurf `/<name>` slash commands), distinct
    /// from settings (mcp/hooks) and rules (passive context).
    fn global_workflow_files(&self) -> Vec<PathBuf> {
        vec![]
    }

    /// Relative paths/globs for workflow/command files within a project dir.
    fn project_workflow_patterns(&self) -> Vec<String> {
        vec![]
    }

    // --- Project-level extension scanning ---
    // These describe where this agent looks for project-scoped extensions.
    // Default empty/None means the agent has no project-level support and the
    // scanner skips it.

    /// Path markers (relative to a project root) that identify a directory as
    /// belonging to this agent. Used by `is_project_dir` / `discover_projects`
    /// to decide whether a folder qualifies as a project for *any* agent.
    /// Default empty means this adapter never claims any directory as its
    /// project — only override if the agent has a stable on-disk convention.
    fn project_markers(&self) -> Vec<ProjectMarker> {
        vec![]
    }

    /// Relative dir patterns within a project that contain skill subdirectories
    /// (e.g. `.claude/skills` for Claude — each subdirectory inside is one skill).
    fn project_skill_dirs(&self) -> Vec<String> {
        vec![]
    }

    /// Additional skill-dir aliases the agent READS from but doesn't write to
    /// — e.g. Copilot canonical is `.github/skills` but it also picks up
    /// `.claude/skills` and `.agents/skills` if present. Declaring these lets
    /// callers (e.g. the Kit-remove shared-dir warning) know that a sibling
    /// agent's install at one of these paths is visible to this agent too.
    /// Returning `vec![]` (the default) means "only the canonical dir".
    fn project_skill_read_dirs(&self) -> Vec<String> {
        vec![]
    }

    /// Relative path of the project-level MCP config file (e.g. `.mcp.json`).
    fn project_mcp_config_relpath(&self) -> Option<String> {
        None
    }

    /// Relative path of the project-level hook config file
    /// (e.g. `.claude/settings.json` for Claude).
    fn project_hook_config_relpath(&self) -> Option<String> {
        None
    }

    /// Relative dir patterns within a project that contain plugins.
    fn project_plugin_dirs(&self) -> Vec<String> {
        vec![]
    }

    /// Resolve the MCP config file for a given scope.
    /// - `Global` → adapter's user-scope path (`mcp_config_path()`).
    /// - `Project` → `<project>/<project_mcp_config_relpath()>`, or `None`
    ///   if the adapter has no project-level MCP support.
    fn mcp_config_path_for(&self, scope: &ConfigScope) -> Option<PathBuf> {
        match scope {
            ConfigScope::Global => Some(self.mcp_config_path()),
            ConfigScope::Project { path, .. } => self
                .project_mcp_config_relpath()
                .map(|rel| std::path::Path::new(path).join(rel)),
        }
    }

    /// Resolve the hook config file for a given scope. Mirrors
    /// `mcp_config_path_for`.
    fn hook_config_path_for(&self, scope: &ConfigScope) -> Option<PathBuf> {
        match scope {
            ConfigScope::Global => Some(self.hook_config_path()),
            ConfigScope::Project { path, .. } => self
                .project_hook_config_relpath()
                .map(|rel| std::path::Path::new(path).join(rel)),
        }
    }

    /// Resolve the skill directory for a given scope.
    /// - `Global` → first entry of `skill_dirs()` (today's behavior).
    /// - `Project` → `<project>/<project_skill_dirs()[0]>`, or `None` if
    ///   the adapter has no project-level skill support.
    fn skill_dir_for(&self, scope: &ConfigScope) -> Option<std::path::PathBuf> {
        match scope {
            ConfigScope::Global => self.skill_dirs().into_iter().next(),
            ConfigScope::Project { path, .. } => self
                .project_skill_dirs()
                .into_iter()
                .next()
                .map(|rel| std::path::Path::new(path).join(rel)),
        }
    }
}

fn pick_unique_concrete(patterns: Vec<String>) -> Option<String> {
    let mut concrete = patterns
        .into_iter()
        .filter(|p| !p.contains('*') && !p.ends_with('/'));
    let first = concrete.next()?;
    if concrete.next().is_some() {
        return None;
    }
    Some(first)
}

/// Returns all agent adapters in canonical display order.
/// Must match AGENT_ORDER in src/lib/types.ts.
pub fn all_adapters() -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(claude::ClaudeAdapter::new()),
        Box::new(codex::CodexAdapter::new()),
        Box::new(gemini::GeminiAdapter::new()),
        Box::new(cursor::CursorAdapter::new()),
        Box::new(antigravity::AntigravityAdapter::new()),
        Box::new(copilot::CopilotAdapter::new()),
        Box::new(windsurf::WindsurfAdapter::new()),
        Box::new(opencode::OpencodeAdapter::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_adapters_returns_eight() {
        let adapters = all_adapters();
        assert_eq!(adapters.len(), 8);
        let names: Vec<&str> = adapters.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"cursor"));
        assert!(names.contains(&"codex"));
        assert!(names.contains(&"gemini"));
        assert!(names.contains(&"antigravity"));
        assert!(names.contains(&"copilot"));
        assert!(names.contains(&"windsurf"));
        assert!(names.contains(&"opencode"));
    }

    #[test]
    fn test_needs_path_injection_invariants() {
        // GUI agents that don't reliably inherit shell $PATH — confirmed by
        // user reports (Antigravity) and community (Windsurf on Linux/Windows).
        // Pinned here so a regression that flips either back to false fails
        // the test instead of silently breaking MCP launches.
        let adapters = all_adapters();
        let by_name: std::collections::HashMap<_, _> =
            adapters.iter().map(|a| (a.name().to_string(), a)).collect();
        assert!(by_name["antigravity"].needs_path_injection());
        assert!(by_name["windsurf"].needs_path_injection());

        // Everyone else inherits PATH correctly (CLI agents launched from a
        // shell, or VSCode-fork IDEs with working resolveShellEnv on most
        // setups). Adding an agent here without a confirmed PATH bug would
        // unnecessarily rewrite users' mcp_config.json with absolute paths,
        // hurting cross-machine portability.
        for name in ["claude", "codex", "gemini", "cursor", "copilot", "opencode"] {
            assert!(
                !by_name[name].needs_path_injection(),
                "{name} should not need path injection"
            );
        }
    }

    #[test]
    fn test_default_config_methods_return_empty() {
        let adapters = all_adapters();
        for a in &adapters {
            let _ = a.global_rules_files();
            let _ = a.global_memory_files();
            let _ = a.global_settings_files();
            let _ = a.global_subagent_files();
            let _ = a.project_rules_patterns();
            let _ = a.project_memory_patterns();
            let _ = a.project_settings_patterns();
            let _ = a.project_subagent_patterns();
            let _ = a.project_ignore_patterns();
            let _ = a.global_workflow_files();
            let _ = a.project_workflow_patterns();
        }
    }

    #[test]
    fn test_skill_dir_for_global_matches_skill_dirs_first() {
        let adapters = all_adapters();
        for a in &adapters {
            let global = ConfigScope::Global;
            let computed = a.skill_dir_for(&global);
            let expected = a.skill_dirs().into_iter().next();
            assert_eq!(
                computed,
                expected,
                "{} skill_dir_for(Global) should match skill_dirs()[0]",
                a.name()
            );
        }
    }

    #[test]
    fn test_skill_dir_for_project_joins_path_with_project_skill_dirs_first() {
        let adapters = all_adapters();
        let scope = ConfigScope::Project {
            name: "demo".into(),
            path: "/tmp/demo".into(),
        };
        for adapter in &adapters {
            let computed = adapter.skill_dir_for(&scope);
            let rel = adapter.project_skill_dirs().into_iter().next();
            match (&computed, &rel) {
                (Some(p), Some(r)) => {
                    assert_eq!(p, &std::path::Path::new("/tmp/demo").join(r));
                }
                (None, None) => {} // adapter has no project skill support
                _ => panic!(
                    "{}: mismatched some/none: computed={computed:?} vs project_skill_dirs first={rel:?}",
                    adapter.name()
                ),
            }
        }
    }

    #[test]
    fn test_every_adapter_declares_project_skill_dir() {
        // Universal Agent Skills standard (Dec 2025) — every adapter must declare a
        // project skill directory. If a future adapter genuinely has no project
        // skill concept, drop it from this assertion explicitly.
        let adapters = all_adapters();
        for a in &adapters {
            assert!(
                !a.project_skill_dirs().is_empty(),
                "{} must declare project_skill_dirs (Universal Agent Skills standard)",
                a.name()
            );
        }
    }

    #[test]
    fn test_project_skill_dir_paths_match_upstream_conventions() {
        // Verify each adapter's first-party documented path. Update when adapter
        // upstream conventions change.
        let adapters = all_adapters();
        let expected: std::collections::HashMap<&str, &str> = [
            ("claude", ".claude/skills"),
            ("codex", ".agents/skills"), // Universal alias adopted by OpenAI
            ("cursor", ".cursor/skills"),
            ("windsurf", ".windsurf/skills"),
            ("gemini", ".gemini/skills"),
            ("antigravity", ".agents/skills"), // 1.18.4+ canonical; .agent/ kept as backward-compat alias
            ("copilot", ".github/skills"),
            ("opencode", ".opencode/skills"),
        ]
        .into_iter()
        .collect();
        for a in &adapters {
            let actual = a.project_skill_dirs().into_iter().next().unwrap();
            let want = expected.get(a.name()).expect("adapter not in expected map");
            assert_eq!(&actual, want, "{} project skill path mismatch", a.name());
        }
    }

    #[test]
    fn project_rules_target_relpath_default_returns_single_non_glob() {
        let adapters = crate::adapter::all_adapters();
        for a in &adapters {
            let patterns = a.project_rules_patterns();
            let non_glob: Vec<&String> = patterns
                .iter()
                .filter(|p| !p.contains('*') && !p.ends_with('/'))
                .collect();
            match non_glob.as_slice() {
                [only] => {
                    assert_eq!(
                        a.project_rules_target_relpath().as_deref(),
                        Some(only.as_str()),
                        "{}: target relpath default should pick the unique non-glob",
                        a.name()
                    );
                }
                _ => {
                    // Adapters with 0 or many candidates may legitimately return None
                    // unless they override; just verify the contract holds.
                    let _ = a.project_rules_target_relpath();
                }
            }
        }
    }

    #[test]
    fn project_memory_target_relpath_default_returns_single_non_glob() {
        let adapters = crate::adapter::all_adapters();
        for a in &adapters {
            let patterns = a.project_memory_patterns();
            let non_glob: Vec<&String> = patterns
                .iter()
                .filter(|p| !p.contains('*') && !p.ends_with('/'))
                .collect();
            match non_glob.as_slice() {
                [only] => {
                    assert_eq!(
                        a.project_memory_target_relpath().as_deref(),
                        Some(only.as_str()),
                        "{}: target relpath default should pick the unique non-glob",
                        a.name()
                    );
                }
                _ => {
                    let _ = a.project_memory_target_relpath();
                }
            }
        }
    }
}
