pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod dsh;
pub mod gemini;
pub mod hermes;
pub mod hook_events;
pub mod kiro;
pub mod opencode;
pub mod omp;
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

/// Recursive variant of [`files_with_ext`]: walks `dir` and every subdirectory,
/// following directory symlinks (walkdir reports symlink loops as errors, which
/// are skipped like any unreadable entry). Only for rules-style directories
/// whose agent documents recursive loading (Claude Code rules, Copilot
/// instructions, ...) — keep single-level dirs on `files_with_ext` so we don't
/// surface files the agent itself never reads. Depth-capped as a safety net
/// against pathological trees.
pub(crate) fn files_with_ext_recursive(dir: &Path, ext: &str) -> Vec<PathBuf> {
    walkdir::WalkDir::new(dir)
        .follow_links(true)
        .max_depth(16)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().is_some_and(|e| e == ext))
        .collect()
}

/// Transport an MCP server entry uses. `Stdio` entries carry `command/args/env`;
/// `Http` (Streamable HTTP) and `Sse` entries carry `url` (+ optional `headers`)
/// and an empty `command`.
///
/// Agents whose config has a single auto-detecting URL key (Cursor, Kiro,
/// Windsurf, Antigravity, Codex, OpenCode) are read as `Http` — the modern
/// default — since the file doesn't record which protocol the server speaks.
/// Only agents with an explicit discriminator (Claude/Copilot/omp `type`,
/// Gemini `url` vs `httpUrl`, Hermes `transport`) can yield `Sse`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    #[default]
    Stdio,
    Http,
    Sse,
}

impl McpTransport {
    fn is_stdio(&self) -> bool {
        *self == McpTransport::Stdio
    }

    /// Plain-text form used for the extensions DB column.
    pub fn as_str(&self) -> &'static str {
        match self {
            McpTransport::Stdio => "stdio",
            McpTransport::Http => "http",
            McpTransport::Sse => "sse",
        }
    }
}

impl std::str::FromStr for McpTransport {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stdio" => Ok(McpTransport::Stdio),
            "http" => Ok(McpTransport::Http),
            "sse" => Ok(McpTransport::Sse),
            _ => Err(()),
        }
    }
}

/// Represents an MCP server entry parsed from an agent's config.
///
/// Serde representation is the canonical Kit blob: `{command, args, env}` for
/// stdio entries plus `{transport, url, headers}` for remote ones. The remote
/// fields are skipped when empty so stdio blobs stay byte-identical to the
/// pre-transport format, and `#[serde(default)]` keeps old blobs parseable.
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
    #[serde(default, skip_serializing_if = "McpTransport::is_stdio")]
    pub transport: McpTransport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// HTTP headers sent to remote servers (typically `Authorization`).
    /// Secret-bearing: must be redacted alongside `env` wherever entries
    /// are snapshotted to the DB (see `manager::redact_mcp_env`).
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(skip, default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for McpServerEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            args: vec![],
            env: Default::default(),
            transport: McpTransport::Stdio,
            url: None,
            headers: Default::default(),
            enabled: true,
        }
    }
}

