// Grok Build (xAI) config references — verified against
// github.com/xai-org/grok-build @ c2ad97f:
// - Home: crates/codegen/xai-grok-home — `$GROK_HOME` verbatim when
//   non-empty, else `<home>/.grok`. Detection must not create the dir.
// - Skills: `$GROK_HOME/skills` and `~/.agents/skills`, project
//   `.grok/skills` plus `.agents/skills` (SKILL.md in a directory, found
//   recursively). `.agents` is claimed because upstream always reads it
//   (`CompatConfig::skill_config_dirs` hard-codes `.grok` and `.agents`);
//   the `.claude` / `.cursor` trees it gates behind compat cells stay with
//   their own adapters.
// - MCP: `[mcp_servers.<name>]` in `$GROK_HOME/config.toml` and
//   `.grok/config.toml`. Remote key is `headers`; `type = "sse"` is SSE.
//   Personal disable: user `disabled_mcp_servers` + per-entry `enabled`.
// - Hooks: `$GROK_HOME/hooks/*.json` and `.grok/hooks/*.json` (Claude-like).
//   Disable file: `$GROK_HOME/disabled-hooks` (one spec.name per line).
//   Spec name: `{global|project}/<stem>:<snake_event>[i].hooks[j]`. The
//   format is an undocumented internal identifier (only written down as a
//   doc comment on upstream's wire type, xai-hooks-plugins-types) — we
//   mirror `xai-grok-hooks/src/config.rs::build_specs` exactly:
//   [i] is keyed per event ENUM VARIANT (SubagentStop and SubagentEnd are
//   separate variants whose names collide on the shared `subagent_stop`
//   display token), and specs dedup on (canonical event, raw command,
//   matcher) with SubagentEnd folding into SubagentStop. When one file
//   spells one event under 2+ alias keys, upstream merges the groups in
//   std-HashMap iteration order — nondeterministic run to run — so the
//   index we compute may address the sibling entry. Not modeled: the
//   shape is rare, upstream itself is unstable there, and HK reads the
//   real state back from `disabled-hooks` on every scan.
//   Cross-source dedup is NOT modeled: upstream also dedups identical
//   hooks across files (first source wins, config-layer `[hooks]` entries
//   ahead of every file), so a row shadowed by a twin elsewhere is absent
//   from Grok's registry and its disable line would be a no-op.
//   HTTP handlers are skipped — HK's HookEntry is command-only.
// - Plugins: `$GROK_HOME/plugins`, `$GROK_HOME/installed-plugins`,
//   project `.grok/plugins`. Stable id `{scope}/{hex8}/{name}`; hex8 =
//   first 8 hex chars of SHA-256 of the canonical plugin root. Enable
//   lists: `[plugins].enabled` / `[plugins].disabled` (user + project
//   files are merged; disabled wins). User/project plugins default to
//   disabled.
// - Do not surface `auth.json` or `mcp_credentials.json`.

use super::{
    files_with_ext, AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, McpTransport,
    PluginEntry, ProjectMarker, RemoteMcpSchema,
};
use crate::models::ConfigScope;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Grok hook events in upstream `HookEventName` table order (drives spec
/// name indices). `aliases` are accepted JSON keys; `display` is the
/// snake_case token Grok writes into spec names; `canonical` is the
/// PascalCase name HK stores on `HookEntry`.
struct GrokEvent {
    canonical: &'static str,
    display: &'static str,
    aliases: &'static [&'static str],
}

const GROK_EVENTS: &[GrokEvent] = &[
    GrokEvent {
        canonical: "SessionStart",
        display: "session_start",
        aliases: &["SessionStart", "session_start", "sessionStart"],
    },
    GrokEvent {
        canonical: "UserPromptSubmit",
        display: "user_prompt_submit",
        aliases: &["UserPromptSubmit", "user_prompt_submit", "beforeSubmitPrompt"],
    },
    GrokEvent {
        canonical: "PreToolUse",
        display: "pre_tool_use",
        aliases: &[
            "PreToolUse",
            "pre_tool_use",
            "preToolUse",
            "beforeShellExecution",
            "beforeMCPExecution",
            "beforeReadFile",
        ],
    },
    GrokEvent {
        canonical: "PostToolUse",
        display: "post_tool_use",
        aliases: &[
            "PostToolUse",
            "post_tool_use",
            "postToolUse",
            "afterShellExecution",
            "afterMCPExecution",
            "afterFileEdit",
            "afterAgentResponse",
            "afterAgentThought",
        ],
    },
    GrokEvent {
        canonical: "PostToolUseFailure",
        display: "post_tool_use_failure",
        aliases: &[
            "PostToolUseFailure",
            "post_tool_use_failure",
            "postToolUseFailure",
        ],
    },
    GrokEvent {
        canonical: "PermissionDenied",
        display: "permission_denied",
        aliases: &["PermissionDenied", "permission_denied", "permissionDenied"],
    },
    GrokEvent {
        canonical: "Stop",
        display: "stop",
        aliases: &["Stop", "stop"],
    },
    GrokEvent {
        canonical: "StopFailure",
        display: "stop_failure",
        aliases: &["StopFailure", "stop_failure", "stopFailure"],
    },
    GrokEvent {
        canonical: "StopCancelled",
        display: "stop_cancelled",
        aliases: &["StopCancelled", "stop_cancelled", "stopCancelled"],
    },
    GrokEvent {
        canonical: "Notification",
        display: "notification",
        aliases: &["Notification", "notification"],
    },
    GrokEvent {
        canonical: "SubagentStart",
        display: "subagent_start",
        aliases: &["SubagentStart", "subagent_start", "subagentStart"],
    },
    GrokEvent {
        canonical: "SubagentStop",
        display: "subagent_stop",
        aliases: &["SubagentStop", "subagent_stop", "subagentStop"],
    },
    GrokEvent {
        canonical: "SubagentEnd",
        display: "subagent_stop",
        aliases: &["SubagentEnd", "subagent_end", "subagentEnd"],
    },
    GrokEvent {
        canonical: "PreCompact",
        display: "pre_compact",
        aliases: &["PreCompact", "pre_compact", "preCompact"],
    },
    GrokEvent {
        canonical: "PostCompact",
        display: "post_compact",
        aliases: &["PostCompact", "post_compact", "postCompact"],
    },
    GrokEvent {
        canonical: "SessionEnd",
        display: "session_end",
        aliases: &["SessionEnd", "session_end", "sessionEnd"],
    },
];

