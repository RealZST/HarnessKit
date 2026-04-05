use hk_core::{adapter, deployer, manager, models::*, scanner, HkError};
use tauri::State;
use super::AppState;
use super::helpers::{find_skill_by_id, is_path_within_allowed_dirs, FileEntry, list_dir_entries};

#[derive(serde::Serialize)]
pub struct ExtensionContent {
    pub content: String,
    pub path: Option<String>,
    /// If the extension directory/file is a symlink, the resolved target path.
    pub symlink_target: Option<String>,
}

#[tauri::command]
pub fn list_extensions(
    state: State<AppState>,
    kind: Option<String>,
    agent: Option<String>,
) -> Result<Vec<Extension>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let kind_filter = kind.as_deref().and_then(|k| k.parse().ok());
    store.list_extensions(kind_filter, agent.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_extension(state: State<'_, AppState>, id: String, enabled: bool) -> Result<(), HkError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock().map_err(|e| HkError::Internal(e.to_string()))?;
        manager::toggle_extension(&store, &id, enabled)
            .map_err(|e| HkError::Internal(e.to_string()))
    }).await.map_err(|e| HkError::Internal(e.to_string()))?
}

#[tauri::command]
pub fn delete_extension(state: State<AppState>, id: String) -> Result<(), String> {
    // Load extension metadata first
    let ext = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.get_extension(&id).map_err(|e| e.to_string())?
            .ok_or_else(|| "Extension not found".to_string())?
    };

    let adapters = adapter::all_adapters();

    // Actually remove from disk/config based on extension kind
    match ext.kind {
        ExtensionKind::Skill => {
            // Delete skill file or directory
            if let Some(loc) = find_skill_by_id(&adapters, &id, &ext.agents) {
                if loc.entry_path.is_dir() {
                    std::fs::remove_dir_all(&loc.entry_path)
                        .map_err(|e| format!("Failed to delete skill directory: {}", e))?;
                } else {
                    std::fs::remove_file(&loc.entry_path)
                        .map_err(|e| format!("Failed to delete skill file: {}", e))?;
                }
            }
        }
        ExtensionKind::Mcp => {
            // Remove MCP server entry from agent config
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for server in adapter.read_mcp_servers() {
                    if scanner::stable_id_for(&server.name, "mcp", adapter.name()) == id {
                        let config_path = adapter.mcp_config_path();
                        deployer::remove_mcp_server(&config_path, &server.name, adapter.mcp_format())
                            .map_err(|e| format!("Failed to remove MCP server config: {}", e))?;
                    }
                }
            }
        }
        ExtensionKind::Hook => {
            // Remove hook entry from agent config
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for hook in adapter.read_hooks() {
                    let hook_name = format!("{}:{}:{}", hook.event, hook.matcher.as_deref().unwrap_or("*"), hook.command);
                    if scanner::stable_id_for(&hook_name, "hook", adapter.name()) == id {
                        let config_path = adapter.hook_config_path();
                        deployer::remove_hook(&config_path, &hook.event, hook.matcher.as_deref(), &hook.command, adapter.hook_format())
                            .map_err(|e| format!("Failed to remove hook config: {}", e))?;
                    }
                }
            }
        }
        ExtensionKind::Cli => {
            // CLI uninstall not yet implemented
        }
        ExtensionKind::Plugin => {
            // Delete plugin files/config from disk
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for plugin in adapter.read_plugins() {
                    if scanner::stable_id_for(&format!("{}:{}", plugin.name, plugin.source), "plugin", adapter.name()) == id {
                        if adapter.name() == "claude" {
                            // For Claude: remove from enabledPlugins in settings.json
                            let config_path = adapter.mcp_config_path();
                            // Reconstruct the key format "name@source"
                            let plugin_key = if plugin.source.is_empty() {
                                plugin.name.clone()
                            } else {
                                format!("{}@{}", plugin.name, plugin.source)
                            };
                            deployer::remove_plugin_entry(&config_path, &plugin_key)
                                .map_err(|e| format!("Failed to remove plugin entry: {}", e))?;
                        } else if let Some(ref path) = plugin.path {
                            // For Codex: path points to a version dir inside
                            // cache/{marketplace}/{plugin}/{version}/ — delete the plugin-name
                            // parent so every version is removed and the plugin doesn't "come
                            // back" on re-scan.
                            // For Cursor/others: path already points to the plugin dir itself,
                            // so we delete it directly.
                            let target = if adapter.name() == "codex" {
                                // Go up one level from the version dir to the plugin-name dir,
                                // but guard against the parent being a root-level directory
                                if let Some(parent) = path.parent() {
                                    if parent.file_name().map(|n| n != "cache" && n != "plugins").unwrap_or(false) {
                                        parent
                                    } else {
                                        path.as_path()
                                    }
                                } else {
                                    path.as_path()
                                }
                            } else {
                                path.as_path()
                            };
                            if target.is_dir() {
                                std::fs::remove_dir_all(target)
                                    .map_err(|e| format!("Failed to delete plugin directory: {}", e))?;
                            } else if target.is_file() {
                                std::fs::remove_file(target)
                                    .map_err(|e| format!("Failed to delete plugin file: {}", e))?;
                            }
                        }
                    }
                }
            }
        }
    }

    // Remove from database (only after successful disk/config deletion)
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.delete_extension(&id).map_err(|e| e.to_string())
}

