use crate::HkError;
use crate::models::*;
use crate::store::Store;
use crate::{adapter, deployer, sanitize, scanner};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct InstallResult {
    pub name: String,
    pub was_update: bool,
    pub revision: Option<String>,
    /// When true the skill was not found in the repo (e.g. removed by author)
    /// and the update was silently skipped.
    #[serde(default)]
    pub skipped: bool,
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

    pub fn toggle(&self, id: &str, enabled: bool) -> Result<(), HkError> {
        toggle_extension(&self.store, id, enabled)
    }

    pub fn uninstall(&self, id: &str) -> Result<(), HkError> {
        self.store.delete_extension(id)
    }

    pub fn update_tags(&self, _id: &str, _tags: Vec<String>) -> Result<(), HkError> {
        // v1: tags stored in extension_tags_json, update via store
        // Implementation: read extension, modify tags, write back
        Ok(())
    }

    pub fn toggle_by_pack(&self, pack: &str, enabled: bool) -> Result<Vec<String>, HkError> {
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
pub fn toggle_extension(store: &Store, id: &str, enabled: bool) -> Result<(), HkError> {
    let adapters = adapter::all_adapters();
    toggle_extension_with_adapters(store, &adapters, id, enabled)
}

/// Same as `toggle_extension` but accepts pre-built adapters to avoid redundant construction.
pub fn toggle_extension_with_adapters(
    store: &Store,
    adapters: &[Box<dyn adapter::AgentAdapter>],
    id: &str,
    enabled: bool,
) -> Result<(), HkError> {
    let ext = store
        .get_extension(id)?
        .ok_or_else(|| HkError::NotFound(format!("Extension not found: {}", id)))?;

    // Already in the target state — nothing to do.
    if ext.enabled == enabled {
        return Ok(());
    }

    match ext.kind {
        ExtensionKind::Skill => {
            toggle_skill(&ext, enabled, adapters)?;
            // Update ALL DB entries for this skill name across all agents
            let all_ids = store.find_ids_by_name_and_kind(&ext.name, ext.kind.as_str())?;
            for ext_id in &all_ids {
                store.set_enabled(ext_id, enabled)?;
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
            // CLI toggle only sets the CLI's own enabled state.
            // Child skills/MCPs are toggled independently by the frontend.
            store.set_enabled(id, enabled)?;
        }
    }
    Ok(())
}

fn toggle_skill(ext: &Extension, enabled: bool, adapters: &[Box<dyn adapter::AgentAdapter>]) -> Result<(), HkError> {
    use crate::scanner::skill_locations;
    let locations = skill_locations(&ext.name, adapters);

    // Fallback: if no paths found via adapters, use the stored source_path
    let paths: Vec<PathBuf> = if locations.is_empty() {
        ext.source_path
            .iter()
            .map(|p| {
                let full = PathBuf::from(p);
                full.parent().unwrap_or(&full).to_path_buf()
            })
            .collect()
    } else {
        locations.into_iter().map(|(_, path)| path).collect()
    };

    for skill_dir in &paths {
        let skill_file = skill_dir.join("SKILL.md");
        let disabled_file = skill_dir.join("SKILL.md.disabled");
        if enabled {
            if disabled_file.exists() {
                std::fs::rename(&disabled_file, &skill_file)?;
            }
        } else if skill_file.exists() {
            std::fs::rename(&skill_file, &disabled_file)?;
        }
    }
    Ok(())
}

fn toggle_mcp(ext: &Extension, enabled: bool, store: &Store, adapters: &[Box<dyn adapter::AgentAdapter>]) -> Result<(), HkError> {
    for a in adapters {
        if !ext.agents.contains(&a.name().to_string()) {
            continue;
        }
        let config_path = a.mcp_config_path();
        let format = a.mcp_format();
        if enabled {
            let saved = store.get_disabled_config(&ext.id)?.ok_or_else(|| {
                HkError::NotFound(format!("No saved config for MCP server '{}'", ext.name))
            })?;
            let mut entry: serde_json::Value = serde_json::from_str(&saved)?;
            // Self-heal: an earlier version of redact_mcp_env redacted PATH along
            // with secrets. For antigravity (where HarnessKit injects PATH because
            // the GUI launch doesn't inherit shell $PATH), recompute and overwrite
            // PATH so the restored config is actually usable.
            let needs_path_repair = a.name() == "antigravity"
                && entry
                    .get("env")
                    .and_then(|env| env.get("PATH"))
                    .and_then(|p| p.as_str())
                    == Some("<redacted>");
            if needs_path_repair {
                let cmd = entry
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let resolved = deployer::resolve_command_path(&cmd);
                if let Some(path_val) = deployer::build_path_for_command(&resolved)
                    && let Some(env_obj) = entry.get_mut("env").and_then(|v| v.as_object_mut())
                {
                    env_obj.insert("PATH".into(), serde_json::Value::String(path_val));
                }
            }
            // Warn about redacted env values — server will be restored but
            // the user needs to manually set the real values in the config.
            // PATH is excluded: it is auto-injected, not a secret, and is
            // self-healed above for antigravity.
            if let Some(env_obj) = entry.get("env").and_then(|v| v.as_object()) {
                let redacted_keys: Vec<&str> = env_obj
                    .iter()
                    .filter(|(k, v)| k.as_str() != "PATH" && v.as_str() == Some("<redacted>"))
                    .map(|(k, _)| k.as_str())
                    .collect();
                if !redacted_keys.is_empty() {
                    eprintln!(
                        "[hk] warning: MCP server '{}' has redacted environment variables ({}) — \
                         the server has been re-enabled but you must set the real values in the agent config",
                        ext.name,
                        redacted_keys.join(", ")
                    );
                }
            }
            deployer::restore_mcp_server(&config_path, &ext.name, &entry, format)?;
            store.set_disabled_config(&ext.id, None)?;
        } else {
            let entry = deployer::read_mcp_server_config(&config_path, &ext.name, format)?
                .ok_or_else(|| {
                    HkError::NotFound(format!("MCP server '{}' not found in config", ext.name))
                })?;
            // Redact env values before persisting to the DB so secrets are not
            // stored in plain text in the SQLite database.
            let redacted = redact_mcp_env(&entry);
            store.set_disabled_config(&ext.id, Some(&redacted.to_string()))?;
            deployer::remove_mcp_server(&config_path, &ext.name, format)?;
        }
    }
    Ok(())
}

/// Redact environment variable values in an MCP server config entry.
/// Replaces all values in the "env" object with "<redacted>" while preserving keys.
/// This prevents secrets (API keys, tokens, etc.) from being stored in the
/// harnesskit SQLite database when an MCP server is disabled.
///
/// `PATH` is excluded — HarnessKit auto-injects it for agents like antigravity
/// that don't inherit shell `$PATH` from a GUI launch (see install.rs). It is an
/// operational variable, not a secret, and must round-trip on disable→enable so
/// the server can find its binary again.
fn redact_mcp_env(entry: &serde_json::Value) -> serde_json::Value {
    let mut redacted = entry.clone();
    if let Some(env_obj) = redacted.get_mut("env").and_then(|v| v.as_object_mut()) {
        for (key, value) in env_obj.iter_mut() {
            if key == "PATH" {
                continue;
            }
            *value = serde_json::Value::String("<redacted>".into());
        }
    }
    redacted
}

fn toggle_hook(ext: &Extension, enabled: bool, store: &Store, adapters: &[Box<dyn adapter::AgentAdapter>]) -> Result<(), HkError> {
    let parts: Vec<&str> = ext.name.splitn(3, ':').collect();
    if parts.len() < 3 {
        return Err(HkError::Validation(format!(
            "Invalid hook name: {}",
            ext.name
        )));
    }
    let (event, matcher_str, command) = (parts[0], parts[1], parts[2]);
    let matcher = if matcher_str == "*" {
        None
    } else {
        Some(matcher_str)
    };
    for a in adapters {
        if !ext.agents.contains(&a.name().to_string()) {
            continue;
        }
        let config_path = a.hook_config_path();
        if enabled {
            let saved = store.get_disabled_config(&ext.id)?.ok_or_else(|| {
                HkError::NotFound(format!("No saved config for hook '{}'", ext.name))
            })?;
            let entry: serde_json::Value = serde_json::from_str(&saved)?;
            deployer::restore_hook(&config_path, event, &entry, a.hook_format())?;
            store.set_disabled_config(&ext.id, None)?;
        } else {
            let entry =
                deployer::read_hook_config(&config_path, event, matcher, command, a.hook_format())?
                    .ok_or_else(|| {
                        HkError::NotFound(format!("Hook '{}' not found in config", ext.name))
                    })?;
            store.set_disabled_config(&ext.id, Some(&entry.to_string()))?;
            deployer::remove_hook(&config_path, event, matcher, command, a.hook_format())?;
        }
    }
    Ok(())
}

/// Reconstruct the config-file plugin key (e.g. "name@marketplace") from extension metadata.
/// Scanner sets description to "Plugin from {source}" or "Plugin for {agent}".
fn plugin_key_from_ext(ext: &Extension) -> String {
    let source = ext.description.strip_prefix("Plugin from ").unwrap_or("");
    if source.is_empty() {
        ext.name.clone()
    } else {
        format!("{}@{}", ext.name, source)
    }
}

fn toggle_plugin(ext: &Extension, enabled: bool, store: &Store, adapters: &[Box<dyn adapter::AgentAdapter>]) -> Result<(), HkError> {
    for a in adapters {
        if !ext.agents.contains(&a.name().to_string()) {
            continue;
        }
        if a.name() == "claude" {
            let config_path = a.plugin_config_path();
            deployer::set_plugin_enabled(&config_path, &plugin_key_from_ext(ext), enabled)?;
            // Clean up any legacy disabled_config (no longer used for Claude)
            store.set_disabled_config(&ext.id, None)?;
        } else if a.name() == "codex" {
            let config_path = a.mcp_config_path();
            deployer::set_codex_plugin_enabled(&config_path, &plugin_key_from_ext(ext), enabled)?;
            store.set_disabled_config(&ext.id, None)?;
        } else if a.name() == "gemini" {
            let extensions_dir = a.base_dir().join("extensions");
            let home = dirs::home_dir()
                .ok_or_else(|| HkError::Internal("Cannot determine home directory".into()))?;
            deployer::set_gemini_extension_enabled(&extensions_dir, &ext.name, enabled, &home)?;
            store.set_disabled_config(&ext.id, None)?;
        } else if a.name() == "copilot" {
            // Check if this is a VS Code agent plugin (has uri from read_plugins).
            // If so, toggle via state.vscdb. Otherwise fall through to manifest rename.
            // Cache read_plugins result to avoid scanning twice for CLI plugins.
            let plugins = a.read_plugins();
            let plugin_uri = plugins.iter()
                .find(|p| {
                    let id_name = format!("{}:{}", p.name, p.source);
                    scanner::stable_id_for(&id_name, "plugin", a.name()) == ext.id
                })
                .and_then(|p| p.uri.clone());
            if let Some(uri) = plugin_uri {
                let vscode_user_dir = a.vscode_user_dir().ok_or_else(|| {
                    HkError::Internal("Copilot adapter missing vscode_user_dir".into())
                })?;
                deployer::set_vscode_plugin_enabled(&vscode_user_dir, &uri, enabled)?;
                store.set_disabled_config(&ext.id, None)?;
            } else {
                // Copilot CLI plugin — reuse cached plugins to avoid second scan
                toggle_plugin_manifest(ext, enabled, store, a.as_ref(), Some(plugins))?;
            }
        } else {
            // Generic: manifest rename for Cursor, Copilot CLI, etc.
            toggle_plugin_manifest(ext, enabled, store, a.as_ref(), None)?;
        }
    }
    Ok(())
}

/// Toggle a plugin by renaming its manifest file.
/// Used for agents without a native enable/disable config (Cursor, Copilot CLI).
/// `prefetched_plugins` avoids a redundant `read_plugins()` call when the caller already has the list.
fn toggle_plugin_manifest(
    ext: &Extension,
    enabled: bool,
    store: &Store,
    adapter: &dyn adapter::AgentAdapter,
    prefetched_plugins: Option<Vec<adapter::PluginEntry>>,
) -> Result<(), HkError> {
    if enabled {
        // Re-enable: try saved disabled_config first, then search plugin_dirs as fallback.
        // If neither finds a disabled manifest, this is a no-op (plugin may already be enabled
        // or was re-enabled externally). We still clear disabled_config to avoid stale state.
        let disabled_manifest = if let Some(saved) = store.get_disabled_config(&ext.id)? {
            let saved_obj: serde_json::Value = serde_json::from_str(&saved)?;
            saved_obj
                .get("manifest_path")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
        } else {
            None
        };
        let disabled_manifest = if let Some(p) = disabled_manifest {
            Some(p)
        } else {
            find_disabled_manifest(adapter, &ext.id)
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
        let plugins = prefetched_plugins.unwrap_or_else(|| adapter.read_plugins());
        let mut found = false;
        for plugin in plugins {
            let plugin_id_name = format!("{}:{}", plugin.name, plugin.source);
            if scanner::stable_id_for(&plugin_id_name, "plugin", adapter.name()) != ext.id {
                continue;
            }
            if let Some(ref path) = plugin.path {
                for manifest_name in &[
                    "plugin.json",
                    ".cursor-plugin/plugin.json",
                    ".codex-plugin/plugin.json",
                    ".plugin/plugin.json",
                    ".github/plugin/plugin.json",
                ] {
                    let manifest = path.join(manifest_name);
                    if manifest.exists() {
                        let disabled_manifest = PathBuf::from(format!(
                            "{}.disabled",
                            manifest.to_string_lossy()
                        ));
                        let saved = serde_json::json!({ "manifest_path": disabled_manifest.to_string_lossy() });
                        store.set_disabled_config(&ext.id, Some(&saved.to_string()))?;
                        std::fs::rename(&manifest, &disabled_manifest)?;
                        found = true;
                        break;
                    }
                }
            }
            break;
        }
        if !found {
            return Err(HkError::NotFound(format!(
                "No manifest found for plugin '{}' — cannot disable",
                ext.name
            )));
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
                if !entry.path().is_dir() {
                    continue;
                }
                // Check known manifest locations with .disabled suffix
                for manifest_name in &[
                    "plugin.json.disabled",
                    ".cursor-plugin/plugin.json.disabled",
                    ".codex-plugin/plugin.json.disabled",
                    ".plugin/plugin.json.disabled",
                    ".github/plugin/plugin.json.disabled",
                ] {
                    let disabled = entry.path().join(manifest_name);
                    if disabled.exists() {
                        // Read the disabled manifest to get the plugin name
                        if let Ok(content) = std::fs::read_to_string(&disabled)
                            && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
                        {
                            let fallback_name = entry.file_name().to_string_lossy().to_string();
                            let name = val
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or(&fallback_name);
                            // Reconstruct the stable ID to check if it matches
                            let dir_name = plugin_dir
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let source = if dir_name == "local" {
                                "local"
                            } else {
                                &dir_name
                            };
                            let id_name = format!("{}:{}", name, source);
                            if scanner::stable_id_for(&id_name, "plugin", adapter.name()) == ext_id
                            {
                                return Some(disabled);
                            }
                        }
                        // If we can't read the manifest, try matching by directory name
                        let dir_name_str = entry.file_name().to_string_lossy().to_string();
                        let source = plugin_dir
                            .file_name()
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
    check_update_with_cache(meta, &mut std::collections::HashMap::new())
}

/// Like `check_update`, but reuses a cache of `url -> Result<remote_hash>` to
/// avoid redundant `git ls-remote` calls for extensions sharing the same repo.
pub fn check_update_with_cache(
    meta: &InstallMeta,
    cache: &mut std::collections::HashMap<String, Result<String, String>>,
) -> UpdateStatus {
    let url = match meta.url_resolved.as_deref().or(meta.url.as_deref()) {
        Some(u) => u,
        None => {
            return UpdateStatus::Error {
                message: "No remote URL".into(),
            };
        }
    };
    // Validate DB-sourced URL before passing to git
    if let Err(e) = sanitize::validate_git_url(url) {
        return UpdateStatus::Error { message: e.to_string() };
    }
    let remote_result = cache
        .entry(url.to_string())
        .or_insert_with(|| {
            get_remote_head(url)
                .map_err(|e| e.to_string())
        });
    match remote_result {
        Ok(remote_hash) => {
            let remote_hash = remote_hash.clone();
            match meta.revision.as_deref() {
                Some(local_hash)
                    if remote_hash.starts_with(local_hash)
                        || local_hash.starts_with(&remote_hash) =>
                {
                    UpdateStatus::UpToDate { remote_hash }
                }
                _ => {
                    // No local revision (e.g. pre-existing skill matched via marketplace)
                    // or revision differs — treat as update available
                    UpdateStatus::UpdateAvailable { remote_hash }
                }
            }
        }
        Err(msg) => {
            if msg.contains("No main or master branch found") {
                UpdateStatus::UpToDate {
                    remote_hash: meta.revision.clone().unwrap_or_default(),
                }
            } else {
                UpdateStatus::Error { message: msg.clone() }
            }
        }
    }
}

pub fn get_remote_head(url: &str) -> Result<String, HkError> {
    let output = Command::new("git")
        .args(["ls-remote", "--heads", "--", url])
        .output()
        .map_err(|e| HkError::CommandFailed(format!("Failed to run git ls-remote: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(HkError::CommandFailed(format!(
            "git ls-remote failed: {}",
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Format: "<hash>\trefs/heads/<branch>"
    // Only check main/master — if neither exists, return None so caller
    // can treat the extension as up-to-date rather than falsely flagging updates.
    let lines: Vec<&str> = stdout.lines().collect();
    for suffix in &["refs/heads/main", "refs/heads/master"] {
        if let Some(line) = lines.iter().find(|l| l.ends_with(suffix))
            && let Some(hash) = line.split_whitespace().next()
        {
            return Ok(hash.to_string());
        }
    }
    Err(HkError::CommandFailed(
        "No main or master branch found".into(),
    ))
}

/// Install a skill from a git URL by cloning and copying to the skills directory.
/// If `skill_id` is provided and non-empty, install only the matching skill subdirectory.
pub fn install_from_git(url: &str, target_dir: &Path) -> Result<InstallResult, HkError> {
    install_from_git_with_id(url, target_dir, None)
}

pub fn install_from_git_with_id(
    url: &str,
    target_dir: &Path,
    skill_id: Option<&str>,
) -> Result<InstallResult, HkError> {
    let temp = tempfile::tempdir()
        .map_err(|e| HkError::Internal(format!("Failed to create temp directory: {e}")))?;
    let clone_dir = temp.path().join("repo");

    let output = Command::new("git")
        .args(["clone", "--depth", "1", "--", url, &clone_dir.to_string_lossy()])
        .output()
        .map_err(|e| HkError::CommandFailed(format!("Failed to run git clone: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(HkError::CommandFailed(format!(
            "git clone failed: {}",
            stderr.trim()
        )));
    }

    // Capture git revision before temp dir is dropped
    let revision = capture_git_revision(&clone_dir);

    let mut result = resolve_and_copy_skill(&clone_dir, target_dir, skill_id, url)?;
    result.revision = revision;
    Ok(result)
}

/// Given an already-cloned repo directory, resolve which skill to install and copy it.
/// Extracted from `install_from_git_with_id` for testability.
fn resolve_and_copy_skill(
    clone_dir: &Path,
    target_dir: &Path,
    skill_id: Option<&str>,
    url: &str,
) -> Result<InstallResult, HkError> {
    let skill_id = skill_id.filter(|s| !s.is_empty());

    // Validate skill_id contains no path traversal
    if let Some(sid) = skill_id {
        sanitize::validate_name(sid)
            .map_err(|e| HkError::Validation(format!("Invalid skill_id: {}: {}", sid, e)))?;
    }

    // If skill_id is specified, look for it in specific paths
    if let Some(sid) = skill_id {
        // Try: skills/{skill_id}/, {skill_id}/
        let candidates = [clone_dir.join("skills").join(sid), clone_dir.join(sid)];
        for candidate in &candidates {
            if candidate.is_dir() && candidate.join("SKILL.md").exists() {
                let name = crate::scanner::parse_skill_name(&candidate.join("SKILL.md"))
                    .unwrap_or_else(|| sid.to_string());
                sanitize::validate_name(&name).map_err(|e| {
                    HkError::Validation(format!(
                        "Skill name '{}' contains invalid characters: {}",
                        name, e
                    ))
                })?;
                let dest = target_dir.join(&name);
                let was_update = dest.is_dir();
                copy_dir_contents(candidate, &dest)?;
                return Ok(InstallResult {
                    name,
                    was_update,
                    revision: None,
                    ..Default::default()
                });
            }
        }
        // Fallback: root-level SKILL.md, but only for genuine single-skill repos.
        // If any subdirectory also contains SKILL.md, the specified skill_id should
        // have matched one of them — don't silently install the root.
        if clone_dir.join("SKILL.md").exists() {
            let has_sub_skills = std::fs::read_dir(clone_dir)
                .ok()
                .map(|entries| {
                    entries.flatten().any(|e| {
                        let p = e.path();
                        if !p.is_dir() {
                            return false;
                        }
                        let name = e.file_name();
                        if name == ".git" {
                            return false;
                        }
                        if name == "skills" {
                            // Check inside skills/ directory
                            return std::fs::read_dir(&p)
                                .ok()
                                .map(|subs| {
                                    subs.flatten().any(|s| s.path().join("SKILL.md").exists())
                                })
                                .unwrap_or(false);
                        }
                        p.join("SKILL.md").exists()
                    })
                })
                .unwrap_or(false);
            if !has_sub_skills {
                let name = crate::scanner::parse_skill_name(&clone_dir.join("SKILL.md"))
                    .unwrap_or_else(|| sid.to_string());
                sanitize::validate_name(&name).map_err(|e| {
                    HkError::Validation(format!(
                        "Skill name '{}' contains invalid characters: {}",
                        name, e
                    ))
                })?;
                let dest = target_dir.join(&name);
                let was_update = dest.is_dir();
                copy_dir_contents(clone_dir, &dest)?;
                return Ok(InstallResult {
                    name,
                    was_update,
                    revision: None,
                    ..Default::default()
                });
            }
        }
        // Fallback: search the repo tree for a directory whose name exactly matches
        // skill_id and contains SKILL.md. This handles repos like impeccable that nest
        // skills under agent directories (e.g. .claude/skills/typeset/SKILL.md).
        if let Some(found) = find_skill_dir_in_tree(clone_dir, sid, 4) {
            let name = crate::scanner::parse_skill_name(&found.join("SKILL.md"))
                .unwrap_or_else(|| sid.to_string());
            sanitize::validate_name(&name).map_err(|e| {
                HkError::Validation(format!(
                    "Skill name '{}' contains invalid characters: {}",
                    name, e
                ))
            })?;
            let dest = target_dir.join(&name);
            let was_update = dest.is_dir();
            copy_dir_contents(&found, &dest)?;
            return Ok(InstallResult {
                name,
                was_update,
                revision: None,
                ..Default::default()
            });
        }
        return Err(HkError::NotFound(format!(
            "Skill '{}' not found in repository. Looked in skills/{0}/, {0}/, root, and searched the repo tree",
            sid
        )));
    }

    // Generic: look for SKILL.md in root or immediate subdirectories
    if clone_dir.join("SKILL.md").exists() {
        let name = crate::scanner::parse_skill_name(&clone_dir.join("SKILL.md"))
            .unwrap_or_else(|| repo_name_from_url(url));
        sanitize::validate_name(&name).map_err(|e| {
            HkError::Validation(format!(
                "Skill name '{}' contains invalid characters: {}",
                name, e
            ))
        })?;
        let dest = target_dir.join(&name);
        let was_update = dest.is_dir();
        copy_dir_contents(clone_dir, &dest)?;
        return Ok(InstallResult {
            name,
            was_update,
            revision: None,
            ..Default::default()
        });
    }

    if let Ok(entries) = std::fs::read_dir(clone_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("SKILL.md").exists() {
                let name =
                    crate::scanner::parse_skill_name(&p.join("SKILL.md")).unwrap_or_else(|| {
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    });
                sanitize::validate_name(&name).map_err(|e| {
                    HkError::Validation(format!(
                        "Skill name '{}' contains invalid characters: {}",
                        name, e
                    ))
                })?;
                let dest = target_dir.join(&name);
                let was_update = dest.is_dir();
                copy_dir_contents(&p, &dest)?;
                return Ok(InstallResult {
                    name,
                    was_update,
                    revision: None,
                    ..Default::default()
                });
            }
        }
    }

    Err(HkError::NotFound("No SKILL.md found in repository".into()))
}

/// Recursively find all directories containing SKILL.md in a tree.
/// Skips `.git` directories. `max_depth` prevents scanning huge trees.
fn find_all_skill_dirs(dir: &Path, max_depth: u32) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if max_depth == 0 {
        return results;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return results;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() || entry.file_name() == ".git" {
            continue;
        }
        if p.join("SKILL.md").exists() {
            results.push(p.clone());
        }
        results.extend(find_all_skill_dirs(&p, max_depth - 1));
    }
    results
}

/// Discover all skills in a cloned repository directory.
pub fn scan_repo_skills(clone_dir: &Path) -> Vec<DiscoveredSkill> {
    // Recursively find all directories containing SKILL.md (max depth 4,
    // handles repos that nest skills under agent dirs like .claude/skills/)
    let skill_dirs = find_all_skill_dirs(clone_dir, 4);

    // Single-skill repo: root SKILL.md with no subdirectory skills
    if clone_dir.join("SKILL.md").exists() && skill_dirs.is_empty() {
        let (name, desc, _) = crate::scanner::parse_skill_frontmatter(
            &std::fs::read_to_string(clone_dir.join("SKILL.md")).unwrap_or_default(),
        )
        .unwrap_or_else(|| (repo_name_from_url(""), String::new(), vec![]));
        return vec![DiscoveredSkill {
            skill_id: String::new(),
            name,
            description: desc,
            path: ".".into(),
        }];
    }

    // Multi-skill repo: collect all discovered skills, deduplicate by directory name
    let mut skills = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    for dir in &skill_dirs {
        let content = std::fs::read_to_string(dir.join("SKILL.md")).unwrap_or_default();
        let dir_name = dir
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let (name, desc, _) = crate::scanner::parse_skill_frontmatter(&content)
            .unwrap_or_else(|| (dir_name.clone(), String::new(), vec![]));
        if seen_ids.insert(dir_name.clone()) {
            let relative_path = dir
                .strip_prefix(clone_dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| dir_name.clone());
            skills.push(DiscoveredSkill {
                skill_id: dir_name,
                name,
                description: desc,
                path: relative_path,
            });
        }
    }

    skills
}

/// Install a specific skill from an already-cloned repository directory.
pub fn install_from_clone(
    clone_dir: &Path,
    target_dir: &Path,
    skill_id: Option<&str>,
    url: &str,
) -> Result<InstallResult, HkError> {
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
    // Single-skill repo: SKILL.md at the repo root
    if clone_dir.join("SKILL.md").exists()
        && let Some(parsed) = crate::scanner::parse_skill_name(&clone_dir.join("SKILL.md"))
        && parsed.eq_ignore_ascii_case(skill_name)
    {
        return Some(clone_dir.to_path_buf());
    }
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
fn find_skill_by_frontmatter_name(
    dir: &Path,
    skill_name: &str,
    max_depth: u32,
) -> Option<std::path::PathBuf> {
    if max_depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        if entry.file_name() == ".git" {
            continue;
        }
        let skill_md = p.join("SKILL.md");
        if skill_md.exists()
            && let Some(parsed_name) = crate::scanner::parse_skill_name(&skill_md)
            && parsed_name.eq_ignore_ascii_case(skill_name)
        {
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
fn find_skill_dir_in_tree(
    dir: &Path,
    skill_id: &str,
    max_depth: u32,
) -> Option<std::path::PathBuf> {
    if max_depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
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

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), HkError> {
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
                eprintln!(
                    "[hk] warning: cannot read metadata for {}: {e}",
                    src_path.display()
                );
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            eprintln!("[hk] warning: skipping symlink: {}", src_path.display());
            continue;
        }
        if meta.file_type().is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
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
            source: Source {
                origin: SourceOrigin::Local,
                url: None,
                version: None,
                commit_hash: None,
            },
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
            source: Source {
                origin: SourceOrigin::Local,
                url: None,
                version: None,
                commit_hash: None,
            },
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
        assert!(
            skill_dir.join("SKILL.md.disabled").exists(),
            "SKILL.md.disabled should exist"
        );
        let fetched = manager
            .store
            .get_extension("test-skill-id")
            .unwrap()
            .unwrap();
        assert!(!fetched.enabled);

        // Re-enable
        manager.toggle("test-skill-id", true).unwrap();
        assert!(skill_file.exists(), "SKILL.md should be restored");
        assert!(!skill_dir.join("SKILL.md.disabled").exists());
        let fetched = manager
            .store
            .get_extension("test-skill-id")
            .unwrap()
            .unwrap();
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
            source: Source {
                origin: SourceOrigin::Local,
                url: None,
                version: None,
                commit_hash: None,
            },
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

        let result =
            super::resolve_and_copy_skill(repo.path(), target.path(), Some("alpha"), "").unwrap();
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

        let result =
            super::resolve_and_copy_skill(repo.path(), target.path(), Some("alpha"), "").unwrap();
        assert_eq!(result.name, "Alpha Skill");
        assert!(result.was_update);
    }

    #[test]
    fn resolve_skill_from_top_level_subdir() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Skill directly at {skill_id}/
        write_skill_md(&repo.path().join("my-skill"), "My Skill");

        let result =
            super::resolve_and_copy_skill(repo.path(), target.path(), Some("my-skill"), "")
                .unwrap();
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

        let result =
            super::resolve_and_copy_skill(repo.path(), target.path(), Some("whatever-id"), "")
                .unwrap();
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
        let result =
            super::resolve_and_copy_skill(repo.path(), target.path(), Some("wrong-id"), "");
        assert!(
            result.is_err(),
            "Should error when skill_id doesn't match and sub-skills exist"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("wrong-id"),
            "Error should mention the requested skill_id"
        );
    }

    #[test]
    fn resolve_wrong_skill_id_with_subdir_skills_at_top_level() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Root SKILL.md + top-level subdirectory skill (not under skills/)
        write_skill_md(repo.path(), "Root Skill");
        write_skill_md(&repo.path().join("other-skill"), "Other Skill");

        let result =
            super::resolve_and_copy_skill(repo.path(), target.path(), Some("nonexistent"), "");
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
        )
        .unwrap();

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("evil"), "");
        assert!(
            result.is_err(),
            "Should reject path traversal in skill name"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid") || err.contains("path") || err.contains("traversal"),
            "Error should mention path issue, got: {}",
            err
        );
    }

    #[test]
    fn resolve_skill_rejects_traversal_in_skill_id() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        write_skill_md(repo.path(), "Normal Skill");

        let result =
            super::resolve_and_copy_skill(repo.path(), target.path(), Some("../../../etc"), "");
        assert!(result.is_err(), "Should reject path traversal in skill_id");
    }

    #[test]
    fn resolve_generic_no_skill_id_picks_root() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        write_skill_md(repo.path(), "Root Skill");

        let result = super::resolve_and_copy_skill(
            repo.path(),
            target.path(),
            None,
            "https://github.com/user/repo.git",
        )
        .unwrap();
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
        let result =
            super::resolve_and_copy_skill(repo.path(), target.path(), Some(""), "").unwrap();
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
        Command::new("git")
            .args(["init", "--bare"])
            .arg(bare.path())
            .output()
            .unwrap();

        // Clone, commit, push
        Command::new("git")
            .args([
                "clone",
                &bare.path().to_string_lossy(),
                &work.path().to_string_lossy(),
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &work.path().to_string_lossy(),
                "checkout",
                "-b",
                branch,
            ])
            .output()
            .unwrap();
        std::fs::write(work.path().join("README.md"), "hello").unwrap();
        Command::new("git")
            .args(["-C", &work.path().to_string_lossy(), "add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &work.path().to_string_lossy(),
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@test.com",
                "commit",
                "-m",
                "init",
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &work.path().to_string_lossy(),
                "push",
                "origin",
                branch,
            ])
            .output()
            .unwrap();

        let out = Command::new("git")
            .args(["-C", &work.path().to_string_lossy(), "rev-parse", "HEAD"])
            .output()
            .unwrap();
        let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();

        (bare, hash)
    }

    /// Helper: push a new commit to an existing bare repo on the given branch
    fn push_new_commit(bare: &Path, branch: &str) -> String {
        let work = TempDir::new().unwrap();
        Command::new("git")
            .args([
                "clone",
                "-b",
                branch,
                &bare.to_string_lossy(),
                &work.path().to_string_lossy(),
            ])
            .output()
            .unwrap();
        std::fs::write(work.path().join("update.txt"), "updated").unwrap();
        Command::new("git")
            .args(["-C", &work.path().to_string_lossy(), "add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &work.path().to_string_lossy(),
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@test.com",
                "commit",
                "-m",
                "update",
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &work.path().to_string_lossy(),
                "push",
                "origin",
                branch,
            ])
            .output()
            .unwrap();

        let out = Command::new("git")
            .args(["-C", &work.path().to_string_lossy(), "rev-parse", "HEAD"])
            .output()
            .unwrap();
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No main or master branch found")
        );
    }

    /// Convert a local path to a file:// URL for check_update tests (which validate URLs).
    fn file_url(path: &Path) -> String {
        format!("file://{}", path.to_string_lossy())
    }

    #[test]
    fn check_update_same_hash_is_up_to_date() {
        let (bare, hash) = create_bare_repo("main");
        let meta = make_meta(&file_url(bare.path()), Some(&hash));
        match check_update(&meta) {
            UpdateStatus::UpToDate { remote_hash } => assert_eq!(remote_hash, hash),
            other => panic!("Expected UpToDate, got {:?}", other),
        }
    }

    #[test]
    fn check_update_different_hash_is_update_available() {
        let (bare, hash) = create_bare_repo("main");
        let meta = make_meta(&file_url(bare.path()), Some(&hash));

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
        let meta = make_meta(&file_url(bare.path()), None);
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
        let meta = make_meta(&file_url(bare.path()), Some(&hash));
        match check_update(&meta) {
            UpdateStatus::UpToDate { remote_hash } => assert_eq!(remote_hash, hash),
            other => panic!(
                "Expected UpToDate for non-main/master repo, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn check_update_prefers_url_resolved_over_url() {
        let (bare, hash) = create_bare_repo("main");
        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some("https://invalid-url-should-not-be-used.example.com/repo.git".into()),
            url_resolved: Some(file_url(bare.path())),
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
    fn check_update_rejects_bare_path_url() {
        let (bare, hash) = create_bare_repo("main");
        // Bare path (no protocol) should be rejected by validate_git_url
        let meta = make_meta(&bare.path().to_string_lossy(), Some(&hash));
        match check_update(&meta) {
            UpdateStatus::Error { message } => assert!(message.contains("Invalid git URL")),
            other => panic!("Expected Error for bare path, got {:?}", other),
        }
    }

    #[test]
    fn check_update_after_update_cycle_is_consistent() {
        // Simulate full cycle: install → check (up-to-date) → remote updates → check (available) → update → check (up-to-date)
        let (bare, hash1) = create_bare_repo("main");
        let url = file_url(bare.path());

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

    #[test]
    fn test_redact_mcp_env() {
        let entry = serde_json::json!({
            "command": "npx",
            "args": ["-y", "@example/server"],
            "env": {
                "API_KEY": "sk-secret-123",
                "GITHUB_TOKEN": "ghp_abcdef"
            }
        });
        let redacted = super::redact_mcp_env(&entry);
        assert_eq!(redacted["command"], "npx");
        assert_eq!(redacted["args"][0], "-y");
        assert_eq!(redacted["env"]["API_KEY"], "<redacted>");
        assert_eq!(redacted["env"]["GITHUB_TOKEN"], "<redacted>");
    }

    #[test]
    fn test_redact_mcp_env_no_env() {
        let entry = serde_json::json!({
            "command": "npx",
            "args": ["-y", "@example/server"]
        });
        let redacted = super::redact_mcp_env(&entry);
        assert_eq!(redacted, entry); // No change when no env
    }

    #[test]
    fn test_redact_mcp_env_empty_env() {
        let entry = serde_json::json!({
            "command": "npx",
            "env": {}
        });
        let redacted = super::redact_mcp_env(&entry);
        assert_eq!(redacted["env"], serde_json::json!({}));
    }

    #[test]
    fn test_redact_mcp_env_preserves_path() {
        // PATH is auto-injected (e.g. for antigravity) and is not a secret —
        // it must round-trip on disable→enable while other env values are redacted.
        let entry = serde_json::json!({
            "command": "/opt/homebrew/bin/npx",
            "args": ["-y", "@modelcontextprotocol/server-memory"],
            "env": {
                "PATH": "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin",
                "API_KEY": "sk-secret-123"
            }
        });
        let redacted = super::redact_mcp_env(&entry);
        assert_eq!(
            redacted["env"]["PATH"],
            "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin",
            "PATH must be preserved verbatim, not redacted"
        );
        assert_eq!(redacted["env"]["API_KEY"], "<redacted>");
    }

    #[cfg(unix)]
    #[test]
    fn test_copy_dir_contents_skips_symlinks_with_recheck() {
        // Verify copy_dir_contents uses symlink_metadata to skip symlinks,
        // closing the TOCTOU gap between readdir and copy.
        let src = TempDir::new().unwrap();
        std::fs::write(src.path().join("SKILL.md"), "# Test").unwrap();
        std::fs::write(src.path().join("helper.py"), "pass").unwrap();

        // Create a symlink to an outside file
        let outside = TempDir::new().unwrap();
        std::fs::write(outside.path().join("secret"), "TOP SECRET").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret"),
            src.path().join("stolen"),
        )
        .unwrap();

        let dst = TempDir::new().unwrap();
        let dst_dir = dst.path().join("result");
        copy_dir_contents(src.path(), &dst_dir).unwrap();

        assert!(dst_dir.join("SKILL.md").exists());
        assert!(dst_dir.join("helper.py").exists());
        // Symlink should NOT be copied
        assert!(!dst_dir.join("stolen").exists());
    }

    // -----------------------------------------------------------------------
    // Issue #16: plugin toggle end-to-end scenarios
    // -----------------------------------------------------------------------

    fn claude_env(dir: &std::path::Path) -> (Vec<Box<dyn adapter::AgentAdapter>>, std::path::PathBuf) {
        let claude_dir = dir.join(".claude");
        std::fs::create_dir_all(claude_dir.join("plugins")).unwrap();
        let settings = claude_dir.join("settings.json");
        let adapter = adapter::claude::ClaudeAdapter::with_home(dir.to_path_buf());
        (vec![Box::new(adapter)], settings)
    }

    fn plugin_ext(id: &str) -> Extension {
        Extension {
            id: id.into(),
            kind: ExtensionKind::Plugin,
            name: "test-plugin".into(),
            description: "Plugin from marketplace".into(),
            source: Source {
                origin: SourceOrigin::Agent,
                url: None,
                version: None,
                commit_hash: None,
            },
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
        }
    }

    /// Scenario A: Normal flow — disable plugin via HK, then re-enable.
    /// Single agent (Claude only). Uses scanner to get real stable IDs.
    /// With native toggle, disable sets enabledPlugins to false (no disabled_config).
    #[test]
    fn test_issue16_normal_disable_reenable() {
        let dir = TempDir::new().unwrap();
        let store = crate::store::Store::open(&dir.path().join("test.db")).unwrap();
        let (adapters, settings) = claude_env(dir.path());

        // Claude has the plugin enabled
        std::fs::write(
            &settings,
            r#"{"enabledPlugins":{"test-plugin@marketplace":true}}"#,
        ).unwrap();
        std::fs::write(
            dir.path().join(".claude/plugins/installed_plugins.json"),
            r#"{"plugins":{"test-plugin@marketplace":[{"installPath":"/tmp/p/1.0","installedAt":"2026-01-01T00:00:00Z"}]}}"#,
        ).unwrap();

        // Use scanner to get the real extension with correct stable ID
        let scanned = scanner::scan_plugins(&*adapters[0]);
        assert_eq!(scanned.len(), 1);
        assert!(scanned[0].enabled, "Plugin should be enabled (in enabledPlugins)");
        store.sync_extensions(&scanned).unwrap();
        let ext_id = scanned[0].id.clone();

        // Disable via HK — native toggle sets enabledPlugins to false
        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, false);
        assert!(r.is_ok(), "disable failed: {:?}", r.err());

        assert!(!store.get_extension(&ext_id).unwrap().unwrap().enabled);
        // Native toggle: no disabled_config needed, settings.json has false
        let s: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(s["enabledPlugins"]["test-plugin@marketplace"], false);

        // Re-enable via HK — native toggle sets enabledPlugins to true
        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, true);
        assert!(r.is_ok(), "re-enable failed: {:?}", r.err());

        assert!(store.get_extension(&ext_id).unwrap().unwrap().enabled);
        let s: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(s["enabledPlugins"]["test-plugin@marketplace"], true);
    }

    /// Claude: enable a scanner-disabled plugin (no disabled_config) should succeed
    /// by setting enabledPlugins value to true.
    #[test]
    fn test_issue16_scanner_disabled_plugin_enable_succeeds() {
        let dir = TempDir::new().unwrap();
        let store = crate::store::Store::open(&dir.path().join("test.db")).unwrap();
        let (adapters, settings) = claude_env(dir.path());

        // Plugin in installed_plugins but enabledPlugins has it as false
        std::fs::write(&settings, r#"{"enabledPlugins":{"test-plugin@marketplace":false}}"#).unwrap();
        std::fs::write(
            dir.path().join(".claude/plugins/installed_plugins.json"),
            r#"{"plugins":{"test-plugin@marketplace":[{"installPath":"/tmp/p/1.0","installedAt":"2026-01-01T00:00:00Z"}]}}"#,
        ).unwrap();

        let scanned = scanner::scan_plugins(&*adapters[0]);
        assert!(!scanned[0].enabled);
        store.sync_extensions(&scanned).unwrap();
        let ext_id = scanned[0].id.clone();

        // Enable should succeed — just set enabledPlugins value to true
        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, true);
        assert!(r.is_ok(), "enable should succeed: {:?}", r.err());

        // Verify settings.json was updated
        let s: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(s["enabledPlugins"]["test-plugin@marketplace"], true);
    }

    /// Claude: disable→enable roundtrip using native true/false (no disabled_config needed)
    #[test]
    fn test_issue16_claude_native_toggle_roundtrip() {
        let dir = TempDir::new().unwrap();
        let store = crate::store::Store::open(&dir.path().join("test.db")).unwrap();
        let (adapters, settings) = claude_env(dir.path());

        std::fs::write(&settings, r#"{"enabledPlugins":{"test-plugin@marketplace":true}}"#).unwrap();
        std::fs::write(
            dir.path().join(".claude/plugins/installed_plugins.json"),
            r#"{"plugins":{"test-plugin@marketplace":[{"installPath":"/tmp/p/1.0","installedAt":"2026-01-01T00:00:00Z"}]}}"#,
        ).unwrap();

        let scanned = scanner::scan_plugins(&*adapters[0]);
        store.sync_extensions(&scanned).unwrap();
        let ext_id = scanned[0].id.clone();

        // Disable
        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, false);
        assert!(r.is_ok(), "disable failed: {:?}", r.err());
        let s: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(s["enabledPlugins"]["test-plugin@marketplace"], false);

        // Re-enable
        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, true);
        assert!(r.is_ok(), "re-enable failed: {:?}", r.err());
        let s: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(s["enabledPlugins"]["test-plugin@marketplace"], true);
    }

    #[test]
    fn test_claude_plugin_toggle_no_source() {
        let dir = TempDir::new().unwrap();
        let store = crate::store::Store::open(&dir.path().join("test.db")).unwrap();
        let (adapters, settings) = claude_env(dir.path());

        std::fs::write(&settings, r#"{"enabledPlugins":{"local-plugin":true}}"#).unwrap();
        std::fs::write(
            dir.path().join(".claude/plugins/installed_plugins.json"),
            r#"{"plugins":{"local-plugin":[{"installPath":"/tmp/p/1.0","installedAt":"2026-01-01T00:00:00Z"}]}}"#,
        ).unwrap();

        let scanned = scanner::scan_plugins(&*adapters[0]);
        assert_eq!(scanned.len(), 1);
        store.sync_extensions(&scanned).unwrap();
        let ext_id = scanned[0].id.clone();

        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, false);
        assert!(r.is_ok(), "disable no-source plugin failed: {:?}", r.err());
        let s: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(s["enabledPlugins"]["local-plugin"], false);

        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, true);
        assert!(r.is_ok(), "enable no-source plugin failed: {:?}", r.err());
        let s: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(s["enabledPlugins"]["local-plugin"], true);
    }

    #[test]
    fn test_codex_plugin_toggle_uses_config_toml() {
        let dir = TempDir::new().unwrap();
        let store = crate::store::Store::open(&dir.path().join("test.db")).unwrap();
        let codex_dir = dir.path().join(".codex");
        std::fs::create_dir_all(codex_dir.join("plugins/cache/mp/my-plugin/1.0.0/.codex-plugin")).unwrap();
        std::fs::write(
            codex_dir.join("plugins/cache/mp/my-plugin/1.0.0/.codex-plugin/plugin.json"),
            r#"{"name":"my-plugin"}"#,
        ).unwrap();
        std::fs::write(codex_dir.join("config.toml"), "").unwrap();

        let codex_adapter = adapter::codex::CodexAdapter::with_home(dir.path().to_path_buf());
        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![Box::new(codex_adapter)];

        let scanned = scanner::scan_plugins(&*adapters[0]);
        assert_eq!(scanned.len(), 1);
        store.sync_extensions(&scanned).unwrap();
        let ext_id = scanned[0].id.clone();

        // Disable via toggle
        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, false);
        assert!(r.is_ok(), "codex disable failed: {:?}", r.err());

        let config: toml::Table = std::fs::read_to_string(codex_dir.join("config.toml"))
            .unwrap().parse().unwrap();
        let plugin_enabled = config["plugins"]["my-plugin@mp"]["enabled"].as_bool().unwrap();
        assert!(!plugin_enabled, "config.toml should show enabled=false");

        // Re-enable
        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, true);
        assert!(r.is_ok(), "codex re-enable failed: {:?}", r.err());

        let config: toml::Table = std::fs::read_to_string(codex_dir.join("config.toml"))
            .unwrap().parse().unwrap();
        let plugin_enabled = config["plugins"]["my-plugin@mp"]["enabled"].as_bool().unwrap();
        assert!(plugin_enabled, "config.toml should show enabled=true");
    }

    // -----------------------------------------------------------------------
    // Task 3: manifest candidate list + silent failure
    // -----------------------------------------------------------------------

    #[test]
    fn test_copilot_vscode_plugin_toggle_uses_state_db() {
        let dir = TempDir::new().unwrap();
        let store = crate::store::Store::open(&dir.path().join("test.db")).unwrap();

        // Set up a Copilot VS Code agent plugin with .github/plugin/plugin.json
        let plugin_dir = dir.path().join(".vscode/agent-plugins/github.com/org/repo/plugins/my-plugin");
        let manifest_dir = plugin_dir.join(".github/plugin");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(manifest_dir.join("plugin.json"), r#"{"name":"my-plugin"}"#).unwrap();
        let plugin_uri = format!("file://{}", plugin_dir.to_string_lossy());

        // installed.json for copilot read_plugins
        let vscode_dir = dir.path().join(".vscode/agent-plugins");
        std::fs::write(
            vscode_dir.join("installed.json"),
            serde_json::json!({
                "installed": [{
                    "marketplace": "github.com",
                    "pluginUri": &plugin_uri
                }]
            }).to_string(),
        ).unwrap();

        // Create VS Code state.vscdb with the enablement table
        #[cfg(target_os = "macos")]
        let vscode_user = dir.path().join("Library/Application Support/Code/User");
        #[cfg(target_os = "windows")]
        let vscode_user = dir.path().join("AppData/Roaming/Code/User");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let vscode_user = dir.path().join(".config/Code/User");
        let state_db_dir = vscode_user.join("globalStorage");
        std::fs::create_dir_all(&state_db_dir).unwrap();
        let state_db = state_db_dir.join("state.vscdb");
        {
            let conn = rusqlite::Connection::open(&state_db).unwrap();
            conn.execute("CREATE TABLE IF NOT EXISTS ItemTable (key TEXT UNIQUE, value TEXT)", []).unwrap();
            // Plugin starts enabled
            let val = serde_json::json!([[&plugin_uri, true]]).to_string();
            conn.execute("INSERT INTO ItemTable (key, value) VALUES ('agentPlugins.enablement', ?1)", [&val]).unwrap();
        }

        let adapter = adapter::copilot::CopilotAdapter::with_home(dir.path().to_path_buf());
        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![Box::new(adapter)];

        let scanned = scanner::scan_plugins(&*adapters[0]);
        assert!(!scanned.is_empty(), "Scanner should find the plugin");
        assert!(scanned[0].enabled, "Plugin should be enabled initially");
        store.sync_extensions(&scanned).unwrap();
        let ext_id = scanned[0].id.clone();

        // Disable — should write false to state.vscdb, NOT rename manifest
        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, false);
        assert!(r.is_ok(), "disable failed: {:?}", r.err());

        // Manifest should still exist (not renamed)
        assert!(
            manifest_dir.join("plugin.json").exists(),
            "Manifest should NOT be renamed for VS Code plugins"
        );
        // state.vscdb should show disabled
        {
            let conn = rusqlite::Connection::open(&state_db).unwrap();
            let val: String = conn.query_row(
                "SELECT value FROM ItemTable WHERE key = 'agentPlugins.enablement'", [], |r| r.get(0)
            ).unwrap();
            let entries: Vec<(String, bool)> = serde_json::from_str(&val).unwrap();
            assert!(!entries[0].1, "state.vscdb should show plugin disabled");
        }

        // Re-enable
        let r = toggle_extension_with_adapters(&store, &adapters, &ext_id, true);
        assert!(r.is_ok(), "re-enable failed: {:?}", r.err());
        {
            let conn = rusqlite::Connection::open(&state_db).unwrap();
            let val: String = conn.query_row(
                "SELECT value FROM ItemTable WHERE key = 'agentPlugins.enablement'", [], |r| r.get(0)
            ).unwrap();
            let entries: Vec<(String, bool)> = serde_json::from_str(&val).unwrap();
            assert!(entries[0].1, "state.vscdb should show plugin enabled");
        }
    }

    #[test]
    fn test_copilot_vscode_scanner_reads_disabled_from_state_db() {
        let dir = TempDir::new().unwrap();

        // Set up VS Code plugin
        let plugin_dir = dir.path().join(".vscode/agent-plugins/github.com/org/repo/plugins/my-plugin");
        std::fs::create_dir_all(plugin_dir.join(".github/plugin")).unwrap();
        std::fs::write(plugin_dir.join(".github/plugin/plugin.json"), r#"{"name":"my-plugin"}"#).unwrap();
        let plugin_uri = format!("file://{}", plugin_dir.to_string_lossy());
        let vscode_dir = dir.path().join(".vscode/agent-plugins");
        std::fs::write(vscode_dir.join("installed.json"), serde_json::json!({
            "installed": [{"marketplace": "github.com", "pluginUri": &plugin_uri}]
        }).to_string()).unwrap();

        // state.vscdb with plugin DISABLED
        #[cfg(target_os = "macos")]
        let vscode_user = dir.path().join("Library/Application Support/Code/User");
        #[cfg(target_os = "windows")]
        let vscode_user = dir.path().join("AppData/Roaming/Code/User");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let vscode_user = dir.path().join(".config/Code/User");
        std::fs::create_dir_all(vscode_user.join("globalStorage")).unwrap();
        {
            let conn = rusqlite::Connection::open(vscode_user.join("globalStorage/state.vscdb")).unwrap();
            conn.execute("CREATE TABLE IF NOT EXISTS ItemTable (key TEXT UNIQUE, value TEXT)", []).unwrap();
            let val = serde_json::json!([[&plugin_uri, false]]).to_string();
            conn.execute("INSERT INTO ItemTable (key, value) VALUES ('agentPlugins.enablement', ?1)", [&val]).unwrap();
        }

        let adapter = adapter::copilot::CopilotAdapter::with_home(dir.path().to_path_buf());
        let scanned = scanner::scan_plugins(&adapter);
        assert_eq!(scanned.len(), 1);
        assert!(!scanned[0].enabled, "Scanner should detect VS Code disabled state from state.vscdb");
    }

    #[test]
    fn test_toggle_plugin_errors_when_no_manifest_found() {
        let dir = TempDir::new().unwrap();
        let store = crate::store::Store::open(&dir.path().join("test.db")).unwrap();

        // Plugin directory exists but has NO manifest file at all
        let plugin_dir = dir.path().join(".cursor/plugins/cache/mp/ghost-plugin/1.0.0");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        // No plugin.json, no .cursor-plugin/plugin.json — nothing

        let mut ext = plugin_ext("ghost-1");
        ext.agents = vec!["cursor".into()];
        ext.enabled = true;
        store.insert_extension(&ext).unwrap();

        let adapter = adapter::cursor::CursorAdapter::with_home(dir.path().to_path_buf());
        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![Box::new(adapter)];

        // Disable should fail because there's no manifest to rename
        let r = toggle_extension_with_adapters(&store, &adapters, "ghost-1", false);
        assert!(r.is_err(), "disable with no manifest should fail");
    }
}