/// `$GROK_HOME` verbatim when non-empty, else `<home>/.grok`. Pure so tests
/// can cover the override without mutating process env (racy under cargo test).
fn resolve_grok_home(grok_home_env: Option<OsString>, os_home: Option<&Path>) -> Option<PathBuf> {
    if let Some(env) = grok_home_env.filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(env));
    }
    os_home.map(|home| home.join(".grok"))
}

fn toml_string_map(
    table: Option<&toml::Table>,
    key: &str,
) -> std::collections::HashMap<String, String> {
    table
        .and_then(|t| t.get(key))
        .and_then(|v| v.as_table())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn toml_string_list(doc: &toml::Table, key: &str) -> Vec<String> {
    doc.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Stable Grok plugin id: `{scope}/{hex8}/{name}`.
/// `hex8` is the first 8 hex chars of SHA-256 of the canonical root path
/// (`xai-grok-agent/src/plugins/discovery.rs` `PluginId::new`).
/// Must be `dunce::canonicalize`, not `std::fs::canonicalize` — upstream
/// bans the latter repo-wide (grok-build clippy.toml disallowed-methods)
/// because it returns `\\?\` verbatim paths on Windows, which would hash
/// to a different hex8 than the id Grok writes to its plugin lists.
pub fn grok_plugin_id(scope: &str, root: &Path, name: &str) -> String {
    // On canonicalize failure upstream skips the plugin; we keep the row
    // with a raw-path id instead — the failure only occurs in a
    // permission/delete race, and an inspection tool showing the plugin
    // beats hiding it (the id just won't match Grok's in that window).
    let canonical = dunce::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let hash = hasher.finalize();
    format!(
        "{scope}/{:02x}{:02x}{:02x}{:02x}/{name}",
        hash[0], hash[1], hash[2], hash[3]
    )
}

fn grok_event_by_alias(key: &str) -> Option<&'static GrokEvent> {
    GROK_EVENTS.iter().find(|e| e.aliases.contains(&key))
}

/// Upstream `MAX_SKILL_WALK_DEPTH`: SKILL.md is found at most six path
/// segments below a skills root (skills/discovery.rs:19).
const MAX_SKILL_WALK_DEPTH: usize = 5;

/// Collect every directory under `dir` (children visited at `depth`, capped
/// like upstream) whose direct children include a skill dir; returns whether
/// `dir` itself directly holds one, so each directory is read exactly once.
/// Sorted for stable output; note upstream's walk is an interleaved DFS, so
/// on a frontmatter-name collision between a top-level and a nested skill
/// the surviving copy can differ from Grok's. Skills nested INSIDE another
/// skill are legal and both are emitted (upstream test
/// `find_skill_paths_parent_and_child_both_have_skill_md`). Symlinked dirs
/// are followed — the depth cap is the only cycle protection, deliberately
/// matching upstream.
fn collect_nested_skill_parents(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut children: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    children.sort();
    let mut holds_skill = false;
    for child in children {
        if child.join("SKILL.md").is_file() || child.join("SKILL.md.disabled").is_file() {
            holds_skill = true;
        }
        if depth <= MAX_SKILL_WALK_DEPTH {
            let mut nested = Vec::new();
            if collect_nested_skill_parents(&child, depth + 1, &mut nested) {
                out.push(child.clone());
            }
            out.append(&mut nested);
        }
    }
    holds_skill
}

/// Upstream folds the legacy SubagentEnd variant into SubagentStop for
/// dispatch and dedup (`HookEventName::canonical`).
fn dedup_event(canonical: &str) -> &str {
    if canonical == "SubagentEnd" {
        "SubagentStop"
    } else {
        canonical
    }
}

/// Names listed in `$GROK_HOME/disabled-hooks` (comments and blanks skipped).
pub fn read_disabled_hook_names(path: &Path) -> HashSet<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return HashSet::new();
    };
    content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

/// One handler's outcome, split the way upstream's two failure modes are.
/// `Err(())` is a whole-file failure: `RawHandler::handler_type` is a plain
/// `String` and `command`/`url` are `Option<String>`, so a missing `type` or
/// a wrong JSON type anywhere is a serde error, and `GroupErrorPolicy::Fail`
/// then yields zero hooks for the file. `Ok(None)` is a per-handler error —
/// an unknown `type` (`HookError::UnsupportedHandlerType`) or a `command`
/// handler with no `command` (`HookError::InvalidConfig`) — which upstream
/// collects inside the loop, so the file's other hooks still register.
/// `http` handlers are well-formed but carry no command, and HK's HookEntry
/// is command-only, so they take the same `Ok(None)` path.
fn handler_command(hook: &serde_json::Value) -> Result<Option<String>, ()> {
    let Some(handler_type) = hook.get("type").and_then(|v| v.as_str()) else {
        return Err(());
    };
    let optional_str = |key: &str| match hook.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(()),
    };
    match handler_type {
        "command" => optional_str("command"),
        "http" => optional_str("url").map(|_| None),
        _ => Ok(None),
    }
}