/// List files in a skill directory as a shallow tree (2 levels deep).
#[tauri::command]
pub fn list_skill_files(state: State<AppState>, path: String) -> Result<Vec<FileEntry>, String> {
    let root = std::path::Path::new(&path);
    if !root.is_dir() {
        return Err("Path is not a directory".into());
    }
    if !is_path_within_allowed_dirs(root, &state)? {
        return Err("Path is not within a known agent or project directory".into());
    }
    list_dir_entries(root, 0)
}

/// Open a file or directory in the system's default application.
#[tauri::command]
pub fn open_in_system(state: State<AppState>, path: String) -> Result<(), String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err("Path does not exist".into());
    }
    if !is_path_within_allowed_dirs(file_path, &state)? {
        return Err("Path is not within a known agent or project directory".into());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }
    Ok(())
}

/// Reveal a file or directory in the system file manager (Finder / Explorer).
#[tauri::command]
pub fn reveal_in_file_manager(state: State<AppState>, path: String) -> Result<(), String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err("Path does not exist".into());
    }
    if !is_path_within_allowed_dirs(file_path, &state)? {
        return Err("Path is not within a known agent or project directory".into());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to reveal: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path))
            .spawn()
            .map_err(|e| format!("Failed to reveal: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        // Most Linux file managers support revealing via parent directory
        let parent = file_path.parent().unwrap_or(file_path);
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("Failed to reveal: {}", e))?;
    }
    Ok(())
}

/// For a given skill name, find all physical paths where it exists across all agents.
/// Returns Vec<(agent_name, path)> for display in the detail panel.
#[tauri::command]
pub fn get_skill_locations(name: String) -> Vec<(String, String, Option<String>)> {
    let adapters = adapter::all_adapters();
    scanner::skill_locations(&name, &adapters)
        .into_iter()
        .map(|(agent, path)| {
            // Check if the path itself or its parent skill_dir is a symlink
            let symlink_target = if path.symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false) {
                std::fs::read_link(&path).ok().map(|t| t.to_string_lossy().to_string())
            } else {
                path.parent()
                    .filter(|p| p.symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false))
                    .and_then(|p| std::fs::read_link(p).ok())
                    .map(|t| t.join(path.file_name().unwrap_or_default()).to_string_lossy().to_string())
            };
            (agent, path.to_string_lossy().to_string(), symlink_target)
        })
        .collect()
}

