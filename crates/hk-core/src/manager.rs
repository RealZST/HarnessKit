use anyhow::{Context, Result};
use crate::models::*;
use crate::store::Store;
use crate::{adapter, deployer, sanitize, scanner};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallResult {
    pub name: String,
    pub was_update: bool,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredSkill {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub path: String,
}

pub struct Manager {
    pub store: Store,
}

impl Manager {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub fn toggle(&self, id: &str, enabled: bool) -> Result<()> {
        toggle_extension(&self.store, id, enabled)
    }

    pub fn uninstall(&self, id: &str) -> Result<()> {
        self.store.delete_extension(id)
    }

    pub fn update_tags(&self, _id: &str, _tags: Vec<String>) -> Result<()> {
        // v1: tags stored in extension_tags_json, update via store
        // Implementation: read extension, modify tags, write back
        Ok(())
    }

    pub fn toggle_by_pack(&self, pack: &str, enabled: bool) -> Result<Vec<String>> {
        let ids = self.store.find_ids_by_pack(pack)?;
        for id in &ids {
            toggle_extension(&self.store, id, enabled)?;
        }
        Ok(ids)
    }
}

/// Toggle an extension's enabled state. Handles all 5 kinds:
/// Skill (file rename), MCP (config read/write), Hook (config read/write),
/// Plugin (Claude config-driven or non-Claude manifest rename), CLI (cascade to children).
pub fn toggle_extension(store: &Store, id: &str, enabled: bool) -> Result<()> {
    let adapters = adapter::all_adapters();
    toggle_extension_with_adapters(store, &adapters, id, enabled)
}

/// Same as `toggle_extension` but accepts pre-built adapters to avoid redundant construction.
pub fn toggle_extension_with_adapters(
    store: &Store,
    adapters: &[Box<dyn adapter::AgentAdapter>],
    id: &str,
    enabled: bool,
) -> Result<()> {
    let ext = store.get_extension(id)?
        .ok_or_else(|| anyhow::anyhow!("Extension not found: {}", id))?;

    // Already in the target state — nothing to do.
    if ext.enabled == enabled {
        return Ok(());
    }

    match ext.kind {
        ExtensionKind::Skill => {
            toggle_skill(&ext, enabled, adapters)?;
            let sibling_ids = store.find_siblings_by_source_path(id)?;
            for sib_id in &sibling_ids {
                store.set_enabled(sib_id, enabled)?;
            }
        }
        ExtensionKind::Mcp => {
            toggle_mcp(&ext, enabled, store, adapters)?;
            store.set_enabled(id, enabled)?;
        }
        ExtensionKind::Hook => {
            toggle_hook(&ext, enabled, store, adapters)?;
            store.set_enabled(id, enabled)?;
        }
        ExtensionKind::Plugin => {
            toggle_plugin(&ext, enabled, store, adapters)?;
            store.set_enabled(id, enabled)?;
        }
        ExtensionKind::Cli => {
            let children = store.get_child_skills(id)?;
            for child in &children {
                match child.kind {
                    ExtensionKind::Skill => {
                        toggle_skill(child, enabled, adapters)?;
                        let sibling_ids = store.find_siblings_by_source_path(&child.id)?;
                        for sib_id in &sibling_ids {
                            store.set_enabled(sib_id, enabled)?;
                        }
                    }
                    ExtensionKind::Mcp => {
                        toggle_mcp(child, enabled, store, adapters)?;
                        store.set_enabled(&child.id, enabled)?;
                    }
                    _ => {
                        store.set_enabled(&child.id, enabled)?;
                    }
                }
            }
            store.set_enabled(id, enabled)?;
        }
    }
    Ok(())
}

fn toggle_skill(ext: &Extension, enabled: bool, adapters: &[Box<dyn adapter::AgentAdapter>]) -> Result<()> {
    use crate::scanner::skill_locations;
    let locations = skill_locations(&ext.name, adapters);

    // Fallback: if no paths found via adapters, use the stored source_path
    let paths: Vec<PathBuf> = if locations.is_empty() {
        ext.source_path.iter().map(|p| {
            let full = PathBuf::from(p);
            full.parent().unwrap_or(&full).to_path_buf()
        }).collect()
    } else {
        locations.into_iter().map(|(_, path)| path).collect()
    };

    for skill_dir in &paths {
        let skill_file = skill_dir.join("SKILL.md");
        let disabled_file = skill_dir.join("SKILL.md.disabled");
        if enabled {
            if disabled_file.exists() { std::fs::rename(&disabled_file, &skill_file)?; }
        } else if skill_file.exists() {
            std::fs::rename(&skill_file, &disabled_file)?;
        }
    }
    Ok(())
}

fn toggle_mcp(ext: &Extension, enabled: bool, store: &Store, adapters: &[Box<dyn adapter::AgentAdapter>]) -> Result<()> {
    for a in adapters {
        if !ext.agents.contains(&a.name().to_string()) { continue; }
        let config_path = a.mcp_config_path();
        let format = a.mcp_format();
        if enabled {
            let saved = store.get_disabled_config(&ext.id)?
                .ok_or_else(|| anyhow::anyhow!("No saved config for MCP server '{}'", ext.name))?;
            let entry: serde_json::Value = serde_json::from_str(&saved)?;
            // Warn if redacted env values are being restored (secrets were scrubbed)
            if let Some(env) = entry.get("env").and_then(|e| e.as_object()) {
                if env.values().any(|v| v.as_str() == Some("<redacted>")) {
                    eprintln!(
                        "[hk] warning: MCP server '{}' has redacted env values — \
                         you may need to re-enter secrets after re-enabling",
                        ext.name
                    );
                }
            }
            deployer::restore_mcp_server(&config_path, &ext.name, &entry, format)?;
            store.set_disabled_config(&ext.id, None)?;
        } else {
            let entry = deployer::read_mcp_server_config(&config_path, &ext.name, format)?
                .ok_or_else(|| anyhow::anyhow!("MCP server '{}' not found in config", ext.name))?;
            // Redact env values before persisting to the DB so secrets are not
            // stored in plain text in the SQLite database.
            let redacted = redact_mcp_env(&entry);
            store.set_disabled_config(&ext.id, Some(&redacted.to_string()))?;
            deployer::remove_mcp_server(&config_path, &ext.name, format)?;
        }
    }
    Ok(())
}

/// Replace env variable values with "<redacted>" while preserving keys.
/// This prevents secrets (API keys, tokens, etc.) from being stored in the
/// harnesskit SQLite database when an MCP server is disabled.
fn redact_mcp_env(entry: &serde_json::Value) -> serde_json::Value {
    let mut redacted = entry.clone();
    if let Some(env) = redacted.get_mut("env").and_then(|e| e.as_object_mut()) {
        for (_key, value) in env.iter_mut() {
            *value = serde_json::Value::String("<redacted>".into());
        }
    }
    redacted
}

fn toggle_hook(ext: &Extension, enabled: bool, store: &Store, adapters: &[Box<dyn adapter::AgentAdapter>]) -> Result<()> {
    let parts: Vec<&str> = ext.name.splitn(3, ':').collect();
    if parts.len() < 3 { anyhow::bail!("Invalid hook name: {}", ext.name); }
    let (event, matcher_str, command) = (parts[0], parts[1], parts[2]);
    let matcher = if matcher_str == "*" { None } else { Some(matcher_str) };
    for a in adapters {
        if !ext.agents.contains(&a.name().to_string()) { continue; }
        let config_path = a.hook_config_path();
        if enabled {
            let saved = store.get_disabled_config(&ext.id)?
                .ok_or_else(|| anyhow::anyhow!("No saved config for hook '{}'", ext.name))?;
            let entry: serde_json::Value = serde_json::from_str(&saved)?;
            deployer::restore_hook(&config_path, event, &entry, a.hook_format())?;
            store.set_disabled_config(&ext.id, None)?;
        } else {
            let entry = deployer::read_hook_config(&config_path, event, matcher, command, a.hook_format())?
                .ok_or_else(|| anyhow::anyhow!("Hook '{}' not found in config", ext.name))?;
            store.set_disabled_config(&ext.id, Some(&entry.to_string()))?;
            deployer::remove_hook(&config_path, event, matcher, command, a.hook_format())?;
        }
    }
    Ok(())
}

fn toggle_plugin(ext: &Extension, enabled: bool, store: &Store, adapters: &[Box<dyn adapter::AgentAdapter>]) -> Result<()> {
    for a in adapters {
        if !ext.agents.contains(&a.name().to_string()) { continue; }
        if a.name() == "claude" {
            let config_path = a.plugin_config_path();
            if enabled {
                // Re-enable: read plugin_key and value from saved disabled_config
                let saved = store.get_disabled_config(&ext.id)?
                    .ok_or_else(|| anyhow::anyhow!("No saved config for plugin '{}'", ext.name))?;
                let saved_obj: serde_json::Value = serde_json::from_str(&saved)?;

                // New format: {"plugin_key": "name@source", "value": <json>}
                // Old format: just the raw value (e.g., true) — reconstruct plugin_key from extension data
                let (plugin_key, value) = if let Some(key) = saved_obj.get("plugin_key").and_then(|v| v.as_str()) {
                    let val = saved_obj.get("value").cloned().unwrap_or(serde_json::Value::Bool(true));
                    (key.to_string(), val)
                } else {
                    // Old format fallback: reconstruct plugin_key from ext.description
                    // Scanner sets description to "Plugin from {source}" or "Plugin for {agent}"
                    let source = ext.description.strip_prefix("Plugin from ").unwrap_or("");
                    let key = if source.is_empty() {
                        ext.name.clone()
                    } else {
                        format!("{}@{}", ext.name, source)
                    };
                    (key, saved_obj)
                };

                deployer::restore_plugin_entry(&config_path, &plugin_key, &value)?;
                store.set_disabled_config(&ext.id, None)?;
            } else {
                // Disable: find plugin_key from live config, save both key and value
                let plugin_key = {
                    let mut found_key = None;
                    for plugin in a.read_plugins() {
                        let id_name = format!("{}:{}", plugin.name, plugin.source);
                        if scanner::stable_id_for(&id_name, "plugin", a.name()) == ext.id {
                            found_key = Some(if plugin.source.is_empty() {
                                plugin.name.clone()
                            } else {
                                format!("{}@{}", plugin.name, plugin.source)
                            });
                            break;
                        }
                    }
                    found_key.ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found in agent config", ext.name))?
                };
                let value = deployer::read_plugin_config(&config_path, &plugin_key)?
                    .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found in config", ext.name))?;
                // Store both plugin_key and value so re-enable doesn't need the live config
                let saved = serde_json::json!({ "plugin_key": plugin_key, "value": value });
                store.set_disabled_config(&ext.id, Some(&saved.to_string()))?;
                deployer::remove_plugin_entry(&config_path, &plugin_key)?;
            }
        } else {
            if enabled {
                // Re-enable: try new format first, then search plugin dirs as fallback
                let disabled_manifest = if let Some(saved) = store.get_disabled_config(&ext.id)? {
                    let saved_obj: serde_json::Value = serde_json::from_str(&saved)?;
                    saved_obj.get("manifest_path").and_then(|v| v.as_str())
                        .map(PathBuf::from)
                } else {
                    None
                };

                // Old format fallback: search plugin_dirs for disabled manifests
                let disabled_manifest = if let Some(p) = disabled_manifest {
                    Some(p)
                } else {
                    find_disabled_manifest(a.as_ref(), &ext.id)
                };

                if let Some(disabled) = disabled_manifest {
                    let s = disabled.to_string_lossy();
                    let manifest = if let Some(stripped) = s.strip_suffix(".disabled") {
                        PathBuf::from(stripped)
                    } else {
                        disabled.clone()
                    };
                    if disabled.exists() {
                        std::fs::rename(&disabled, &manifest)?;
                    }
                }
                store.set_disabled_config(&ext.id, None)?;
            } else {
                // Disable: find plugin via live scan, rename manifest, save path
                for plugin in a.read_plugins() {
                    let plugin_id_name = format!("{}:{}", plugin.name, plugin.source);
                    if scanner::stable_id_for(&plugin_id_name, "plugin", a.name()) != ext.id { continue; }
                    if let Some(ref path) = plugin.path {
                        // Try known manifest locations
                        for manifest_name in &["plugin.json", ".cursor-plugin/plugin.json", ".codex-plugin/plugin.json"] {
                            let manifest = path.join(manifest_name);
                            if manifest.exists() {
                                let disabled_manifest = PathBuf::from(
                                    format!("{}.disabled", manifest.to_string_lossy())
                                );
                                let saved = serde_json::json!({ "manifest_path": disabled_manifest.to_string_lossy() });
                                store.set_disabled_config(&ext.id, Some(&saved.to_string()))?;
                                std::fs::rename(&manifest, &disabled_manifest)?;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Search plugin directories for a disabled manifest matching the given extension ID.
/// Used as a fallback for plugins disabled before we started saving the manifest path.
fn find_disabled_manifest(adapter: &dyn adapter::AgentAdapter, ext_id: &str) -> Option<PathBuf> {
    for plugin_dir in adapter.plugin_dirs() {
        if let Ok(entries) = std::fs::read_dir(&plugin_dir) {
            for entry in entries.flatten() {
                if !entry.path().is_dir() { continue; }
                // Check known manifest locations with .disabled suffix
                for manifest_name in &["plugin.json.disabled", ".cursor-plugin/plugin.json.disabled", ".codex-plugin/plugin.json.disabled"] {
                    let disabled = entry.path().join(manifest_name);
                    if disabled.exists() {
                        // Read the disabled manifest to get the plugin name
                        if let Ok(content) = std::fs::read_to_string(&disabled)
                            && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                                let fallback_name = entry.file_name().to_string_lossy().to_string();
                                let name = val.get("name").and_then(|v| v.as_str())
                                    .unwrap_or(&fallback_name);
                                // Reconstruct the stable ID to check if it matches
                                let dir_name = plugin_dir.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                let source = if dir_name == "local" { "local" } else { &dir_name };
                                let id_name = format!("{}:{}", name, source);
                                if scanner::stable_id_for(&id_name, "plugin", adapter.name()) == ext_id {
                                    return Some(disabled);
                                }
                            }
                        // If we can't read the manifest, try matching by directory name
                        let dir_name_str = entry.file_name().to_string_lossy().to_string();
                        let source = plugin_dir.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let id_name = format!("{}:{}", dir_name_str, source);
                        if scanner::stable_id_for(&id_name, "plugin", adapter.name()) == ext_id {
                            return Some(disabled);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Check if an installed extension has an update available.
/// Uses `InstallMeta` (persisted install source) for the remote URL and local revision.
pub fn check_update(meta: &InstallMeta) -> UpdateStatus {
    let url = match meta.url_resolved.as_deref().or(meta.url.as_deref()) {
        Some(u) => u,
        None => return UpdateStatus::Error { message: "No remote URL".into() },
    };
    match get_remote_head(url) {
        Ok(remote_hash) => {
            match meta.revision.as_deref() {
                Some(local_hash) if remote_hash.starts_with(local_hash) || local_hash.starts_with(&remote_hash) => {
                    UpdateStatus::UpToDate { remote_hash }
                }
                _ => {
                    // No local revision (e.g. pre-existing skill matched via marketplace)
                    // or revision differs — treat as update available
                    UpdateStatus::UpdateAvailable { remote_hash }
                }
            }
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("No main or master branch found") {
                // Repo has no main/master — we can't reliably determine updates,
                // so default to up-to-date to avoid false positives / infinite loops.
                UpdateStatus::UpToDate { remote_hash: meta.revision.clone().unwrap_or_default() }
            } else {
                UpdateStatus::Error { message: msg }
            }
        }
    }
}

pub fn get_remote_head(url: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["ls-remote", "--heads", url])
        .output()
        .context("Failed to run git ls-remote")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git ls-remote failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Format: "<hash>\trefs/heads/<branch>"
    // Only check main/master — if neither exists, return None so caller
    // can treat the extension as up-to-date rather than falsely flagging updates.
    let lines: Vec<&str> = stdout.lines().collect();
    for suffix in &["refs/heads/main", "refs/heads/master"] {
        if let Some(line) = lines.iter().find(|l| l.ends_with(suffix))
            && let Some(hash) = line.split_whitespace().next() {
                return Ok(hash.to_string());
            }
    }
    anyhow::bail!("No main or master branch found")
}

/// Install a skill from a git URL by cloning and copying to the skills directory.
/// If `skill_id` is provided and non-empty, install only the matching skill subdirectory.
pub fn install_from_git(url: &str, target_dir: &Path) -> Result<InstallResult> {
    install_from_git_with_id(url, target_dir, None)
}

pub fn install_from_git_with_id(url: &str, target_dir: &Path, skill_id: Option<&str>) -> Result<InstallResult> {
    let temp = tempfile::tempdir().context("Failed to create temp directory")?;
    let clone_dir = temp.path().join("repo");

    let output = Command::new("git")
        .args(["clone", "--depth", "1", "--", url, &clone_dir.to_string_lossy()])
        .output()
        .context("Failed to run git clone")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git clone failed: {}", stderr.trim());
    }

    // Capture git revision before temp dir is dropped
    let revision = capture_git_revision(&clone_dir);

    let mut result = resolve_and_copy_skill(&clone_dir, target_dir, skill_id, url)?;
    result.revision = revision;
    Ok(result)
}

/// Given an already-cloned repo directory, resolve which skill to install and copy it.
/// Extracted from `install_from_git_with_id` for testability.
fn resolve_and_copy_skill(clone_dir: &Path, target_dir: &Path, skill_id: Option<&str>, url: &str) -> Result<InstallResult> {
    let skill_id = skill_id.filter(|s| !s.is_empty());

    // Validate skill_id contains no path traversal
    if let Some(sid) = skill_id {
        sanitize::validate_name(sid)
            .context(format!("Invalid skill_id: {}", sid))?;
    }

    // If skill_id is specified, look for it in specific paths
    if let Some(sid) = skill_id {
        // Try: skills/{skill_id}/, {skill_id}/
        let candidates = [
            clone_dir.join("skills").join(sid),
            clone_dir.join(sid),
        ];
        for candidate in &candidates {
            if candidate.is_dir() && candidate.join("SKILL.md").exists() {
                let name = crate::scanner::parse_skill_name(&candidate.join("SKILL.md"))
                    .unwrap_or_else(|| sid.to_string());
                sanitize::validate_name(&name)
                    .context(format!("Skill name '{}' contains invalid characters", name))?;
                let dest = target_dir.join(&name);
                let was_update = dest.is_dir();
                copy_dir_contents(candidate, &dest)?;
                return Ok(InstallResult { name, was_update, revision: None });
            }
        }
        // Fallback: root-level SKILL.md, but only for genuine single-skill repos.
        // If any subdirectory also contains SKILL.md, the specified skill_id should
        // have matched one of them — don't silently install the root.
        if clone_dir.join("SKILL.md").exists() {
            let has_sub_skills = std::fs::read_dir(clone_dir).ok()
                .map(|entries| entries.flatten().any(|e| {
                    let p = e.path();
                    if !p.is_dir() { return false; }
                    let name = e.file_name();
                    if name == ".git" { return false; }
                    if name == "skills" {
                        // Check inside skills/ directory
                        return std::fs::read_dir(&p).ok()
                            .map(|subs| subs.flatten().any(|s| s.path().join("SKILL.md").exists()))
                            .unwrap_or(false);
                    }
                    p.join("SKILL.md").exists()
                }))
                .unwrap_or(false);
            if !has_sub_skills {
                let name = crate::scanner::parse_skill_name(&clone_dir.join("SKILL.md"))
                    .unwrap_or_else(|| sid.to_string());
                sanitize::validate_name(&name)
                    .context(format!("Skill name '{}' contains invalid characters", name))?;
                let dest = target_dir.join(&name);
                let was_update = dest.is_dir();
                copy_dir_contents(clone_dir, &dest)?;
                return Ok(InstallResult { name, was_update, revision: None });
            }
        }
        // Fallback: search the repo tree for a directory whose name exactly matches
        // skill_id and contains SKILL.md. This handles repos like impeccable that nest
        // skills under agent directories (e.g. .claude/skills/typeset/SKILL.md).
        if let Some(found) = find_skill_dir_in_tree(clone_dir, sid, 4) {
            let name = crate::scanner::parse_skill_name(&found.join("SKILL.md"))
                .unwrap_or_else(|| sid.to_string());
            sanitize::validate_name(&name)
                .context(format!("Skill name '{}' contains invalid characters", name))?;
            let dest = target_dir.join(&name);
            let was_update = dest.is_dir();
            copy_dir_contents(&found, &dest)?;
            return Ok(InstallResult { name, was_update, revision: None });
        }
        anyhow::bail!("Skill '{}' not found in repository. Looked in skills/{0}/, {0}/, root, and searched the repo tree", sid);
    }

    // Generic: look for SKILL.md in root or immediate subdirectories
    if clone_dir.join("SKILL.md").exists() {
        let name = crate::scanner::parse_skill_name(&clone_dir.join("SKILL.md"))
            .unwrap_or_else(|| repo_name_from_url(url));
        sanitize::validate_name(&name)
            .context(format!("Skill name '{}' contains invalid characters", name))?;
        let dest = target_dir.join(&name);
        let was_update = dest.is_dir();
        copy_dir_contents(clone_dir, &dest)?;
        return Ok(InstallResult { name, was_update, revision: None });
    }

    if let Ok(entries) = std::fs::read_dir(clone_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("SKILL.md").exists() {
                let name = crate::scanner::parse_skill_name(&p.join("SKILL.md"))
                    .unwrap_or_else(|| p.file_name().unwrap_or_default().to_string_lossy().to_string());
                sanitize::validate_name(&name)
                    .context(format!("Skill name '{}' contains invalid characters", name))?;
                let dest = target_dir.join(&name);
                let was_update = dest.is_dir();
                copy_dir_contents(&p, &dest)?;
                return Ok(InstallResult { name, was_update, revision: None });
            }
        }
    }

    anyhow::bail!("No SKILL.md found in repository")
}

/// Discover all skills in a cloned repository directory.
pub fn scan_repo_skills(clone_dir: &Path) -> Vec<DiscoveredSkill> {
    let mut skills = Vec::new();

    // Check root SKILL.md
    let root_skill = clone_dir.join("SKILL.md");
    if root_skill.exists() {
        let (name, desc, _) = crate::scanner::parse_skill_frontmatter(
            &std::fs::read_to_string(&root_skill).unwrap_or_default()
        ).unwrap_or_else(|| (repo_name_from_url(""), String::new(), vec![]));
        // Only add root if there are no subdirectory skills
        let has_sub = has_subdirectory_skills(clone_dir);
        if !has_sub {
            skills.push(DiscoveredSkill {
                skill_id: String::new(),
                name,
                description: desc,
                path: ".".into(),
            });
            return skills;
        }
    }

    // Check skills/*/ subdirectories
    let skills_dir = clone_dir.join("skills");
    if skills_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.join("SKILL.md").exists() {
                    let content = std::fs::read_to_string(p.join("SKILL.md")).unwrap_or_default();
                    let (name, desc, _) = crate::scanner::parse_skill_frontmatter(&content)
                        .unwrap_or_else(|| (entry.file_name().to_string_lossy().to_string(), String::new(), vec![]));
                    skills.push(DiscoveredSkill {
                        skill_id: entry.file_name().to_string_lossy().to_string(),
                        name,
                        description: desc,
                        path: format!("skills/{}", entry.file_name().to_string_lossy()),
                    });
                }
            }
        }

    // Check immediate subdirectories (top-level)
    if let Ok(entries) = std::fs::read_dir(clone_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            let fname = entry.file_name();
            if fname == ".git" || fname == "skills" { continue; }
            if p.is_dir() && p.join("SKILL.md").exists() {
                let content = std::fs::read_to_string(p.join("SKILL.md")).unwrap_or_default();
                let (name, desc, _) = crate::scanner::parse_skill_frontmatter(&content)
                    .unwrap_or_else(|| (fname.to_string_lossy().to_string(), String::new(), vec![]));
                // Avoid duplicate if already found in skills/ scan
                if !skills.iter().any(|s| s.skill_id == fname.to_string_lossy().as_ref()) {
                    skills.push(DiscoveredSkill {
                        skill_id: fname.to_string_lossy().to_string(),
                        name,
                        description: desc,
                        path: fname.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }

    skills
}

fn has_subdirectory_skills(clone_dir: &Path) -> bool {
    // Check skills/*/SKILL.md
    if let Ok(entries) = std::fs::read_dir(clone_dir.join("skills")) {
        for entry in entries.flatten() {
            if entry.path().is_dir() && entry.path().join("SKILL.md").exists() {
                return true;
            }
        }
    }
    // Check */SKILL.md (immediate subdirs)
    if let Ok(entries) = std::fs::read_dir(clone_dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            if fname == ".git" || fname == "skills" { continue; }
            if entry.path().is_dir() && entry.path().join("SKILL.md").exists() {
                return true;
            }
        }
    }
    false
}

/// Install a specific skill from an already-cloned repository directory.
pub fn install_from_clone(clone_dir: &Path, target_dir: &Path, skill_id: Option<&str>, url: &str) -> Result<InstallResult> {
    let revision = capture_git_revision(clone_dir);
    let mut result = resolve_and_copy_skill(clone_dir, target_dir, skill_id, url)?;
    result.revision = revision;
    Ok(result)
}

/// Find a skill directory by name in a cloned repo.
/// 1. Try exact directory name match at common locations (skills/{name}/, {name}/)
/// 2. Recursive directory name match in tree
/// 3. Recursive SKILL.md frontmatter name match (handles repos where directory
///    name differs from skill name, e.g. "rag-pinecone" dir with name: "pinecone")
pub fn find_skill_in_repo(clone_dir: &Path, skill_name: &str) -> Option<std::path::PathBuf> {
    // Try common locations first (exact directory name)
    for prefix in &["skills", ""] {
        let candidate = if prefix.is_empty() {
            clone_dir.join(skill_name)
        } else {
            clone_dir.join(prefix).join(skill_name)
        };
        if candidate.is_dir() && candidate.join("SKILL.md").exists() {
            return Some(candidate);
        }
    }
    // Recursive directory name match
    if let Some(found) = find_skill_dir_in_tree(clone_dir, skill_name, 4) {
        return Some(found);
    }
    // Last resort: scan all SKILL.md files and match by frontmatter name.
    // Safe because the repo is already the confirmed source.
    find_skill_by_frontmatter_name(clone_dir, skill_name, 5)
}

/// Recursively search for a SKILL.md whose frontmatter `name` field matches `skill_name`.
fn find_skill_by_frontmatter_name(dir: &Path, skill_name: &str, max_depth: u32) -> Option<std::path::PathBuf> {
    if max_depth == 0 { return None; }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() { continue; }
        if entry.file_name() == ".git" { continue; }
        let skill_md = p.join("SKILL.md");
        if skill_md.exists()
            && let Some(parsed_name) = crate::scanner::parse_skill_name(&skill_md)
                && parsed_name.eq_ignore_ascii_case(skill_name) {
                    return Some(p);
                }
        if let Some(found) = find_skill_by_frontmatter_name(&p, skill_name, max_depth - 1) {
            return Some(found);
        }
    }
    None
}

/// Public wrapper for `capture_git_revision` (used by commands.rs).
pub fn capture_git_revision_pub(repo_dir: &Path) -> Option<String> {
    capture_git_revision(repo_dir)
}

/// Recursively search a directory tree for a subdirectory whose name exactly matches
/// `skill_id` and contains a SKILL.md file. Returns the first match found.
/// `max_depth` limits recursion to avoid scanning huge trees.
fn find_skill_dir_in_tree(dir: &Path, skill_id: &str, max_depth: u32) -> Option<std::path::PathBuf> {
    if max_depth == 0 { return None; }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() { continue; }
        let name = entry.file_name();
        if name == ".git" { continue; }
        // Exact directory name match + has SKILL.md
        if name.to_string_lossy() == skill_id && p.join("SKILL.md").exists() {
            return Some(p);
        }
        // Recurse into subdirectories
        if let Some(found) = find_skill_dir_in_tree(&p, skill_id, max_depth - 1) {
            return Some(found);
        }
    }
    None
}

/// Run `git rev-parse HEAD` in the given directory to capture the current revision.
/// Returns None if the command fails (e.g. not a git repo).
fn capture_git_revision(repo_dir: &Path) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn repo_name_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("unknown")
        .strip_suffix(".git")
        .unwrap_or(url.rsplit('/').next().unwrap_or("unknown"))
        .to_string()
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        // TOCTOU-safe symlink check: use symlink_metadata (lstat) instead of
        // following symlinks. Re-check right before the copy to close the race
        // window between readdir and the actual file operation.
        let meta = match std::fs::symlink_metadata(&src_path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[hk] warning: cannot read metadata for {}: {e}", src_path.display());
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            eprintln!("[hk] warning: skipping symlink: {}", src_path.display());
            continue;
        }
        if meta.file_type().is_dir() {
            if entry.file_name() == ".git" { continue; }
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_toggle_extension() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = crate::store::Store::open(&db_path).unwrap();

        // Create a fake skill file so toggle_skill can rename it
        let skill_dir = dir.path().join("skills").join("test");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(&skill_file, "---\nname: test\n---\n").unwrap();

        let ext = Extension {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ExtensionKind::Skill,
            name: "test".into(),
            description: "".into(),
            source: Source { origin: SourceOrigin::Local, url: None, version: None, commit_hash: None },
            agents: vec!["claude".into()],
            tags: vec![],
            pack: None,
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),

            source_path: Some(skill_file.to_string_lossy().to_string()),
            cli_parent_id: None,
            cli_meta: None,
            install_meta: None,
        };
        store.insert_extension(&ext).unwrap();

        let manager = Manager::new(store);
        manager.toggle(&ext.id, false).unwrap();
        let fetched = manager.store.get_extension(&ext.id).unwrap().unwrap();
        assert!(!fetched.enabled);
    }

    #[test]
    fn test_toggle_skill_renames_file() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = crate::store::Store::open(&db_path).unwrap();

        // Create a fake skill directory
        let skill_dir = dir.path().join("skills").join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(&skill_file, "---\nname: my-skill\n---\n").unwrap();

        let ext = Extension {
            id: "test-skill-id".into(),
            kind: ExtensionKind::Skill,
            name: "my-skill".into(),
            description: "".into(),
            source: Source { origin: SourceOrigin::Local, url: None, version: None, commit_hash: None },
            agents: vec!["claude".into()],
            tags: vec![],
            pack: None,
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),

            source_path: Some(skill_file.to_string_lossy().to_string()),
            cli_parent_id: None,
            cli_meta: None,
            install_meta: None,
        };
        store.insert_extension(&ext).unwrap();

        let manager = Manager::new(store);

        // Disable
        manager.toggle("test-skill-id", false).unwrap();
        assert!(!skill_file.exists(), "SKILL.md should be renamed away");
        assert!(skill_dir.join("SKILL.md.disabled").exists(), "SKILL.md.disabled should exist");
        let fetched = manager.store.get_extension("test-skill-id").unwrap().unwrap();
        assert!(!fetched.enabled);

        // Re-enable
        manager.toggle("test-skill-id", true).unwrap();
        assert!(skill_file.exists(), "SKILL.md should be restored");
        assert!(!skill_dir.join("SKILL.md.disabled").exists());
        let fetched = manager.store.get_extension("test-skill-id").unwrap().unwrap();
        assert!(fetched.enabled);
    }

    #[test]
    fn test_uninstall_extension() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = crate::store::Store::open(&db_path).unwrap();
        let ext = Extension {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ExtensionKind::Skill,
            name: "to-delete".into(),
            description: "".into(),
            source: Source { origin: SourceOrigin::Local, url: None, version: None, commit_hash: None },
            agents: vec!["claude".into()],
            tags: vec![],
            pack: None,
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),

            source_path: None,
            cli_parent_id: None,
            cli_meta: None,
            install_meta: None,
        };
        store.insert_extension(&ext).unwrap();

        let manager = Manager::new(store);
        manager.uninstall(&ext.id).unwrap();
        assert!(manager.store.get_extension(&ext.id).unwrap().is_none());
    }

    // --- resolve_and_copy_skill tests ---

    /// Helper: write a minimal SKILL.md with frontmatter
    fn write_skill_md(dir: &std::path::Path, name: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), format!("---\nname: {}\n---\n", name)).unwrap();
    }

    #[test]
    fn resolve_skill_from_subdirectory() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Multi-skill repo: skills/alpha/ and skills/beta/
        write_skill_md(&repo.path().join("skills").join("alpha"), "Alpha Skill");
        write_skill_md(&repo.path().join("skills").join("beta"), "Beta Skill");

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("alpha"), "").unwrap();
        assert_eq!(result.name, "Alpha Skill");
        assert!(!result.was_update);
        assert!(target.path().join("Alpha Skill").join("SKILL.md").exists());
    }

    #[test]
    fn resolve_skill_reports_was_update_when_dest_exists() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        write_skill_md(&repo.path().join("skills").join("alpha"), "Alpha Skill");
        // Pre-create destination to simulate a previous install
        std::fs::create_dir_all(target.path().join("Alpha Skill")).unwrap();

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("alpha"), "").unwrap();
        assert_eq!(result.name, "Alpha Skill");
        assert!(result.was_update);
    }

    #[test]
    fn resolve_skill_from_top_level_subdir() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Skill directly at {skill_id}/
        write_skill_md(&repo.path().join("my-skill"), "My Skill");

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("my-skill"), "").unwrap();
        assert_eq!(result.name, "My Skill");
    }

    #[test]
    fn resolve_root_skill_with_skill_id_single_skill_repo() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Single-skill repo: only root SKILL.md, no subdirectories with skills
        write_skill_md(repo.path(), "Root Skill");
        // Add a non-skill subdirectory (src/)
        std::fs::create_dir_all(repo.path().join("src")).unwrap();

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("whatever-id"), "").unwrap();
        assert_eq!(result.name, "Root Skill");
    }

    #[test]
    fn resolve_root_fallback_blocked_when_subdirectory_skills_exist() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Multi-skill repo with root SKILL.md AND subdirectory skills
        write_skill_md(repo.path(), "Root Skill");
        write_skill_md(&repo.path().join("skills").join("real-skill"), "Real Skill");

        // Asking for wrong skill_id should NOT silently install root
        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("wrong-id"), "");
        assert!(result.is_err(), "Should error when skill_id doesn't match and sub-skills exist");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("wrong-id"), "Error should mention the requested skill_id");
    }

    #[test]
    fn resolve_wrong_skill_id_with_subdir_skills_at_top_level() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Root SKILL.md + top-level subdirectory skill (not under skills/)
        write_skill_md(repo.path(), "Root Skill");
        write_skill_md(&repo.path().join("other-skill"), "Other Skill");

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("nonexistent"), "");
        assert!(result.is_err(), "Should error, not silently install root");
    }

    #[test]
    fn resolve_skill_rejects_traversal_in_name() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Create a skill whose SKILL.md has a name with path traversal
        let skill_dir = repo.path().join("skills").join("evil");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: ../../.claude/settings\n---\n",
        ).unwrap();

        let result = super::resolve_and_copy_skill(
            repo.path(), target.path(), Some("evil"), "",
        );
        assert!(result.is_err(), "Should reject path traversal in skill name");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid") || err.contains("path") || err.contains("traversal"),
            "Error should mention path issue, got: {}", err);
    }

    #[test]
    fn resolve_skill_rejects_traversal_in_skill_id() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        write_skill_md(repo.path(), "Normal Skill");

        let result = super::resolve_and_copy_skill(
            repo.path(), target.path(), Some("../../../etc"), "",
        );
        assert!(result.is_err(), "Should reject path traversal in skill_id");
    }

    #[test]
    fn resolve_generic_no_skill_id_picks_root() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        write_skill_md(repo.path(), "Root Skill");

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), None, "https://github.com/user/repo.git").unwrap();
        assert_eq!(result.name, "Root Skill");
    }

    #[test]
    fn resolve_generic_no_skill_id_picks_first_subdir() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // No root SKILL.md, one subdirectory with a skill
        write_skill_md(&repo.path().join("my-skill"), "My Skill");

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), None, "").unwrap();
        assert_eq!(result.name, "My Skill");
    }

    #[test]
    fn resolve_empty_skill_id_treated_as_none() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        write_skill_md(repo.path(), "Root Skill");

        // Empty string should be treated as None (generic path)
        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some(""), "").unwrap();
        assert_eq!(result.name, "Root Skill");
    }

    #[test]
    fn resolve_no_skill_md_anywhere_errors() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Empty repo
        let result = super::resolve_and_copy_skill(repo.path(), target.path(), None, "");
        assert!(result.is_err());
    }

    // --- check_update / get_remote_head tests ---

    /// Helper: create a bare git repo with a commit on the given branch, return (repo_path, commit_hash)
    fn create_bare_repo(branch: &str) -> (TempDir, String) {
        let bare = TempDir::new().unwrap();
        let work = TempDir::new().unwrap();

        // Init bare repo
        Command::new("git").args(["init", "--bare"])
            .arg(bare.path()).output().unwrap();

        // Clone, commit, push
        Command::new("git").args(["clone", &bare.path().to_string_lossy(), &work.path().to_string_lossy()])
            .output().unwrap();
        Command::new("git").args(["-C", &work.path().to_string_lossy(), "checkout", "-b", branch])
            .output().unwrap();
        std::fs::write(work.path().join("README.md"), "hello").unwrap();
        Command::new("git").args(["-C", &work.path().to_string_lossy(), "add", "."])
            .output().unwrap();
        Command::new("git")
            .args(["-C", &work.path().to_string_lossy(), "-c", "user.name=test", "-c", "user.email=test@test.com", "commit", "-m", "init"])
            .output().unwrap();
        Command::new("git").args(["-C", &work.path().to_string_lossy(), "push", "origin", branch])
            .output().unwrap();

        let out = Command::new("git").args(["-C", &work.path().to_string_lossy(), "rev-parse", "HEAD"])
            .output().unwrap();
        let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();

        (bare, hash)
    }

    /// Helper: push a new commit to an existing bare repo on the given branch
    fn push_new_commit(bare: &Path, branch: &str) -> String {
        let work = TempDir::new().unwrap();
        Command::new("git").args(["clone", "-b", branch, &bare.to_string_lossy(), &work.path().to_string_lossy()])
            .output().unwrap();
        std::fs::write(work.path().join("update.txt"), "updated").unwrap();
        Command::new("git").args(["-C", &work.path().to_string_lossy(), "add", "."])
            .output().unwrap();
        Command::new("git")
            .args(["-C", &work.path().to_string_lossy(), "-c", "user.name=test", "-c", "user.email=test@test.com", "commit", "-m", "update"])
            .output().unwrap();
        Command::new("git").args(["-C", &work.path().to_string_lossy(), "push", "origin", branch])
            .output().unwrap();

        let out = Command::new("git").args(["-C", &work.path().to_string_lossy(), "rev-parse", "HEAD"])
            .output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn make_meta(url: &str, revision: Option<&str>) -> InstallMeta {
        InstallMeta {
            install_type: "git".into(),
            url: Some(url.into()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: revision.map(|s| s.into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        }
    }

    #[test]
    fn get_remote_head_finds_main() {
        let (bare, hash) = create_bare_repo("main");
        let result = get_remote_head(&bare.path().to_string_lossy()).unwrap();
        assert_eq!(result, hash);
    }

    #[test]
    fn get_remote_head_finds_master() {
        let (bare, hash) = create_bare_repo("master");
        let result = get_remote_head(&bare.path().to_string_lossy()).unwrap();
        assert_eq!(result, hash);
    }

    #[test]
    fn get_remote_head_no_main_or_master_returns_error() {
        let (bare, _hash) = create_bare_repo("trunk");
        let result = get_remote_head(&bare.path().to_string_lossy());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No main or master branch found"));
    }

    #[test]
    fn check_update_same_hash_is_up_to_date() {
        let (bare, hash) = create_bare_repo("main");
        let meta = make_meta(&bare.path().to_string_lossy(), Some(&hash));
        match check_update(&meta) {
            UpdateStatus::UpToDate { remote_hash } => assert_eq!(remote_hash, hash),
            other => panic!("Expected UpToDate, got {:?}", other),
        }
    }

    #[test]
    fn check_update_different_hash_is_update_available() {
        let (bare, hash) = create_bare_repo("main");
        let meta = make_meta(&bare.path().to_string_lossy(), Some(&hash));

        // Push a new commit so remote moves ahead
        let new_hash = push_new_commit(bare.path(), "main");
        assert_ne!(hash, new_hash);

        match check_update(&meta) {
            UpdateStatus::UpdateAvailable { remote_hash } => assert_eq!(remote_hash, new_hash),
            other => panic!("Expected UpdateAvailable, got {:?}", other),
        }
    }

    #[test]
    fn check_update_no_local_revision_is_update_available() {
        let (bare, hash) = create_bare_repo("main");
        let meta = make_meta(&bare.path().to_string_lossy(), None);
        match check_update(&meta) {
            UpdateStatus::UpdateAvailable { remote_hash } => assert_eq!(remote_hash, hash),
            other => panic!("Expected UpdateAvailable, got {:?}", other),
        }
    }

    #[test]
    fn check_update_no_url_is_error() {
        let meta = InstallMeta {
            install_type: "git".into(),
            url: None,
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: Some("abc".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        match check_update(&meta) {
            UpdateStatus::Error { message } => assert!(message.contains("No remote URL")),
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn check_update_no_main_master_defaults_to_up_to_date() {
        let (bare, hash) = create_bare_repo("trunk");
        let meta = make_meta(&bare.path().to_string_lossy(), Some(&hash));
        match check_update(&meta) {
            UpdateStatus::UpToDate { remote_hash } => assert_eq!(remote_hash, hash),
            other => panic!("Expected UpToDate for non-main/master repo, got {:?}", other),
        }
    }

    #[test]
    fn check_update_prefers_url_resolved_over_url() {
        let (bare, hash) = create_bare_repo("main");
        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some("https://invalid-url-should-not-be-used.example.com/repo.git".into()),
            url_resolved: Some(bare.path().to_string_lossy().into()),
            branch: None,
            subpath: None,
            revision: Some(hash.clone()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        match check_update(&meta) {
            UpdateStatus::UpToDate { remote_hash } => assert_eq!(remote_hash, hash),
            other => panic!("Expected UpToDate (using url_resolved), got {:?}", other),
        }
    }

    #[test]
    fn check_update_after_update_cycle_is_consistent() {
        // Simulate full cycle: install → check (up-to-date) → remote updates → check (available) → update → check (up-to-date)
        let (bare, hash1) = create_bare_repo("main");
        let url = bare.path().to_string_lossy().to_string();

        // 1. After install: revision = hash1
        let meta1 = make_meta(&url, Some(&hash1));
        match check_update(&meta1) {
            UpdateStatus::UpToDate { .. } => {}
            other => panic!("Step 1: expected UpToDate, got {:?}", other),
        }

        // 2. Remote gets new commit
        let hash2 = push_new_commit(bare.path(), "main");

        // 3. Check detects update
        match check_update(&meta1) {
            UpdateStatus::UpdateAvailable { remote_hash } => assert_eq!(remote_hash, hash2),
            other => panic!("Step 3: expected UpdateAvailable, got {:?}", other),
        }

        // 4. After update: revision = hash2 (simulating what update_extension does)
        let meta2 = make_meta(&url, Some(&hash2));
        match check_update(&meta2) {
            UpdateStatus::UpToDate { remote_hash } => assert_eq!(remote_hash, hash2),
            other => panic!("Step 4: expected UpToDate after update, got {:?}", other),
        }
    }

    // --- redact_mcp_env tests ---

    #[test]
    fn redact_mcp_env_replaces_values() {
        let entry = serde_json::json!({
            "command": "npx",
            "args": ["-y", "some-server"],
            "env": {
                "API_KEY": "sk-secret-12345",
                "ENDPOINT": "https://api.example.com"
            }
        });
        let redacted = super::redact_mcp_env(&entry);
        assert_eq!(redacted["command"], "npx");
        assert_eq!(redacted["args"][0], "-y");
        assert_eq!(redacted["env"]["API_KEY"], "<redacted>");
        assert_eq!(redacted["env"]["ENDPOINT"], "<redacted>");
    }

    #[test]
    fn redact_mcp_env_preserves_entry_without_env() {
        let entry = serde_json::json!({
            "command": "npx",
            "args": ["server"]
        });
        let redacted = super::redact_mcp_env(&entry);
        assert_eq!(redacted, entry);
    }

    #[test]
    fn redact_mcp_env_handles_empty_env() {
        let entry = serde_json::json!({
            "command": "npx",
            "env": {}
        });
        let redacted = super::redact_mcp_env(&entry);
        assert_eq!(redacted["env"], serde_json::json!({}));
    }
}
