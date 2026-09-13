//! Rust port of the desktop's extension grouping, so `hk list` / `hk status`
//! / `hk audit` render the same rows as the desktop Extensions list.
//! Mirrors `extensionGroupKey` (src/lib/types.ts) and `buildGroups` /
//! `getCachedFiltered` (src/stores/extension-helpers.ts) — keep in sync.

use std::collections::{HashMap, HashSet};

use hk_core::models::{Extension, ExtensionKind};

/// One list row: every instance of the same logical extension across
/// agents and scopes, with the desktop's aggregate field semantics.
pub struct ExtensionGroup<'a> {
    /// First-seen instance; carries the group's name/kind/description.
    pub instances: Vec<&'a Extension>,
    /// Union across instances, in canonical agent display order.
    pub agents: Vec<String>,
    /// First instance that carries a pack.
    pub pack: Option<&'a str>,
    /// Enabled if any instance is.
    pub enabled: bool,
    /// Minimum across instances that have one.
    pub trust_score: Option<u8>,
}

impl<'a> ExtensionGroup<'a> {
    pub fn first(&self) -> &'a Extension {
        self.instances[0]
    }

    /// Human-facing name, matching the desktop table cell: hooks show their
    /// command with directory paths stripped from each token
    /// (`/usr/bin/afplay /System/…/Glass.aiff` → `afplay Glass.aiff`);
    /// everything else shows its plain name.
    pub fn display_name(&self) -> String {
        let first = self.first();
        if first.kind == ExtensionKind::Hook {
            let command = logical_name(first);
            if command != first.name {
                return command
                    .split(' ')
                    .map(|token| match token.rsplit('/').next() {
                        Some(last) if !last.is_empty() => last,
                        _ => token,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
            }
        }
        first.name.clone()
    }

    /// Desktop `agentsInScope` projection (All-scope mode): the union minus
    /// agents the user switched off in HarnessKit.
    pub fn visible_agents(&self, enabled_agents: &HashSet<String>) -> Vec<&str> {
        self.agents
            .iter()
            .map(String::as_str)
            .filter(|a| enabled_agents.contains(*a))
            .collect()
    }

    /// Desktop `groupHasEnabledAgent`: a group that lives only on
    /// switched-off agents drops out of every list and count; agentless
    /// rows (scanner-synthesised) always pass.
    pub fn has_enabled_agent(&self, enabled_agents: &HashSet<String>) -> bool {
        self.agents.is_empty() || self.agents.iter().any(|a| enabled_agents.contains(a))
    }
}

/// Hook names are `event:matcher:command`; their logical identity is the
/// command alone, so the same command deployed under different event names
/// folds into one row. Everything else groups by its plain name.
fn logical_name(ext: &Extension) -> &str {
    if ext.kind == ExtensionKind::Hook {
        let mut parts = ext.name.splitn(3, ':');
        if let (Some(_), Some(_), Some(command)) = (parts.next(), parts.next(), parts.next()) {
            return command;
        }
    }
    &ext.name
}

/// Authoritative "where did this come from" URL, resolution order
/// `install_meta.url` → `source.url` → `pack` (synthesized to a GitHub URL).
/// See `deriveExtensionUrl` in types.ts for why install_meta wins.
fn derive_extension_url(ext: &Extension) -> Option<String> {
    ext.install_meta
        .as_ref()
        .and_then(|m| m.url.clone())
        .or_else(|| ext.source.url.clone())
        .or_else(|| ext.pack.as_ref().map(|p| format!("https://github.com/{p}")))
}

/// `owner/repo` for GitHub URLs (without a `.git` suffix), the whole URL
/// otherwise.
fn extract_developer(url: &str) -> String {
    if let Some(pos) = url.find("github.com/") {
        let rest = &url[pos + "github.com/".len()..];
        let mut parts = rest.splitn(3, '/');
        if let (Some(owner), Some(repo)) = (parts.next(), parts.next())
            && !owner.is_empty()
            && !repo.is_empty()
        {
            return format!("{}/{}", owner, repo.trim_end_matches(".git"));
        }
    }
    url.to_string()
}

/// Stable grouping key `kind \0 logical name \0 developer`. Sourceless
/// skills/plugins/CLIs fall back to their scope so a project-level skill
/// never merges with an unrelated same-name global one; MCP servers and
/// hooks skip that fallback because their name IS their identity across
/// scopes (see the comment on `extensionGroupKey` in types.ts).
fn group_key(ext: &Extension) -> String {
    let developer = match derive_extension_url(ext) {
        Some(url) => extract_developer(&url),
        None => match ext.kind {
            ExtensionKind::Mcp | ExtensionKind::Hook => String::new(),
            _ => format!("({})", ext.scope.scope_key()),
        },
    };
    format!("{}\0{}\0{}", ext.kind.as_str(), logical_name(ext), developer)
}

/// Key for the sibling pre-pass in `build_groups`: same kind + logical
/// name + scope.
fn scoped_key(ext: &Extension) -> String {
    format!(
        "{}\0{}\0{}",
        ext.kind.as_str(),
        logical_name(ext),
        ext.scope.scope_key()
    )
}

/// Group extensions exactly like the desktop's `buildGroups`, preserving
/// first-seen order. The pre-pass lets a sourceless instance (an
/// agent-discovered copy without install metadata) attach to its
/// URL-carrying sibling instead of forming a separate row — but only when
/// there is exactly one such sibling in the same kind/name/scope.
/// `agent_order` (adapter names in registration order) fixes the agents
/// union's display order.
pub fn build_groups<'a>(
    extensions: &'a [Extension],
    agent_order: &[&str],
) -> Vec<ExtensionGroup<'a>> {
    let mut url_siblings: HashMap<String, HashSet<String>> = HashMap::new();
    for ext in extensions {
        if derive_extension_url(ext).is_some() {
            url_siblings
                .entry(scoped_key(ext))
                .or_default()
                .insert(group_key(ext));
        }
    }

    let mut keys_in_order: Vec<String> = Vec::new();
    let mut members: HashMap<String, Vec<&Extension>> = HashMap::new();
    for ext in extensions {
        let mut key = group_key(ext);
        if derive_extension_url(ext).is_none()
            && let Some(siblings) = url_siblings.get(&scoped_key(ext))
            && siblings.len() == 1
        {
            key = siblings.iter().next().unwrap().clone();
        }
        match members.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut e) => e.get_mut().push(ext),
            std::collections::hash_map::Entry::Vacant(e) => {
                keys_in_order.push(e.key().clone());
                e.insert(vec![ext]);
            }
        }
    }

    keys_in_order
        .into_iter()
        .map(|key| {
            let instances = members.remove(&key).expect("key recorded on insert");
            let mut agents: Vec<String> = instances
                .iter()
                .flat_map(|e| e.agents.iter().cloned())
                .collect();
            agents.sort();
            agents.dedup();
            agents.sort_by_key(|a| {
                agent_order
                    .iter()
                    .position(|n| n == a)
                    .unwrap_or(usize::MAX)
            });
            ExtensionGroup {
                agents,
                pack: instances.iter().find_map(|e| e.pack.as_deref()),
                enabled: instances.iter().any(|e| e.enabled),
                trust_score: instances.iter().filter_map(|e| e.trust_score).min(),
                instances,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ORDER: &[&str] = &["claude", "codex"];

    fn ext(id: &str, kind: &str, name: &str, url: Option<&str>, scope: serde_json::Value) -> Extension {
        serde_json::from_value(json!({
            "id": id,
            "kind": kind,
            "name": name,
            "description": "",
            "source": {
                "origin": "local",
                "url": url,
                "version": null,
                "commit_hash": null,
                "from_manifest": false
            },
            "agents": ["claude"],
            "tags": [],
            "pack": null,
            "permissions": [],
            "enabled": true,
            "trust_score": null,
            "installed_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "source_path": null,
            "cli_parent_id": null,
            "cli_meta": null,
            "install_meta": null,
            "scope": scope
        }))
        .unwrap()
    }

    fn global() -> serde_json::Value {
        json!({ "type": "global" })
    }

    fn project() -> serde_json::Value {
        json!({ "type": "project", "name": "demo", "path": "/p" })
    }

    #[test]
    fn sourceless_instance_merges_only_into_a_single_url_sibling() {
        let with_url = ext("a", "skill", "demo", Some("https://github.com/acme/tools"), global());
        let sourceless = ext("c", "skill", "demo", None, global());

        let one_sibling = vec![with_url.clone(), sourceless.clone()];
        assert_eq!(build_groups(&one_sibling, ORDER).len(), 1);

        // A second developer makes the sibling ambiguous: the sourceless
        // copy keeps its own row.
        let two_siblings = vec![
            with_url,
            ext("b", "skill", "demo", Some("https://github.com/other/repo"), global()),
            sourceless,
        ];
        assert_eq!(build_groups(&two_siblings, ORDER).len(), 3);
    }

    #[test]
    fn hooks_group_by_command_across_event_names() {
        let extensions = vec![
            ext("a", "hook", "PreToolUse:*:/usr/bin/afplay /System/Sounds/Glass.aiff", None, global()),
            ext("b", "hook", "PostToolUse:*:/usr/bin/afplay /System/Sounds/Glass.aiff", None, global()),
        ];
        let groups = build_groups(&extensions, ORDER);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].display_name(), "afplay Glass.aiff");
    }

    #[test]
    fn sourceless_skill_keeps_scopes_apart_but_mcp_merges_them() {
        // Sourceless skills fall back to their scope so unrelated same-name
        // copies never merge; an MCP server's name IS its identity, so its
        // Global + Project copies are one logical entry.
        let skills = vec![
            ext("a", "skill", "demo", None, global()),
            ext("b", "skill", "demo", None, project()),
        ];
        assert_eq!(build_groups(&skills, ORDER).len(), 2);

        let mcps = vec![
            ext("a", "mcp", "github", None, global()),
            ext("b", "mcp", "github", None, project()),
        ];
        assert_eq!(build_groups(&mcps, ORDER).len(), 1);
    }

}