#[tauri::command]
pub fn get_extension_content(state: State<AppState>, id: String) -> Result<ExtensionContent, String> {
    // Read extension metadata and release lock before file I/O
    let ext = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.get_extension(&id).map_err(|e| e.to_string())?
            .ok_or_else(|| "Extension not found".to_string())?
    };

    match ext.kind {
        ExtensionKind::Skill => {
            let adapters = adapter::all_adapters();
            if let Some(loc) = find_skill_by_id(&adapters, &id, &ext.agents) {
                let dir = if loc.entry_path.is_dir() {
                    loc.entry_path.to_string_lossy().to_string()
                } else {
                    loc.skill_file.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
                };
                // Detect symlink: check entry itself, then parent skill_dir
                let dir_symlink_target = if loc.skill_dir.symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false) {
                    std::fs::read_link(&loc.skill_dir).ok()
                } else {
                    None
                };
                let symlink_target = if loc.entry_path.symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false) {
                    // Entry itself is a symlink
                    std::fs::read_link(&loc.entry_path).ok().map(|t| t.to_string_lossy().to_string())
                } else if let Some(ref resolved_dir) = dir_symlink_target {
                    // Entry is real but accessed through a symlinked parent dir
                    let entry_name = loc.entry_path.file_name().unwrap_or_default();
                    Some(resolved_dir.join(entry_name).to_string_lossy().to_string())
                } else {
                    None
                };
                let content = std::fs::read_to_string(&loc.skill_file).map_err(|e| e.to_string())?;
                Ok(ExtensionContent { content, path: Some(dir), symlink_target })
            } else {
                Err("Skill file not found".into())
            }
        }
        ExtensionKind::Mcp => {
            // Pull actual config from the adapter for rich detail
            let adapters = adapter::all_adapters();
            let mut fallback_config_path = None;
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                let config_path = adapter.mcp_config_path();
                if fallback_config_path.is_none() {
                    fallback_config_path = Some(config_path.to_string_lossy().to_string());
                }
                for server in adapter.read_mcp_servers() {
                    if scanner::stable_id_for(&server.name, "mcp", adapter.name()) == id {
                        let mut lines = vec![
                            format!("Command: {}", server.command),
                        ];
                        if !server.args.is_empty() {
                            lines.push(format!("Args: {}", server.args.join(" ")));
                        }
                        if !server.env.is_empty() {
                            lines.push("Environment:".into());
                            for k in server.env.keys() {
                                lines.push(format!("  {} = ****", k));
                            }
                        }
                        return Ok(ExtensionContent {
                            content: lines.join("\n"),
                            path: Some(config_path.to_string_lossy().to_string()),
                            symlink_target: None,
                        });
                    }
                }
            }
            // Disabled MCP: still show the config path where it was removed from
            Ok(ExtensionContent {
                content: ext.description,
                path: fallback_config_path,
                symlink_target: None,
            })
        }
        ExtensionKind::Hook => {
            let adapters = adapter::all_adapters();
            let mut fallback_config_path = None;
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                let config_path = adapter.hook_config_path();
                if fallback_config_path.is_none() {
                    fallback_config_path = Some(config_path.to_string_lossy().to_string());
                }
                for hook in adapter.read_hooks() {
                    let hook_name = format!("{}:{}:{}", hook.event, hook.matcher.as_deref().unwrap_or("*"), hook.command);
                    if scanner::stable_id_for(&hook_name, "hook", adapter.name()) == id {
                        let mut lines = vec![
                            format!("Event: {}", hook.event),
                        ];
                        if let Some(m) = &hook.matcher {
                            lines.push(format!("Matcher: {}", m));
                        }
                        lines.push(format!("Command: {}", hook.command));
                        return Ok(ExtensionContent {
                            content: lines.join("\n"),
                            path: Some(config_path.to_string_lossy().to_string()),
                            symlink_target: None,
                        });
                    }
                }
            }
            // Disabled hook: still show the config path
            Ok(ExtensionContent {
                content: ext.description,
                path: fallback_config_path,
                symlink_target: None,
            })
        }
        ExtensionKind::Plugin => {
            let adapters = adapter::all_adapters();
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for plugin in adapter.read_plugins() {
                    if scanner::stable_id_for(&format!("{}:{}", plugin.name, plugin.source), "plugin", adapter.name()) == id {
                        let path_str = plugin.path.as_ref()
                            .map(|p| p.to_string_lossy().to_string());
                        return Ok(ExtensionContent {
                            content: ext.description,
                            path: path_str,
                            symlink_target: None,
                        });
                    }
                }
            }
            Ok(ExtensionContent {
                content: ext.description,
                path: None,
                symlink_target: None,
            })
        }
        ExtensionKind::Cli => {
            Ok(ExtensionContent {
                content: ext.description,
                path: None,
                symlink_target: None,
            })
        }
    }
}

#[tauri::command]
pub fn scan_and_sync(state: State<AppState>) -> Result<usize, String> {
    // Scan filesystem WITHOUT holding the lock — this is the slow part
    let adapters = adapter::all_adapters();
    let extensions = scanner::scan_all(&adapters);
    let count = extensions.len();

    // Lock briefly for a single transactional write (fast — one fsync total)
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.sync_extensions(&extensions).map_err(|e| e.to_string())?;
    Ok(count)
}

