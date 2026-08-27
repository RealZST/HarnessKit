// Grok Build (xAI) config references — verified against
// github.com/xai-org/grok-build @ c2ad97f:
// - Home: crates/codegen/xai-grok-home — `$GROK_HOME` verbatim when
//   non-empty, else `<home>/.grok`. Detection must not create the dir.
// - Skills: `$GROK_HOME/skills`, project `.grok/skills` (SKILL.md).
//   Compat trees (`.claude`, `.cursor`, `.agents`) are not claimed.
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
pub fn grok_plugin_id(scope: &str, root: &Path, name: &str) -> String {
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
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

fn is_convention_plugin(dir: &Path) -> bool {
    dir.join("skills").is_dir()
        || dir.join("agents").is_dir()
        || dir.join(".mcp.json").is_file()
        || dir.join("hooks").join("hooks.json").is_file()
}

fn plugin_name_from_dir(dir: &Path) -> String {
    if let Some(manifest) = plugin_manifest_path(dir)
        && let Ok(content) = std::fs::read_to_string(manifest)
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(name) = v.get("name").and_then(|n| n.as_str()).filter(|s| !s.is_empty())
    {
        return name.to_string();
    }
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn is_grok_plugin_dir(dir: &Path) -> bool {
    dir.is_dir() && (plugin_manifest_path(dir).is_some() || is_convention_plugin(dir))
}

pub struct GrokAdapter {
    grok_home: PathBuf,
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
        }
    }

    /// Test/deployer constructor: `<home>/.grok`. Does not read `$GROK_HOME`.
    pub fn with_home(home: PathBuf) -> Self {
        Self {
            grok_home: home.join(".grok"),
        }
    }

    /// Test constructor for a verbatim `$GROK_HOME` override.
    pub fn with_grok_home(grok_home: PathBuf) -> Self {
        Self { grok_home }
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
        let transport = if url.is_some() {
            match table
                .and_then(|t| t.get("type"))
                .and_then(|v| v.as_str())
            {
                Some("sse") => McpTransport::Sse,
                _ => McpTransport::Http,
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
            if !is_grok_plugin_dir(&path) {
                continue;
            }
            let name = plugin_name_from_dir(&path);
            if name.is_empty() {
                continue;
            }
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
        vec![self.grok_home.join("skills")]
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
        vec![
            self.grok_home.join("config.toml"),
            self.grok_home.join("pager.toml"),
        ]
    }

    fn global_subagent_files(&self) -> Vec<PathBuf> {
        files_with_ext(&self.grok_home.join("agents"), "md").collect()
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        let memory = self.grok_home.join("memory");
        let mut files = vec![memory.join("MEMORY.md")];
        files.extend(files_with_ext(&memory, "md").filter(|p| p.is_file()));
        files.sort();
        files.dedup();
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
        vec![
            ".grok/memory/MEMORY.md".into(),
            ".grok/memory/*.md".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".grok/config.toml".into()]
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
            vec![PathBuf::from("/tmp/hk-grok/.grok/skills")]
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
        write(&adapter.base_dir().join("memory/index/hash.md"), "nested");
        write(&adapter.base_dir().join("commands/ship.md"), "cmd");

        assert!(adapter
            .global_rules_files()
            .iter()
            .any(|p| p.ends_with("AGENTS.md")));
        assert_eq!(adapter.global_subagent_files().len(), 1);
        let memory = adapter.global_memory_files();
        assert!(memory.iter().any(|p| p.ends_with("MEMORY.md")));
        assert!(memory.iter().any(|p| p.ends_with("notes.md")));
        assert!(
            !memory.iter().any(|p| p.ends_with("hash.md")),
            "nested memory index dirs must not be claimed"
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