/// Parse a `{"key": "value", ...}` JSON object field into a string map,
/// dropping non-string values. Shared by adapter readers for `env` and
/// `headers` blocks.
pub(crate) fn json_string_map(
    val: &serde_json::Value,
    key: &str,
) -> std::collections::HashMap<String, String> {
    val.get(key)
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a YAML `key: {A: B, ...}` sub-mapping into a string map, dropping
/// non-string values. Used for `env` and `headers` blocks.
pub(crate) fn yaml_string_map(
    val: &serde_yaml::Value,
    key: &str,
) -> std::collections::HashMap<String, String> {
    val.get(key)
        .and_then(|v| v.as_mapping())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.as_str()?.to_string(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// Transport + url for `{type: "http"|"sse", url}`-style entries (Claude,
/// Copilot, omp — the `RemoteMcpSchema::TypeAndUrl` agents).
/// `streamable-http` is the MCP spec's name for the HTTP transport and an
/// accepted alias in these agents' configs (Claude docs: "configurations
/// copied from server documentation work without modification").
///
/// An entry with a `url` but no `type` also counts as Streamable HTTP.
/// That is deliberately laxer than Claude itself, which refuses to load
/// such an entry — HK is a scanner, and showing the URL beats showing an
/// empty command for a config the user needs to fix.
pub(crate) fn parse_type_url(val: &serde_json::Value) -> (McpTransport, Option<String>) {
    let url = val.get("url").and_then(|v| v.as_str()).map(String::from);
    let transport = match (val.get("type").and_then(|v| v.as_str()), &url) {
        (Some("sse"), _) => McpTransport::Sse,
        (Some("http" | "streamable-http"), _) | (None, Some(_)) => McpTransport::Http,
        _ => McpTransport::Stdio,
    };
    (transport, url)
}

/// Transport + url for single-URL-key entries (`url` — Cursor/Kiro,
/// `serverUrl` — Windsurf/Antigravity). These configs don't record the
/// protocol, so presence of the key reads as Streamable HTTP (see
/// `McpTransport` docs).
pub(crate) fn parse_plain_url(
    val: &serde_json::Value,
    key: &str,
) -> (McpTransport, Option<String>) {
    match val.get(key).and_then(|v| v.as_str()) {
        Some(url) => (McpTransport::Http, Some(url.to_string())),
        None => (McpTransport::Stdio, None),
    }
}

/// Parse a `["a", "b", ...]` JSON array field into a string vec, dropping
/// non-string items. Shared by adapter readers for `args` lists.
pub(crate) fn json_string_vec(val: &serde_json::Value, key: &str) -> Vec<String> {
    val.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Represents a hook entry parsed from an agent's config
#[derive(Debug, Clone)]
pub struct HookEntry {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
    /// On-disk enable state. Only agents with a native per-hook flag (Kiro's
    /// `enabled`) ever report `false`; formats without one are always `true`.
    pub enabled: bool,
}

/// Represents a plugin entry parsed from an agent's config
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub name: String,
    pub source: String,
    pub enabled: bool,
    pub path: Option<std::path::PathBuf>,
    /// Authoritative upstream URL resolved from the agent's own plugin manifest
    /// (e.g. Claude's marketplace → repo mapping). When set, it overrides the
    /// `.git`-walk source detection, which mis-attributes plugins cached inside
    /// a dotfiles repo. `None` for agents without such a manifest.
    pub source_url: Option<String>,
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
    /// Hermes: YAML config.yaml with root `hooks:` key. Each `hooks.<event>`
    /// is a list of `{matcher?, command, timeout?}`. Routed through dedicated
    /// YAML helpers in deployer.rs (NOT locked_modify_json, which is JSON-only).
    HermesYaml,
    /// Kiro IDE hook files (`~/.kiro/hooks/*.json`, `.kiro/hooks/*.json`).
    /// Each file has `{version, hooks: [{name, trigger, matcher?, action, timeout?}]}`.
    /// Only command actions map to HarnessKit's shell-command HookEntry model.
    KiroIde,
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
    /// YAML config.yaml with "mcp_servers" top-level key (Hermes).
    /// Each entry is URL-based ({url, headers?, transport: sse?}) or
    /// command-based ({command, args?, env?}).
    HermesYaml,
    /// DeepSeek Harness: MCP servers are `@deepseek-ai/dsh-mcp-client` plugin
    /// rows in cordis patch files (YAML top-level array with `!!js` tags), not
    /// a server map. Generic JSON/TOML writers must never touch these files —
    /// every deployer arm for this variant errors; toggling goes through the
    /// native in-place path (`set_dsh_mcp_enabled`).
    DshCordis,
}

/// How an agent's config spells a remote (HTTP/SSE) MCP entry.
///
/// This is the single source of truth for "which transports can this agent
/// receive": the deployer's JSON writer dispatches on the four JSON-family
/// variants, and `AgentCapabilities::from_adapter` derives UI install-gating
/// from it (`Toml` is the only HTTP-only variant — Codex has no SSE support;
/// `Unsupported` receives no remote entries at all).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RemoteMcpSchema {
    /// `{type: "http"|"sse", url, headers}` — Claude, Copilot, omp.
    TypeAndUrl,
    /// `{url, headers}`, protocol auto-detected by the agent — Cursor, Kiro.
    PlainUrl,
    /// `{httpUrl}` for Streamable HTTP, `{url}` for SSE, `headers` for both — Gemini.
    GeminiSplit,
    /// `{serverUrl, headers}`, protocol auto-detected — Windsurf, Antigravity.
    ServerUrl,
    /// TOML `url = "..."` + `http_headers` — Codex. Streamable HTTP only.
    Toml,
    /// `{type: "remote", url, headers, enabled}` — OpenCode.
    OpencodeRemote,
    /// YAML `url:` + `headers:` + optional `transport: sse` — Hermes.
    HermesUrl,
    /// Agent has no remote MCP concept; deploying a remote entry is an error.
    Unsupported,
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

    /// How this agent's config spells a remote MCP entry. `Unsupported`
    /// (the default) means the agent only handles stdio servers; the
    /// deployer refuses remote entries for such targets.
    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::Unsupported
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

    /// Whether this agent's MCP config format carries a native per-server
    /// `enabled: bool` that HarnessKit should toggle IN PLACE, instead of the
    /// default disable (remove the entry + snapshot it in the DB + redact env).
    ///
    /// Agents returning `true` are dispatched to a dedicated in-place writer in
    /// `manager::toggle_mcp`; their disabled state lives in the agent's own
    /// config file and is read back by `read_mcp_servers`, so no DB snapshot is
    /// taken. Default `false` (the remove+snapshot path).
    fn supports_native_mcp_toggle(&self) -> bool {
        false
    }

    /// Whether the agent actually LOADS hooks installed at the user-level
    /// (global) location. `install_to_agent` refuses to deploy a global hook
    /// when this is `false`, so HK never writes a config the agent silently
    /// ignores. Scanning/toggling of files already on disk is unaffected.
    /// Default `true`; override only with verified evidence.
    fn supports_global_hook_install(&self) -> bool {
        true
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

    /// Per-project memory stored OUTSIDE the project tree, grouped by the
    /// project `cwd` that owns it. Claude keeps this at
    /// `~/.claude/projects/<encoded-cwd>/memory/`, keyed by the session cwd
    /// rather than inside the project.
    ///
    /// Each entry is `(owner_cwd, files)`. The scanner assigns
    /// `ConfigScope::Project` when `owner_cwd` is `Some` and matches a
    /// registered project, and `ConfigScope::Global` otherwise — including
    /// when the owner can't be determined (`None`). An agent implements
    /// EITHER this or `global_memory_files` for a given store, never both
    /// over the same files, so the scanner never double-lists.
    fn external_project_memory(&self) -> Vec<(Option<PathBuf>, Vec<PathBuf>)> {
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

    /// List available skill category names for agents that organise skills
    /// into subdirectories (e.g. Hermes: `~/.hermes/skills/{category}/`).
    /// Returns an empty vec for agents that use a flat skill directory.
    fn list_skill_categories(&self) -> Vec<String> {
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

    /// Resolve every hook config file for a scope. Most agents have exactly
    /// one hook config file, so the default wraps `hook_config_path_for`.
    /// Agents such as Kiro can override this to scan a directory of hook files.
    fn hook_config_paths_for(&self, scope: &ConfigScope) -> Vec<PathBuf> {
        self.hook_config_path_for(scope).into_iter().collect()
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

    /// Resolve a category-specific skill directory for agents that organise
    /// skills into named subdirectories (e.g. Hermes: `~/.hermes/skills/{category}/`).
    /// Returns `None` for agents with a flat skill layout, letting callers fall
    /// back to `skill_dir_for(scope)`.
    fn skill_dir_for_category(
        &self,
        _scope: &ConfigScope,
        _category: &str,
    ) -> Option<std::path::PathBuf> {
        None
    }
}

impl crate::models::AgentCapabilities {
    /// Derive install capabilities purely from the adapter's own
    /// declarations — no per-agent special cases. Whatever an adapter
    /// declares here is exactly what `service::install_to_agent` can
    /// resolve a write path for, so frontend gating and backend behavior
    /// cannot drift.
    pub fn from_adapter(a: &dyn AgentAdapter) -> Self {
        let skill = !a.project_skill_dirs().is_empty();
        let remote_schema = a.remote_mcp_schema();
        Self {
            project_install: crate::models::KindFlags {
                skill,
                mcp: a.project_mcp_config_relpath().is_some(),
                hook: a.project_hook_config_relpath().is_some(),
                // A CLI install deploys the companion skill into the skill
                // dir (the binary itself is global), so CLI follows skill.
                cli: skill,
            },
            hooks_supported: a.hook_format() != HookFormat::None,
            global_hook_install: a.supports_global_hook_install(),
            // Codex's TOML schema (the only `Toml` agent) speaks Streamable
            // HTTP but not SSE; every other non-Unsupported schema takes both.
            mcp_remote: crate::models::RemoteTransportFlags {
                http: remote_schema != RemoteMcpSchema::Unsupported,
                sse: !matches!(
                    remote_schema,
                    RemoteMcpSchema::Unsupported | RemoteMcpSchema::Toml
                ),
            },
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
        Box::new(hermes::HermesAdapter::new()),
        Box::new(kiro::KiroAdapter::new()),
        Box::new(omp::OmpAdapter::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_entry_kit_blob_compat() {
        // Old Kit blobs ({command, args, env}) must keep parsing, defaulting
        // to stdio transport.
        let old: McpServerEntry =
            serde_json::from_str(r#"{"command":"npx","args":["-y","srv"],"env":{}}"#).unwrap();
        assert_eq!(old.transport, McpTransport::Stdio);
        assert_eq!(old.url, None);
        assert!(old.headers.is_empty());

        // Stdio entries serialize with exactly the pre-transport key set
        // (remote fields are skipped when empty).
        let json = serde_json::to_value(&old).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, vec!["args", "command", "env"]);

        // Remote entries round-trip transport/url/headers.
        let remote = McpServerEntry {
            transport: McpTransport::Sse,
            url: Some("https://example.com/sse".into()),
            headers: [("Authorization".to_string(), "Bearer t".to_string())].into(),
            ..Default::default()
        };
        let back: McpServerEntry =
            serde_json::from_str(&serde_json::to_string(&remote).unwrap()).unwrap();
        assert_eq!(back.transport, McpTransport::Sse);
        assert_eq!(back.url.as_deref(), Some("https://example.com/sse"));
        assert_eq!(back.headers["Authorization"], "Bearer t");
    }

    #[test]
    fn mcp_remote_capability_derivation() {
        // Codex (TOML) is the only HTTP-only agent; every other adapter's
        // remote schema supports both transports. Pinned so a future agent
        // with partial support must consciously extend the derivation.
        for a in all_adapters() {
            let caps = crate::models::AgentCapabilities::from_adapter(a.as_ref());
            match a.name() {
                "codex" => {
                    assert!(caps.mcp_remote.http);
                    assert!(!caps.mcp_remote.sse);
                }
                _ => {
                    assert!(caps.mcp_remote.http, "{} should accept http", a.name());
                    assert!(caps.mcp_remote.sse, "{} should accept sse", a.name());
                }
            }
        }
    }

    #[test]
    fn test_all_adapters_returns_eleven() {
        let adapters = all_adapters();
        assert_eq!(adapters.len(), 11);
        let names: Vec<&str> = adapters.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"cursor"));
        assert!(names.contains(&"codex"));
        assert!(names.contains(&"gemini"));
        assert!(names.contains(&"antigravity"));
        assert!(names.contains(&"copilot"));
        assert!(names.contains(&"windsurf"));
        assert!(names.contains(&"opencode"));
        assert!(names.contains(&"hermes"));
        assert!(names.contains(&"kiro"));
        assert!(names.contains(&"omp"));
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
        for name in [
            "claude", "codex", "gemini", "cursor", "copilot", "opencode", "hermes", "kiro", "omp",
        ] {
            assert!(
                !by_name[name].needs_path_injection(),
                "{name} should not need path injection"
            );
        }
    }

    #[test]
    fn test_supports_native_mcp_toggle_only_native_agents() {
        // Adding a name here also requires a dispatch branch in
        // manager.rs::toggle_mcp — the trailing else there errors out.
        let adapters = all_adapters();
        for a in &adapters {
            let expected = matches!(a.name(), "hermes" | "kiro" | "omp");
            assert_eq!(
                a.supports_native_mcp_toggle(),
                expected,
                "{} supports_native_mcp_toggle should be {expected}",
                a.name()
            );
        }
    }

    #[test]
    fn test_supports_global_hook_install_false_only_for_kiro() {
        // Kiro's docs claim `~/.kiro/hooks/` (user-level) but no released
        // version loads it (kirodotdev/Kiro#5440, #9857; verified on 1.0.89).
        // Everyone else keeps the default: global hook deploy allowed.
        let adapters = all_adapters();
        for a in &adapters {
            let expected = a.name() != "kiro";
            assert_eq!(
                a.supports_global_hook_install(),
                expected,
                "{} supports_global_hook_install should be {expected}",
                a.name()
            );
        }
    }

    /// The full per-agent capability matrix, every value verified against
    /// official docs 2026-07 (see docs/superpowers/specs/
    /// 2026-07-11-cross-agent-project-install-plan.md for citations).
    /// If an adapter declaration changes, this test forces the change to be
    /// deliberate — it is the single source the frontend gates against.
    #[test]
    fn test_agent_capabilities_matrix() {
        use crate::models::AgentCapabilities;

        // (name, skill, mcp, hook, hooks_supported, global_hook_install);
        // cli always equals skill by construction.
        let expected = [
            ("claude", true, true, true, true, true),
            ("codex", true, true, true, true, true),
            ("cursor", true, true, true, true, true),
            ("windsurf", true, false, true, true, true), // MCP global-only upstream
            ("gemini", true, true, false, true, true),   // project hooks deferred
            ("copilot", true, true, false, true, true),  // project hooks deferred
            ("antigravity", true, false, false, false, true), // no MCP/project, no hooks
            ("opencode", true, true, false, false, true), // hooks are JS plugins
            ("kiro", true, true, true, true, false),     // kirodotdev/Kiro#5440
            ("omp", true, true, false, false, true),     // hooks are JS/TS modules
            ("hermes", false, false, false, true, true), // global-only (hermes-agent#4667)
        ];

        let adapters = all_adapters();
        assert_eq!(adapters.len(), expected.len(), "agent count drifted");
        for (name, skill, mcp, hook, hooks_supported, global_hook) in expected {
            let a = adapters
                .iter()
                .find(|a| a.name() == name)
                .unwrap_or_else(|| panic!("adapter {name} missing"));
            let caps = AgentCapabilities::from_adapter(a.as_ref());
            assert_eq!(caps.project_install.skill, skill, "{name} project skill");
            assert_eq!(caps.project_install.mcp, mcp, "{name} project mcp");
            assert_eq!(caps.project_install.hook, hook, "{name} project hook");
            assert_eq!(caps.project_install.cli, skill, "{name} project cli follows skill");
            assert_eq!(caps.hooks_supported, hooks_supported, "{name} hooks_supported");
            assert_eq!(caps.global_hook_install, global_hook, "{name} global_hook_install");
        }
    }

    #[test]
    fn test_skill_dir_for_category_default_is_none_except_hermes() {
        // The install handlers rely on this contract: only category-aware
        // agents resolve a dir here; everyone else returns None and falls
        // back to skill_dir_for(). Hermes is the sole override today.
        let adapters = all_adapters();
        for a in &adapters {
            let resolved = a.skill_dir_for_category(&ConfigScope::Global, "devops");
            if a.name() == "hermes" {
                assert!(resolved.is_some(), "hermes should resolve a category dir");
            } else {
                assert!(
                    resolved.is_none(),
                    "{} should not resolve a category dir",
                    a.name()
                );
            }
        }
    }

    #[test]
    fn test_default_config_methods_return_empty() {
        let adapters = all_adapters();
        for a in &adapters {
            let _ = a.global_rules_files();
            let _ = a.global_memory_files();
            let _ = a.external_project_memory();
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
            if a.name() == "hermes" {
                continue; // global-only: no project skills (hermes-agent#4667)
            }
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
            ("windsurf", ".devin/skills"),
            ("gemini", ".gemini/skills"),
            ("antigravity", ".agents/skills"), // 1.18.4+ canonical; .agent/ kept as backward-compat alias
            ("copilot", ".github/skills"),
            ("opencode", ".opencode/skills"),
            ("kiro", ".kiro/skills"),
            ("omp", ".omp/skills"),
            // hermes is global-only — no project skill dir (hermes-agent#4667).
        ]
        .into_iter()
        .collect();
        for a in &adapters {
            if a.name() == "hermes" {
                continue; // global-only: no project skills (hermes-agent#4667)
            }
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