#[tauri::command]
pub fn get_cached_update_statuses(state: State<AppState>) -> Result<Vec<(String, UpdateStatus)>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let extensions = store.list_extensions(None, None).map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for ext in extensions {
        let Some(meta) = ext.install_meta else { continue };
        // Only include extensions that have been checked before
        if meta.checked_at.is_none() { continue; }
        let status = match (meta.revision.as_deref(), meta.remote_revision.as_deref()) {
            (Some(local), Some(remote)) => {
                if local.starts_with(remote) || remote.starts_with(local) {
                    UpdateStatus::UpToDate { remote_hash: remote.to_string() }
                } else {
                    UpdateStatus::UpdateAvailable { remote_hash: remote.to_string() }
                }
            }
            (None, Some(remote)) => {
                // No local revision (pre-existing skill) — treat as update available
                UpdateStatus::UpdateAvailable { remote_hash: remote.to_string() }
            }
            _ => {
                if let Some(err) = meta.check_error {
                    UpdateStatus::Error { message: err }
                } else {
                    continue; // No remote_revision and no error — nothing to report
                }
            }
        };
        results.push((ext.id, status));
    }
    Ok(results)
}

#[tauri::command]
pub async fn check_updates(state: State<'_, AppState>) -> Result<Vec<(String, UpdateStatus)>, String> {
    let store_clone = state.store.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<(String, UpdateStatus)>, String> {
        // Read all extensions and release the lock before doing slow network calls
        type Updatable = Vec<(String, InstallMeta)>;
        type Unlinked = Vec<(String, String)>;
        let (updatable, unlinked): (Updatable, Unlinked) = {
            let store = store_clone.lock().map_err(|e| e.to_string())?;
            let extensions = store.list_extensions(None, None).map_err(|e| e.to_string())?;
            let mut has_meta = Vec::new();
            let mut no_meta = Vec::new();
            for e in extensions {
                if let Some(meta) = e.install_meta {
                    match meta.install_type.as_str() {
                        "git" | "marketplace" => has_meta.push((e.id, meta)),
                        _ => {}
                    }
                } else if e.kind == ExtensionKind::Skill {
                    no_meta.push((e.id, e.name));
                }
            }
            (has_meta, no_meta)
        };

        // Try to match unlinked skills against marketplace by name.
        // Only link when there is exactly one result with an exact name match.
        if !unlinked.is_empty() {
            let unique_names: std::collections::HashSet<&str> =
                unlinked.iter().map(|(_, name)| name.as_str()).collect();

            // For each unique name, search marketplace and resolve remote revision
            let mut matched: std::collections::HashMap<String, (String, String, Option<String>)> = std::collections::HashMap::new();
            for name in &unique_names {
                if let Ok(results) = hk_core::marketplace::search_skills(name, 5) {
                    let exact: Vec<_> = results.iter()
                        .filter(|r| r.name.eq_ignore_ascii_case(name))
                        .collect();
                    if exact.len() == 1 {
                        let item = exact[0];
                        let git_url = hk_core::marketplace::git_url_for_source(&item.source);
                        // Get current remote HEAD as baseline — so we don't falsely
                        // show "update available" when we don't know the local version
                        let remote_rev = manager::get_remote_head(&git_url).ok();
                        matched.insert(
                            name.to_string(),
                            (git_url, item.skill_id.clone(), remote_rev),
                        );
                    }
                }
            }

            if !matched.is_empty() {
                let store = store_clone.lock().map_err(|e| e.to_string())?;
                let now = chrono::Utc::now();
                for (id, name) in &unlinked {
                    if let Some((git_url, skill_id, remote_rev)) = matched.get(name.as_str()) {
                        // Set revision = remote_rev as baseline: "assume local is at this version"
                        // Next check_updates will detect if remote moved past this point
                        let meta = InstallMeta {
                            install_type: "marketplace".into(),
                            url: Some(format!("{}/{}", &git_url.trim_end_matches(".git"), skill_id)),
                            url_resolved: Some(git_url.clone()),
                            branch: None,
                            subpath: if skill_id.is_empty() { None } else { Some(skill_id.clone()) },
                            revision: remote_rev.clone(),
                            remote_revision: remote_rev.clone(),
                            checked_at: Some(now),
                            check_error: None,
                        };
                        let _ = store.set_install_meta(id, &meta);
                        // Don't push to updatable — we already know the status (up-to-date)
                        // and don't need to call get_remote_head again
                    }
                }
            }
        }

        // Check each extension for updates (network-heavy: git ls-remote per extension)
        let statuses: Vec<_> = updatable
            .iter()
            .map(|(id, meta)| {
                let status = manager::check_update(meta);
                (id.clone(), meta.clone(), status)
            })
            .collect();

        // Persist check state
        let store = store_clone.lock().map_err(|e| e.to_string())?;
        let now = chrono::Utc::now();
        for (id, _meta, status) in &statuses {
            let (remote_rev, check_err) = match status {
                UpdateStatus::UpToDate { remote_hash } => (Some(remote_hash.as_str()), None),
                UpdateStatus::UpdateAvailable { remote_hash } => (Some(remote_hash.as_str()), None),
                UpdateStatus::Error { message } => (None, Some(message.as_str())),
            };
            let _ = store.update_check_state(id, remote_rev, now, check_err);
        }

        Ok(statuses.into_iter().map(|(id, _, status)| (id, status)).collect())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_extension(state: State<'_, AppState>, id: String) -> Result<manager::InstallResult, String> {
    let store_clone = state.store.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<manager::InstallResult, String> {
    let (ext, install_meta) = {
        let store = store_clone.lock().map_err(|e| e.to_string())?;
        let ext = store.get_extension(&id).map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Extension '{}' not found", id))?;
        let meta = ext.install_meta.clone()
            .ok_or("Extension has no install metadata — cannot update")?;
        match meta.install_type.as_str() {
            "git" | "marketplace" => {}
            _ => return Err(format!("Extensions with install type '{}' cannot be updated", meta.install_type)),
        }
        (ext, meta)
    };
    let url = install_meta.url_resolved.as_deref()
        .or(install_meta.url.as_deref())
        .ok_or("Extension has no remote URL")?;

    // Clone the repo once
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let clone_dir = temp.path().join("repo");
    let output = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, &clone_dir.to_string_lossy()])
        .output()
        .map_err(|e| format!("Failed to run git clone: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed: {}", stderr.trim()));
    }
    let revision = manager::capture_git_revision_pub(&clone_dir);

    let skill_name = &ext.name;
    let skill_source = manager::find_skill_in_repo(&clone_dir, skill_name)
        .ok_or_else(|| format!("Skill '{}' not found in repository", skill_name))?;

    // Find all installed paths (deduplicated) and copy the latest version to each
    let all_siblings: Vec<Extension> = {
        let store = store_clone.lock().map_err(|e| e.to_string())?;
        let all = store.list_extensions(Some(ext.kind), None).map_err(|e| e.to_string())?;
        all.into_iter()
            .filter(|e| e.name == ext.name && e.source_path.is_some())
            .collect()
    };

    let mut updated_dirs = std::collections::HashSet::new();
    for sibling in &all_siblings {
        let source_path = sibling.source_path.as_deref().unwrap();
        let skill_dir = std::path::Path::new(source_path).parent()
            .ok_or("Cannot determine skill directory from source path")?;
        if !updated_dirs.insert(skill_dir.to_string_lossy().to_string()) {
            continue;
        }
        hk_core::deployer::deploy_skill(&skill_source, skill_dir.parent().unwrap_or(skill_dir))
            .map_err(|e| e.to_string())?;
    }

    // Update install metadata for all siblings
    {
        let store = store_clone.lock().map_err(|e| e.to_string())?;
        let updated_meta = InstallMeta {
            revision: revision.clone().or(install_meta.revision.clone()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
            ..install_meta
        };
        for sibling in &all_siblings {
            let _ = store.set_install_meta(&sibling.id, &updated_meta);
        }
    }

    // Re-scan affected agents only and persist
    let adapters = adapter::all_adapters();
    let affected_agents: std::collections::HashSet<String> = all_siblings.iter()
        .flat_map(|s| s.agents.iter().cloned())
        .collect();
    {
        let store = store_clone.lock().map_err(|e| e.to_string())?;
        for a in &adapters {
            if affected_agents.contains(a.name()) {
                let exts = scanner::scan_adapter(a.as_ref());
                store.sync_extensions_for_agent(a.name(), &exts).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(manager::InstallResult {
        name: skill_name.clone(),
        was_update: true,
        revision,
    })
    }).await.map_err(|e| e.to_string())?
}