fn plugin_manifest_path(dir: &Path) -> Option<PathBuf> {
    for rel in ["plugin.json", ".grok-plugin/plugin.json", ".claude-plugin/plugin.json"]
    {
        let path = dir.join(rel);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Upstream plugin names are validated, never sanitized: lowercase ASCII,
/// digits, hyphens; no leading/trailing hyphen; 1..=64 chars
/// (`plugins/manifest.rs::is_valid_plugin_name`).
fn is_valid_grok_plugin_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Upstream `name_from_dirname`: ASCII-lowercase, every other char becomes
/// '-' (consecutive hyphens NOT collapsed), hyphens trimmed at both ends,
/// then reject empty or >64 — over-length rejects, never truncates.
fn grok_name_from_dirname(dir: &Path) -> Option<String> {
    let dirname = dir.file_name()?.to_str()?;
    let sanitized: String = dirname
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    is_valid_grok_plugin_name(trimmed).then(|| trimmed.to_string())
}

fn is_convention_plugin(dir: &Path) -> bool {
    dir.join("skills").is_dir()
        || dir.join("commands").is_dir()
        || dir.join("agents").is_dir()
        || dir.join(".mcp.json").is_file()
        || dir.join(".lsp.json").is_file()
        || dir.join("hooks").join("hooks.json").is_file()
}

/// The plugin name when `dir` is a plugin Grok would load, else None.
/// Mirrors upstream `collect_plugin`: a manifest that fails to parse or
/// carries an invalid name rejects the directory outright (no dirname
/// fallback); without a manifest the sanitized dirname must survive AND
/// at least one plugin component must exist (name check first, like
/// upstream).
fn grok_plugin_identity(dir: &Path) -> Option<String> {
    if let Some(manifest) = plugin_manifest_path(dir) {
        let content = std::fs::read_to_string(manifest).ok()?;
        let v: serde_json::Value = serde_json::from_str(&content).ok()?;
        let name = v.get("name")?.as_str()?;
        return is_valid_grok_plugin_name(name).then(|| name.to_string());
    }
    let name = grok_name_from_dirname(dir)?;
    is_convention_plugin(dir).then_some(name)
}

pub struct GrokAdapter {
    grok_home: PathBuf,
    /// `~/.agents` — the vendor-neutral config dir Grok always reads skills
    /// from. Independent of `$GROK_HOME` (upstream uses `dirs::home_dir`).
    agents_home: PathBuf,
}

impl Default for GrokAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GrokAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        Self {
            grok_home: resolve_grok_home(std::env::var_os("GROK_HOME"), Some(&home))
                .unwrap_or_else(|| home.join(".grok")),
            agents_home: home.join(".agents"),
        }
    }

    /// Test/deployer constructor: `<home>/.grok`. Does not read `$GROK_HOME`.
    pub fn with_home(home: PathBuf) -> Self {
        Self {
            grok_home: home.join(".grok"),
            agents_home: home.join(".agents"),
        }
    }

    /// Test constructor for a verbatim `$GROK_HOME` override. The `.agents`
    /// home binds the REAL one, like upstream — `$GROK_HOME` does not move
    /// it — so don't scan skills through an adapter built this way in tests.
    pub fn with_grok_home(grok_home: PathBuf) -> Self {
        Self {
            grok_home,
            agents_home: dirs::home_dir().unwrap_or_default().join(".agents"),
        }
    }

    fn parse_json(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn read_toml(path: &Path) -> Option<toml::Table> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|c| c.parse().ok())
    }

    fn disabled_mcp_names(&self) -> HashSet<String> {
        Self::read_toml(&self.mcp_config_path())
            .map(|doc| toml_string_list(&doc, "disabled_mcp_servers").into_iter().collect())
            .unwrap_or_default()
    }

    fn plugin_lists_from(&self, path: &Path) -> (HashSet<String>, HashSet<String>) {
        let Some(doc) = Self::read_toml(path) else {
            return (HashSet::new(), HashSet::new());
        };
        let Some(plugins) = doc.get("plugins").and_then(|v| v.as_table()) else {
            return (HashSet::new(), HashSet::new());
        };
        let list = |key: &str| -> HashSet<String> {
            plugins
                .get(key)
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        };
        (list("enabled"), list("disabled"))
    }

    fn plugin_is_enabled(&self, id: &str, name: &str, extra_config: Option<&Path>) -> bool {
        let (mut enabled, mut disabled) = self.plugin_lists_from(&self.plugin_config_path());
        if let Some(extra) = extra_config {
            let (more_enabled, more_disabled) = self.plugin_lists_from(extra);
            enabled.extend(more_enabled);
            disabled.extend(more_disabled);
        }
        let listed = |set: &HashSet<String>| set.contains(id) || set.contains(name);
        listed(&enabled) && !listed(&disabled)
    }

    fn parse_mcp_entry(
        name: &str,
        val: &toml::Value,
        disabled: &HashSet<String>,
    ) -> McpServerEntry {
        let table = val.as_table();
        let canonical_name = table
            .and_then(|t| t.get("_hk_name"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| name.to_string());
        let url = table
            .and_then(|t| t.get("url"))
            .and_then(|v| v.as_str())
            .map(String::from);
        // Mirrors upstream to_acp_mcp_server (config-types mcp.rs:468-481):
        // `type = "sse"` (ASCII case-insensitive) OR a byte-exact "/sse"
        // url suffix means SSE, joined by `||` — the suffix wins even over
        // an explicit `type = "http"`. "/sse/", "/sse?x=1" and "/SSE" stay
        // HTTP; do not "fix" any of this, it must match Grok byte-for-byte.
        let transport = if let Some(url_str) = url.as_deref() {
            let ty = table.and_then(|t| t.get("type")).and_then(|v| v.as_str());
            if ty.is_some_and(|t| t.eq_ignore_ascii_case("sse")) || url_str.ends_with("/sse") {
                McpTransport::Sse
            } else {
                McpTransport::Http
            }
        } else {
            McpTransport::Stdio
        };
        let native_enabled = table
            .and_then(|t| t.get("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        McpServerEntry {
            command: table
                .and_then(|t| t.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            args: table
                .and_then(|t| t.get("args"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            env: toml_string_map(table, "env"),
            transport,
            url,
            headers: toml_string_map(table, "headers"),
            enabled: native_enabled
                && !disabled.contains(name)
                && !disabled.contains(&canonical_name),
            name: canonical_name,
        }
    }

    fn hook_prefix_for(&self, path: &Path) -> &'static str {
        // Only `$GROK_HOME/hooks` is global. A project living under a custom
        // `$GROK_HOME` (e.g. `/work/repo` when `GROK_HOME=/work`) must stay
        // `project/` — `starts_with($GROK_HOME)` would mislabel it.
        if path.starts_with(self.grok_home.join("hooks")) {
            "global/"
        } else {
            "project/"
        }
    }

    fn disabled_hooks_path(&self) -> PathBuf {
        self.grok_home.join("disabled-hooks")
    }

    /// Recompute Grok's real spec.name for a command hook so toggle can write
    /// `$GROK_HOME/disabled-hooks` without extending `HookEntry`.
    pub fn hook_spec_name_for(
        &self,
        source_path: &Path,
        event: &str,
        matcher: Option<&str>,
        command: &str,
    ) -> Option<String> {
        self.command_hook_specs(source_path)
            .into_iter()
            .find(|(_, hook)| {
                hook.event == event
                    && hook.matcher.as_deref() == matcher
                    && hook.command == command
            })
            .map(|(name, _)| name)
    }

    fn command_hook_specs(&self, path: &Path) -> Vec<(String, HookEntry)> {
        let Some(mut config) = Self::parse_json(path) else {
            return vec![];
        };
        let Some(serde_json::Value::Object(hooks)) =
            config.as_object_mut().and_then(|o| o.remove("hooks"))
        else {
            return vec![];
        };
        let prefix = self.hook_prefix_for(path);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let disabled = read_disabled_hook_names(&self.disabled_hooks_path());

        // Bucket matcher groups by event VARIANT — upstream keys its map by
        // enum variant (xai-grok-hooks/src/config.rs:31), so SubagentStop and
        // SubagentEnd carry independent [i] counters even though both render
        // the `subagent_stop` display token.
        let mut by_variant: HashMap<&'static str, Vec<serde_json::Value>> = HashMap::new();
        for (key, val) in hooks {
            let Some(meta) = grok_event_by_alias(&key) else {
                continue;
            };
            if let serde_json::Value::Array(groups) = val
                && !groups.is_empty()
            {
                by_variant.entry(meta.canonical).or_default().extend(groups);
            }
        }

        let mut out: Vec<(String, HookEntry)> = Vec::new();
        // Upstream iterates variants in enum-declaration order
        // (`events.sort_by_key` on the derived Ord); GROK_EVENTS mirrors
        // that table order, which also makes the dedup below keep the
        // SubagentStop copy over the SubagentEnd one, like upstream.
        for event in GROK_EVENTS {
            let Some(groups) = by_variant.get(event.canonical) else {
                continue;
            };
            for (group_idx, group) in groups.iter().enumerate() {
                let matcher = group
                    .get("matcher")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                // `MatcherGroup::hooks` is a plain Vec: a group without a
                // `hooks` array fails upstream's deserialize, and
                // GroupErrorPolicy::Fail drops the whole file's hooks.
                let Some(handlers) = group.get("hooks").and_then(|v| v.as_array()) else {
                    return vec![];
                };
                // `hook_idx` enumerates every handler, valid or not: upstream
                // pushes a per-handler error and moves on, so a rejected
                // handler still consumes its index and its siblings keep the
                // spec names Grok generates for them.
                for (hook_idx, handler) in handlers.iter().enumerate() {
                    let command = match handler_command(handler) {
                        Err(()) => return vec![],
                        Ok(None) => continue,
                        Ok(Some(command)) => command,
                    };
                    let spec = format!(
                        "{prefix}{stem}:{}[{group_idx}].hooks[{hook_idx}]",
                        event.display
                    );
                    let enabled = !disabled.contains(&spec);
                    out.push((
                        spec,
                        HookEntry {
                            event: event.canonical.to_string(),
                            matcher: matcher.clone(),
                            command,
                            enabled,
                        },
                    ));
                }
            }
        }
        // Upstream dedups specs on (canonical event, raw command, matcher),
        // first wins (discovery.rs:218-231), with SubagentEnd folding into
        // SubagentStop — a hook hedged under both spellings runs once, so
        // it must be one row here too. Cross-file dedup (same content in
        // two files → upstream keeps only the first source) is not modeled.
        let mut seen: HashSet<(String, String, Option<String>)> = HashSet::new();
        out.retain(|(_, hook)| {
            seen.insert((
                dedup_event(&hook.event).to_string(),
                hook.command.clone(),
                hook.matcher.clone(),
            ))
        });
        out
    }

    fn scan_plugin_dir(
        &self,
        dir: &Path,
        scope: &str,
        source: &str,
        extra_config: Option<&Path>,
    ) -> Vec<PluginEntry> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return vec![];
        };
        let mut plugins = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = grok_plugin_identity(&path) else {
                continue;
            };
            let id = grok_plugin_id(scope, &path, &name);
            let enabled = self.plugin_is_enabled(&id, &name, extra_config);
            plugins.push(PluginEntry {
                name,
                source: source.to_string(),
                enabled,
                path: Some(path),
                source_url: None,
                uri: Some(id),
                installed_at: None,
                updated_at: None,
                base_layers: vec![],
                pack: None,
            });
        }
        plugins
    }
}

