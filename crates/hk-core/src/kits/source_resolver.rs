use crate::adapter::AgentAdapter;
use crate::models::{ConfigScope, ExtensionKind};
use crate::scanner;
use std::collections::HashMap;
use std::path::PathBuf;

/// Heuristic: keys whose lowercased name contains any of these is treated
/// as a secret. Conservative — false positives cost a one-time re-config
/// by the receiver; false negatives leak secrets.
const SECRET_KEY_FRAGMENTS: &[&str] =
    &["token", "secret", "key", "password", "passwd", "api_key", "apikey", "auth"];

/// Result of locating an extension's on-disk source.
#[derive(Debug, Clone)]
pub struct ExtensionLocation {
    /// For skills/CLI: the directory or file root.
    /// For MCP/Hook: a single config file path (the caller reads the entry
    /// by name from this file, using the adapter parser identified by `agent`).
    pub entry_path: PathBuf,
    /// Adapter that owns `entry_path`. Required for kinds whose parse logic
    /// depends on the source agent's format (MCP — JSON/TOML/OpenCode-schema;
    /// Hook — claude-like vs codex-like). `None` for Skill (file-format
    /// agnostic — pack just walks the directory).
    pub agent: Option<String>,
}

/// Locate the current on-disk source of an extension by kind.
///
/// `extension_id` is only matched for `Skill`. For Mcp/Hook/Cli the returned
/// path is the agent's config file — entry-by-name lookup happens later in
/// `service::embed_extension`.
pub fn find_extension_source_by_id(
    adapters: &[Box<dyn AgentAdapter>],
    extension_id: &str,
    kind: ExtensionKind,
    agents: &[String],
    scope: &ConfigScope,
    projects: &[(String, String)],
) -> Option<ExtensionLocation> {
    match kind {
        ExtensionKind::Skill => scanner::find_skill_by_id(adapters, extension_id, agents, projects)
            .map(|loc| ExtensionLocation {
                entry_path: loc.entry_path,
                agent: None,
            }),
        ExtensionKind::Mcp => find_mcp_source(adapters, agents, scope),
        ExtensionKind::Hook => find_hook_source(adapters, agents, scope),
        ExtensionKind::Cli => find_cli_source(adapters, agents, scope),
        ExtensionKind::Plugin => None, // not Kit-able in v1
    }
}

/// Return the first enrolled agent's MCP config file path for the given scope.
/// The actual matching of the server entry by stable id happens at pack/refresh time
/// (in `embed_extension`), which re-reads this file and looks up the entry by name.
fn find_mcp_source(
    adapters: &[Box<dyn AgentAdapter>],
    agents: &[String],
    scope: &ConfigScope,
) -> Option<ExtensionLocation> {
    for adapter in adapters.iter() {
        if !agents.iter().any(|a| a == adapter.name()) {
            continue;
        }
        if let Some(path) = adapter.mcp_config_path_for(scope)
            && path.exists()
        {
            return Some(ExtensionLocation {
                entry_path: path,
                agent: Some(adapter.name().to_string()),
            });
        }
    }
    None
}

fn find_hook_source(
    adapters: &[Box<dyn AgentAdapter>],
    agents: &[String],
    scope: &ConfigScope,
) -> Option<ExtensionLocation> {
    for adapter in adapters.iter() {
        if !agents.iter().any(|a| a == adapter.name()) {
            continue;
        }
        if let Some(path) = adapter.hook_config_path_for(scope)
            && path.exists()
        {
            return Some(ExtensionLocation {
                entry_path: path,
                agent: Some(adapter.name().to_string()),
            });
        }
    }
    None
}

fn find_cli_source(
    adapters: &[Box<dyn AgentAdapter>],
    agents: &[String],
    scope: &ConfigScope,
) -> Option<ExtensionLocation> {
    // CLI extensions live as files inside skill directories in HarnessKit's
    // model — there is no separate `cli_dirs()` on the adapter trait. Pick the
    // first existing skill directory of an enrolled agent for this scope.
    for adapter in adapters.iter() {
        if !agents.iter().any(|a| a == adapter.name()) {
            continue;
        }
        let dirs: Vec<PathBuf> = match scope {
            ConfigScope::Global => adapter.skill_dirs(),
            ConfigScope::Project { path, .. } => adapter
                .project_skill_dirs()
                .into_iter()
                .map(|rel| std::path::Path::new(path).join(rel))
                .collect(),
        };
        for dir in dirs {
            if dir.exists() {
                return Some(ExtensionLocation {
                    entry_path: dir,
                    agent: Some(adapter.name().to_string()),
                });
            }
        }
    }
    None
}

/// Blank values for env keys that look like secrets. Mutates in place;
/// returns `true` if any non-empty secret value was cleared.
///
/// Operates on `McpServerEntry.env` directly — the adapter parser normalizes
/// every source format (JSON `env`, JSON `environment`, TOML `env`, OpenCode
/// `environment`) into this single HashMap, so this helper doesn't need to
/// know about source-side field naming.
pub fn strip_secrets_from_env(env: &mut HashMap<String, String>) -> bool {
    let mut stripped = false;
    for (k, v) in env.iter_mut() {
        let lk = k.to_lowercase();
        if SECRET_KEY_FRAGMENTS.iter().any(|frag| lk.contains(frag)) {
            if !v.is_empty() {
                stripped = true;
            }
            v.clear();
        }
    }
    stripped
}