impl AgentAdapter for GrokAdapter {
    fn name(&self) -> &str {
        "grok"
    }

    fn base_dir(&self) -> PathBuf {
        self.grok_home.clone()
    }

    fn detect(&self) -> bool {
        self.grok_home.exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        // `.grok` and `.agents` are the two config dirs Grok always reads
        // skills from — `CompatConfig::skill_config_dirs` hard-codes both and
        // gates only `.claude`/`.cursor` behind their compat cells, and the
        // global pass adds `~/.agents` unconditionally beside grok_home
        // (compat.rs skill_config_dirs, prompt/skills.rs). The vendor dirs
        // stay with the claude/cursor adapters. Own dir first: it is the
        // install target via `skill_dir_for`.
        vec![
            self.grok_home.join("skills"),
            self.agents_home.join("skills"),
        ]
    }

    fn expand_skill_roots(&self, root: &Path) -> Vec<PathBuf> {
        // Grok discovers skills RECURSIVELY under each skills root
        // (walk_for_skill_md, depth cap 5), so nested layouts like
        // skills/team/infra/SKILL.md are real skills. Surface them by
        // returning every nested parent dir for the flat scanner.
        let mut roots = vec![root.to_path_buf()];
        collect_nested_skill_parents(root, 1, &mut roots);
        roots
    }

    fn standalone_md_skills(&self) -> bool {
        // `walk_for_skill_md` filters `read_dir` to directories before it ever
        // looks at a filename, and the only name it accepts is `SKILL.md`
        // inside one (skills/discovery.rs:127-146). A bare `notes.md` under a
        // skills root is invisible to Grok — the one place a loose `.md`
        // becomes a Grok entity is `commands/`, a different root that HK
        // already lists as a config file. Verified against grok 1.0.5.
        false
    }

    fn mcp_config_path(&self) -> PathBuf {
        self.grok_home.join("config.toml")
    }

    fn hook_config_path(&self) -> PathBuf {
        self.grok_home.join("hooks").join("harnesskit.json")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.grok_home.join("plugins"),
            self.grok_home.join("installed-plugins"),
        ]
    }

    fn plugin_config_path(&self) -> PathBuf {
        self.grok_home.join("config.toml")
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::GrokToml
    }

    fn remote_mcp_schema(&self) -> RemoteMcpSchema {
        RemoteMcpSchema::GrokToml
    }

    fn supports_native_mcp_toggle(&self) -> bool {
        true
    }

    fn hook_format(&self) -> HookFormat {
        HookFormat::ClaudeLike
    }

    fn translate_hook_event(&self, event: &str) -> Option<String> {
        super::hook_events::to_grok(event)
    }

    fn hook_config_paths_for(&self, scope: &ConfigScope) -> Vec<PathBuf> {
        let dir = match scope {
            ConfigScope::Global => self.grok_home.join("hooks"),
            ConfigScope::Project { path, .. } => Path::new(path).join(".grok").join("hooks"),
        };
        // Upstream's is_direct_hook_json_name skips dotfiles.
        files_with_ext(&dir, "json")
            .filter(|p| {
                p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| !n.starts_with('.'))
            })
            .collect()
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Some(doc) = Self::read_toml(path) else {
            return vec![];
        };
        let Some(servers) = doc.get("mcp_servers").and_then(|v| v.as_table()) else {
            return vec![];
        };
        // Personal disable lives on the user file and applies to project
        // entries too — Grok's `disabled_mcp_servers` is user-tier.
        let disabled = self.disabled_mcp_names();
        servers
            .iter()
            .map(|(name, val)| Self::parse_mcp_entry(name, val, &disabled))
            .collect()
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        self.hook_config_paths_for(&ConfigScope::Global)
            .into_iter()
            .flat_map(|path| self.read_hooks_from(&path))
            .collect()
    }

    fn read_hooks_from(&self, path: &Path) -> Vec<HookEntry> {
        self.command_hook_specs(path)
            .into_iter()
            .map(|(_, hook)| hook)
            .collect()
    }

    fn read_plugins(&self) -> Vec<PluginEntry> {
        // Both dirs are user scope; the source label distinguishes a
        // hand-placed plugin from a marketplace install.
        self.plugin_dirs()
            .iter()
            .zip(["user", "installed"])
            .flat_map(|(dir, source)| self.scan_plugin_dir(dir, "user", source, None))
            .collect()
    }

    fn read_plugins_from(&self, dir: &Path) -> Vec<PluginEntry> {
        // `<project>/.grok/plugins` → sibling `config.toml` is the project list.
        let extra = dir.parent().map(|parent| parent.join("config.toml"));
        self.scan_plugin_dir(dir, "project", "project", extra.as_deref())
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.grok_home.join("AGENTS.md")];
        files.extend(files_with_ext(&self.grok_home.join("rules"), "md"));
        files
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        // User-editable config only. Deliberately excluded:
        // managed_config.toml + requirements.toml (server-synced, atomically
        // overwritten per fetch and deleted on logout — surfacing them
        // invites edits that silently vanish), /etc/grok/* (machine-admin),
        // and state/cache/secret files at the root (auth.json,
        // mcp_credentials.json, mcp_preferences.json, trusted_folders.toml,
        // campaigns_state.json, managed_config_cache.json, *.sig.json).
        vec![
            self.grok_home.join("config.toml"),
            self.grok_home.join("pager.toml"),
            self.grok_home.join("sandbox.toml"),
            self.grok_home.join("lsp.json"),
        ]
    }

    fn global_subagent_files(&self) -> Vec<PathBuf> {
        files_with_ext(&self.grok_home.join("agents"), "md").collect()
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        // The global MEMORY.md plus each subagent's MEMORY.md. Grok never
        // reads flat non-MEMORY *.md at the memory/ top level, and the
        // {slug}-{hash8}/ workspace subdirs hold session transcripts and
        // index.sqlite — state, not user-editable memory.
        let mut files = vec![self.grok_home.join("memory").join("MEMORY.md")];
        if let Ok(entries) = std::fs::read_dir(self.grok_home.join("agent-memory")) {
            for entry in entries.flatten() {
                let memory_md = entry.path().join("MEMORY.md");
                if memory_md.is_file() {
                    files.push(memory_md);
                }
            }
        }
        files
    }

    fn global_workflow_files(&self) -> Vec<PathBuf> {
        files_with_ext(&self.grok_home.join("commands"), "md").collect()
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        vec![ProjectMarker::Dir(".grok")]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        vec![".grok/skills".into()]
    }

    fn project_skill_read_dirs(&self) -> Vec<String> {
        // Grok always reads `.agents/skills`; write stays on `.grok/skills`
        // so Codex/Gemini keep ownership of the shared alias.
        vec![".agents/skills".into()]
    }

    fn project_mcp_config_relpath(&self) -> Option<String> {
        Some(".grok/config.toml".into())
    }

    fn project_hook_config_relpath(&self) -> Option<String> {
        Some(".grok/hooks/harnesskit.json".into())
    }

    fn project_plugin_dirs(&self) -> Vec<String> {
        vec![".grok/plugins".into()]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            "AGENTS.md".into(),
            "Agents.md".into(),
            "AGENT.md".into(),
            ".grok/rules/*.md".into(),
        ]
    }

    fn project_memory_patterns(&self) -> Vec<String> {
        // Grok has no <project>/.grok/memory — project memory is per-subagent
        // (MemoryScope in xai-grok-agent config.rs). agent-memory-local is
        // the personal/uncommitted variant. Only MEMORY.md is the prompt
        // contract; sessions/*.md and index.sqlite are state. All-glob on
        // purpose: no unique concrete pattern means no Kit memory write
        // target, since HK cannot invent a subagent name.
        vec![
            ".grok/agent-memory/*/MEMORY.md".into(),
            ".grok/agent-memory-local/*/MEMORY.md".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        // Project-scope counterparts of the global list: sandbox profiles
        // (profiles.rs also reads .grok/sandbox.toml) and LSP config.
        vec![
            ".grok/config.toml".into(),
            ".grok/sandbox.toml".into(),
            ".grok/lsp.json".into(),
        ]
    }

    fn project_subagent_patterns(&self) -> Vec<String> {
        vec![".grok/agents/*.md".into()]
    }

    fn project_workflow_patterns(&self) -> Vec<String> {
        vec![".grok/commands/*.md".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn resolve_grok_home_env_wins_verbatim() {
        let resolved = resolve_grok_home(
            Some(OsString::from("/custom/grok")),
            Some(Path::new("/home/u")),
        );
        assert_eq!(resolved, Some(PathBuf::from("/custom/grok")));
    }

    #[test]
    fn resolve_grok_home_empty_env_falls_through() {
        let resolved = resolve_grok_home(Some(OsString::new()), Some(Path::new("/home/u")));
        assert_eq!(resolved, Some(PathBuf::from("/home/u/.grok")));
    }

    #[test]
    fn with_home_does_not_follow_process_grok_home() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        assert_eq!(adapter.base_dir(), tmp.path().join(".grok"));
        assert!(!adapter.detect());
        fs::create_dir_all(adapter.base_dir()).unwrap();
        assert!(adapter.detect());
    }

    #[test]
    fn with_grok_home_uses_override_root() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("override");
        fs::create_dir_all(&root).unwrap();
        let adapter = GrokAdapter::with_grok_home(root.clone());
        assert_eq!(adapter.base_dir(), root);
        assert!(adapter.detect());
        assert_eq!(adapter.mcp_config_path(), root.join("config.toml"));
        assert_eq!(
            adapter.hook_config_path(),
            root.join("hooks/harnesskit.json")
        );
    }

    #[test]
    fn skill_and_project_paths() {
        let adapter = GrokAdapter::with_home(PathBuf::from("/tmp/hk-grok"));
        assert_eq!(
            adapter.skill_dirs(),
            vec![
                PathBuf::from("/tmp/hk-grok/.grok/skills"),
                PathBuf::from("/tmp/hk-grok/.agents/skills"),
            ],
            "Grok always reads both config dirs; own dir first is the install target"
        );
        assert_eq!(adapter.project_skill_dirs(), vec![".grok/skills".to_string()]);
        assert_eq!(
            adapter.project_skill_read_dirs(),
            vec![".agents/skills".to_string()]
        );
        assert_eq!(
            adapter.project_mcp_config_relpath().as_deref(),
            Some(".grok/config.toml")
        );
        assert_eq!(
            adapter.project_hook_config_relpath().as_deref(),
            Some(".grok/hooks/harnesskit.json")
        );
        assert_eq!(adapter.project_plugin_dirs(), vec![".grok/plugins".to_string()]);
    }

    #[test]
    fn reads_stdio_http_and_sse_mcp() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        write(
            &adapter.mcp_config_path(),
            r#"
[mcp_servers.local]
command = "npx"
args = ["-y", "srv"]
cwd = "/tmp/work"

[mcp_servers.http]
url = "https://example.com/mcp"
headers = { Authorization = "Bearer t" }
startup_timeout_sec = 30

[mcp_servers.sse]
url = "https://example.com/sse"
type = "sse"
enabled = false
"#,
        );
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 3);
        let local = servers.iter().find(|s| s.name == "local").unwrap();
        assert_eq!(local.transport, McpTransport::Stdio);
        assert!(local.enabled);
        let http = servers.iter().find(|s| s.name == "http").unwrap();
        assert_eq!(http.transport, McpTransport::Http);
        assert_eq!(http.headers["Authorization"], "Bearer t");
        let sse = servers.iter().find(|s| s.name == "sse").unwrap();
        assert_eq!(sse.transport, McpTransport::Sse);
        assert!(!sse.enabled);
    }

    #[test]
    fn mcp_disabled_list_applies_to_user_and_project() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        write(
            &adapter.mcp_config_path(),
            r#"
disabled_mcp_servers = ["shared"]

[mcp_servers.shared]
command = "echo"
"#,
        );
        let project = tmp.path().join("proj/.grok/config.toml");
        write(
            &project,
            r#"
[mcp_servers.shared]
command = "echo"
enabled = true
"#,
        );
        assert!(!adapter.read_mcp_servers()[0].enabled);
        assert!(!adapter.read_mcp_servers_from(&project)[0].enabled);
    }

    #[test]
    fn hooks_scan_command_only_and_compute_spec_name() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        let hook_file = adapter.base_dir().join("hooks/session-start.json");
        write(
            &hook_file,
            r#"{
              "hooks": {
                "PreToolUse": [{
                  "matcher": "Bash",
                  "hooks": [
                    {"type": "http", "url": "https://example.com/hook"},
                    {"type": "command", "command": "echo hi"}
                  ]
                }]
              }
            }"#,
        );
        write(
            &adapter.disabled_hooks_path(),
            "# comment\nglobal/session-start:pre_tool_use[0].hooks[1]\n",
        );
        let hooks = adapter.read_hooks_from(&hook_file);
        assert_eq!(hooks.len(), 1, "HTTP handlers must be skipped");
        assert_eq!(hooks[0].event, "PreToolUse");
        assert_eq!(hooks[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(hooks[0].command, "echo hi");
        assert!(!hooks[0].enabled);
        assert_eq!(
            adapter
                .hook_spec_name_for(&hook_file, "PreToolUse", Some("Bash"), "echo hi")
                .as_deref(),
            Some("global/session-start:pre_tool_use[0].hooks[1]")
        );
    }

    #[test]
    fn hook_prefix_does_not_treat_nested_project_as_global() {
        let tmp = TempDir::new().unwrap();
        let grok_home = tmp.path().join("work");
        let adapter = GrokAdapter::with_grok_home(grok_home.clone());
        let global = grok_home.join("hooks/session-start.json");
        write(
            &global,
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo global"}]}]}}"#,
        );
        let nested = grok_home.join("repo/.grok/hooks/safety.json");
        write(
            &nested,
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo project"}]}]}}"#,
        );
        assert_eq!(
            adapter
                .hook_spec_name_for(&global, "Stop", None, "echo global")
                .as_deref(),
            Some("global/session-start:stop[0].hooks[0]")
        );
        assert_eq!(
            adapter
                .hook_spec_name_for(&nested, "Stop", None, "echo project")
                .as_deref(),
            Some("project/safety:stop[0].hooks[0]")
        );
    }

    #[test]
    fn subagent_variants_keep_independent_indices() {
        // Upstream keys spec indices by enum variant (config.rs:31), and both
        // variants declare display "subagent_stop" in the hook_events! table,
        // so each starts at [0] — the names collide by design upstream, where
        // one disabled-hooks line then covers both.
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        let hook_file = adapter.base_dir().join("hooks/agents.json");
        write(
            &hook_file,
            r#"{
              "hooks": {
                "SubagentStop": [{"hooks":[{"type":"command","command":"echo stop"}]}],
                "SubagentEnd": [{"hooks":[{"type":"command","command":"echo end"}]}]
              }
            }"#,
        );
        let hooks = adapter.read_hooks_from(&hook_file);
        assert_eq!(hooks.len(), 2, "different commands are distinct hooks");
        assert_eq!(
            adapter
                .hook_spec_name_for(&hook_file, "SubagentStop", None, "echo stop")
                .as_deref(),
            Some("global/agents:subagent_stop[0].hooks[0]"),
            "each variant counts from [0], like upstream"
        );
        assert_eq!(
            adapter
                .hook_spec_name_for(&hook_file, "SubagentEnd", None, "echo end")
                .as_deref(),
            Some("global/agents:subagent_stop[0].hooks[0]")
        );
    }

    #[test]
    fn identical_hook_under_both_subagent_spellings_dedups_to_one_row() {
        // Upstream dedups on (canonical event, raw command, matcher) with
        // SubagentEnd folding into SubagentStop (discovery.rs test
        // `deduplicates_hooks_across_alias_spellings`), so a hook hedged
        // under both spellings runs once and must be one row here.
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        let hook_file = adapter.base_dir().join("hooks/agents.json");
        write(
            &hook_file,
            r#"{
              "hooks": {
                "SubagentStop": [{"hooks":[{"type":"command","command":"notify.sh"}]}],
                "SubagentEnd": [{"hooks":[{"type":"command","command":"notify.sh"}]}]
              }
            }"#,
        );
        let hooks = adapter.read_hooks_from(&hook_file);
        assert_eq!(hooks.len(), 1, "alias hedge must not double-register");
        assert_eq!(hooks[0].event, "SubagentStop", "first (table-order) copy wins");
        assert_eq!(
            adapter
                .hook_spec_name_for(&hook_file, "SubagentStop", None, "notify.sh")
                .as_deref(),
            Some("global/agents:subagent_stop[0].hooks[0]")
        );
    }

    #[test]
    fn handler_errors_are_per_handler_but_type_errors_take_the_file() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());

        // An unknown `type` deserializes fine and errors inside upstream's
        // loop (HookError::UnsupportedHandlerType), as does a `command`
        // handler with no `command` (HookError::InvalidConfig) — the file's
        // other hooks still register, and the rejected handlers keep their
        // index so the survivor's spec name is the one Grok generates.
        let per_handler = adapter.base_dir().join("hooks/per-handler.json");
        write(
            &per_handler,
            r#"{
              "hooks": {
                "Stop": [{"hooks":[
                  {"type":"webhook","command":"nope.sh"},
                  {"type":"command"},
                  {"type":"command","command":"ok.sh"}
                ]}]
              }
            }"#,
        );
        let commands: Vec<String> = adapter
            .read_hooks_from(&per_handler)
            .into_iter()
            .map(|h| h.command)
            .collect();
        assert_eq!(commands, vec!["ok.sh"]);
        assert_eq!(
            adapter.hook_spec_name_for(&per_handler, "Stop", None, "ok.sh"),
            Some("global/per-handler:stop[0].hooks[2]".to_string()),
            "a rejected handler still consumes its index"
        );

        // A missing `type` is a serde error on RawHandler's plain String
        // field, and GroupErrorPolicy::Fail then yields zero hooks for the
        // whole file — listing the valid sibling would be a phantom row.
        let file_level = adapter.base_dir().join("hooks/file-level.json");
        write(
            &file_level,
            r#"{
              "hooks": {
                "Stop": [{"hooks":[
                  {"type":"command","command":"ok.sh"},
                  {"command":"no-type.sh"}
                ]}]
              }
            }"#,
        );
        assert!(adapter.read_hooks_from(&file_level).is_empty());
    }

    #[test]
    fn cursor_style_alias_keys_merge_into_one_event() {
        // Cursor's per-operation names map onto PreToolUse (docs 10-hooks.md
        // "Cursor Hook Compatibility"), so both land in one index space —
        // upstream merges them in HashMap order, which is why the index for
        // such a file is best-effort (see the module header).
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        let hook_file = adapter.base_dir().join("hooks/cursor-import.json");
        write(
            &hook_file,
            r#"{
              "hooks": {
                "beforeShellExecution": [{"hooks":[{"type":"command","command":"guard.sh"}]}],
                "beforeReadFile": [{"hooks":[{"type":"command","command":"scan.sh"}]}],
                "Stop": [{"hooks":[{"type":"command","command":"done.sh"}]}]
              }
            }"#,
        );
        let hooks = adapter.read_hooks_from(&hook_file);
        assert_eq!(hooks.len(), 3);
        assert_eq!(
            hooks.iter().filter(|h| h.event == "PreToolUse").count(),
            2,
            "both Cursor spellings resolve to PreToolUse"
        );
        assert_eq!(
            adapter
                .hook_spec_name_for(&hook_file, "Stop", None, "done.sh")
                .as_deref(),
            Some("global/cursor-import:stop[0].hooks[0]")
        );
    }

    #[test]
    fn project_hook_spec_uses_project_prefix() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        let hook_file = tmp.path().join("repo/.grok/hooks/safety.json");
        write(
            &hook_file,
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"notify"}]}]}}"#,
        );
        assert_eq!(
            adapter
                .hook_spec_name_for(&hook_file, "Stop", None, "notify")
                .as_deref(),
            Some("project/safety:stop[0].hooks[0]")
        );
    }

    #[test]
    fn plugin_stable_id_and_default_disabled() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        let plugin = adapter.base_dir().join("plugins/my-tool");
        write(&plugin.join("plugin.json"), r#"{"name":"my-tool"}"#);
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 1);
        assert!(!plugins[0].enabled, "user plugins default to disabled");
        let id = plugins[0].uri.as_deref().unwrap();
        assert!(id.starts_with("user/"));
        assert!(id.ends_with("/my-tool"));
        assert_eq!(id, grok_plugin_id("user", &plugin, "my-tool"));
    }

    #[test]
    fn plugin_enabled_list_and_disabled_wins() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        let plugin = adapter.base_dir().join("plugins/my-tool");
        write(&plugin.join("skills/.keep"), "");
        let id = grok_plugin_id("user", &plugin, "my-tool");
        write(
            &adapter.plugin_config_path(),
            &format!(
                "[plugins]\nenabled = [\"{id}\"]\ndisabled = [\"{id}\"]\n"
            ),
        );
        let plugins = adapter.read_plugins();
        assert!(!plugins[0].enabled, "disabled list takes precedence");

        write(
            &adapter.plugin_config_path(),
            &format!("[plugins]\nenabled = [\"{id}\"]\n"),
        );
        let plugins = adapter.read_plugins();
        assert!(plugins[0].enabled);
    }

    #[test]
    fn project_plugin_is_discovered_with_project_id() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        let plugin = tmp.path().join("repo/.grok/plugins/team-tool");
        write(&plugin.join("plugin.json"), r#"{"name":"team-tool"}"#);
        let plugins = adapter.read_plugins_from(plugin.parent().unwrap());
        assert_eq!(plugins.len(), 1);
        assert!(!plugins[0].enabled, "project plugins default to disabled");
        let id = plugins[0].uri.as_deref().unwrap();
        assert!(id.starts_with("project/"));
        assert!(id.ends_with("/team-tool"));
        assert!(adapter.read_plugins().is_empty());
    }

    #[test]
    fn settings_do_not_include_auth_files() {
        let adapter = GrokAdapter::with_home(PathBuf::from("/tmp/hk-grok"));
        let settings = adapter.global_settings_files();
        assert!(settings.iter().all(|p| {
            let name = p.file_name().unwrap();
            name != "auth.json" && name != "mcp_credentials.json"
        }));
        assert!(settings.iter().any(|p| p.ends_with("config.toml")));
        assert!(settings.iter().any(|p| p.ends_with("pager.toml")));
        assert!(settings.iter().any(|p| p.ends_with("sandbox.toml")));
        assert!(settings.iter().any(|p| p.ends_with("lsp.json")));
        assert!(
            !settings
                .iter()
                .any(|p| p.ends_with("managed_config.toml") || p.ends_with("requirements.toml")),
            "server-synced artifacts are overwritten per fetch and must not be listed"
        );
    }

    #[test]
    fn mcp_sse_inferred_from_url_suffix_byte_exact() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        write(
            &adapter.mcp_config_path(),
            r#"
[mcp_servers.suffix]
url = "https://example.com/mcp/sse"
type = "http"

[mcp_servers.upper]
url = "https://example.com/SSE"

[mcp_servers.slash]
url = "https://example.com/sse/"

[mcp_servers.ty]
url = "https://example.com/x"
type = "SSE"
"#,
        );
        let servers = adapter.read_mcp_servers();
        let transport =
            |name: &str| servers.iter().find(|s| s.name == name).unwrap().transport;
        assert_eq!(transport("suffix"), McpTransport::Sse, "suffix wins over type=http");
        assert_eq!(transport("upper"), McpTransport::Http, "suffix is case-sensitive");
        assert_eq!(transport("slash"), McpTransport::Http, "trailing slash defeats it");
        assert_eq!(transport("ty"), McpTransport::Sse, "type match ignores case");
    }

    #[test]
    fn expand_skill_roots_walks_nested_and_caps_depth() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        let root = adapter.base_dir().join("skills");
        write(&root.join("flat/SKILL.md"), "flat");
        write(&root.join("team/infra/SKILL.md"), "nested");
        // A skill nested INSIDE another skill — both are real upstream.
        write(&root.join("team/infra/child/SKILL.md"), "inner");
        // Deeper than upstream's walk cap (parent depth > 5) — not a root.
        write(&root.join("a/b/c/d/e/f/too-deep/SKILL.md"), "deep");

        let roots = adapter.expand_skill_roots(&root);
        assert!(roots.contains(&root), "canonical root stays first");
        assert_eq!(roots[0], root);
        assert!(roots.contains(&root.join("team")), "nested parent found");
        assert!(
            roots.contains(&root.join("team/infra")),
            "skill dirs can hold nested skills"
        );
        assert!(
            !roots.iter().any(|r| r.ends_with("f")),
            "depth cap mirrors upstream MAX_SKILL_WALK_DEPTH"
        );
    }

    #[test]
    fn plugin_identity_mirrors_upstream_rules() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        let plugins_dir = adapter.base_dir().join("plugins");

        // Convention plugin via commands/ only, dirname sanitized like
        // upstream (consecutive hyphens are NOT collapsed).
        write(&plugins_dir.join("My__Tool/commands/run.md"), "cmd");
        // Convention plugin via .lsp.json only.
        write(&plugins_dir.join("lsp-only/.lsp.json"), "{}");
        // Broken manifest rejects the dir outright — no dirname fallback.
        write(&plugins_dir.join("broken/plugin.json"), "{not json");
        // Manifest with an invalid (uppercase) name is rejected, not sanitized.
        write(&plugins_dir.join("badname/plugin.json"), r#"{"name":"BadName"}"#);
        // Unsalvageable dirname is rejected even with components present.
        write(&plugins_dir.join("---/skills/.keep"), "");

        let mut names: Vec<String> =
            adapter.read_plugins().into_iter().map(|p| p.name).collect();
        names.sort();
        assert_eq!(names, vec!["lsp-only", "my--tool"]);
    }

    #[test]
    fn rules_subagents_memory_and_commands() {
        let tmp = TempDir::new().unwrap();
        let adapter = GrokAdapter::with_home(tmp.path().to_path_buf());
        write(&adapter.base_dir().join("AGENTS.md"), "hi");
        write(&adapter.base_dir().join("rules/style.md"), "style");
        write(&adapter.base_dir().join("agents/reviewer.md"), "agent");
        write(&adapter.base_dir().join("memory/MEMORY.md"), "mem");
        write(&adapter.base_dir().join("memory/notes.md"), "note");
        write(&adapter.base_dir().join("agent-memory/reviewer/MEMORY.md"), "am");
        write(&adapter.base_dir().join("commands/ship.md"), "cmd");

        assert!(adapter
            .global_rules_files()
            .iter()
            .any(|p| p.ends_with("AGENTS.md")));
        assert_eq!(adapter.global_subagent_files().len(), 1);
        let memory = adapter.global_memory_files();
        assert!(memory.iter().any(|p| p.ends_with("memory/MEMORY.md")));
        assert!(
            memory
                .iter()
                .any(|p| p.ends_with("agent-memory/reviewer/MEMORY.md")),
            "subagent memory files are claimed"
        );
        assert!(
            !memory.iter().any(|p| p.ends_with("notes.md")),
            "Grok never reads flat non-MEMORY *.md at the memory root"
        );
        assert_eq!(adapter.global_workflow_files().len(), 1);
        assert!(!adapter
            .project_rules_patterns()
            .iter()
            .any(|p| p.contains("CLAUDE.md")));
    }

    #[test]
    fn adapter_declarations() {
        let adapter = GrokAdapter::with_home(PathBuf::from("/tmp/hk-grok"));
        assert_eq!(adapter.name(), "grok");
        assert_eq!(adapter.mcp_format(), McpFormat::GrokToml);
        assert_eq!(adapter.remote_mcp_schema(), RemoteMcpSchema::GrokToml);
        assert!(adapter.supports_native_mcp_toggle());
        assert_eq!(adapter.hook_format(), HookFormat::ClaudeLike);
        assert!(!adapter.needs_path_injection());
        assert!(adapter.supports_global_hook_install());
    }
}
