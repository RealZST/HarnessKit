use super::AppState;
use super::helpers::{FileEntry, find_skill_by_id, is_path_within_allowed_dirs, list_dir_entries};
use hk_core::{HkError, deployer, manager, models::*, scanner};
use tauri::{Emitter, State};

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
) -> Result<Vec<Extension>, HkError> {
    let store = state.store.lock();
    let kind_filter = kind.as_deref().and_then(|k| k.parse().ok());
    store.list_extensions(kind_filter, agent.as_deref())
}

#[tauri::command]
pub async fn toggle_extension(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), HkError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        manager::toggle_extension(&store, &id, enabled)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn delete_extension(state: State<'_, AppState>, id: String) -> Result<(), HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Load extension metadata first
        let ext = {
            let store = store.lock();
            store
                .get_extension(&id)?
                .ok_or_else(|| HkError::NotFound("Extension not found".into()))?
        };

        // Actually remove from disk/config based on extension kind
        match ext.kind {
            ExtensionKind::Skill => {
                // Delete skill file or directory
                if let Some(loc) = find_skill_by_id(&adapters, &id, &ext.agents) {
                    if loc.entry_path.is_dir() {
                        std::fs::remove_dir_all(&loc.entry_path)?;
                    } else {
                        std::fs::remove_file(&loc.entry_path)?;
                    }
                }
            }
            ExtensionKind::Mcp => {
                // Remove MCP server entry from agent config
                for adapter in adapters.iter() {
                    if !ext.agents.contains(&adapter.name().to_string()) {
                        continue;
                    }
                    for server in adapter.read_mcp_servers() {
                        if scanner::stable_id_for(&server.name, "mcp", adapter.name()) == id {
                            let config_path = adapter.mcp_config_path();
                            deployer::remove_mcp_server(
                                &config_path,
                                &server.name,
                                adapter.mcp_format(),
                            )?;
                        }
                    }
                }
            }
            ExtensionKind::Hook => {
                // Remove hook entry from agent config
                for adapter in adapters.iter() {
                    if !ext.agents.contains(&adapter.name().to_string()) {
                        continue;
                    }
                    for hook in adapter.read_hooks() {
                        let hook_name = format!(
                            "{}:{}:{}",
                            hook.event,
                            hook.matcher.as_deref().unwrap_or("*"),
                            hook.command
                        );
                        if scanner::stable_id_for(&hook_name, "hook", adapter.name()) == id {
                            let config_path = adapter.hook_config_path();
                            deployer::remove_hook(
                                &config_path,
                                &hook.event,
                                hook.matcher.as_deref(),
                                &hook.command,
                                adapter.hook_format(),
                            )?;
                        }
                    }
                }
            }
            ExtensionKind::Cli => {
                // Child skills/MCPs are deleted separately by their own IDs.
                // This branch only runs for full CLI uninstall (parent record cleanup).
            }
            ExtensionKind::Plugin => {
                // Delete plugin files/config from disk
                for adapter in adapters.iter() {
                    if !ext.agents.contains(&adapter.name().to_string()) {
                        continue;
                    }
                    for plugin in adapter.read_plugins() {
                        if scanner::stable_id_for(
                            &format!("{}:{}", plugin.name, plugin.source),
                            "plugin",
                            adapter.name(),
                        ) != id
                        {
                            continue;
                        }
                        let plugin_key = if plugin.source.is_empty() {
                            plugin.name.clone()
                        } else {
                            format!("{}@{}", plugin.name, plugin.source)
                        };
                        if adapter.name() == "claude" {
                            let config_path = adapter.plugin_config_path();
                            deployer::remove_plugin_entry(&config_path, &plugin_key)?;
                        } else if adapter.name() == "codex" {
                            // Remove folder + config.toml entry
                            if let Some(ref path) = plugin.path {
                                let target = if let Some(parent) = path.parent() {
                                    if parent
                                        .file_name()
                                        .map(|n| n != "cache" && n != "plugins")
                                        .unwrap_or(false)
                                    {
                                        parent
                                    } else {
                                        path.as_path()
                                    }
                                } else {
                                    path.as_path()
                                };
                                if target.is_dir() {
                                    std::fs::remove_dir_all(target)?;
                                } else if target.is_file() {
                                    std::fs::remove_file(target)?;
                                }
                            }
                            deployer::remove_codex_plugin_entry(
                                &adapter.mcp_config_path(),
                                &plugin_key,
                            )?;
                        } else if adapter.name() == "gemini" {
                            // Remove folder + enablement entry
                            if let Some(ref path) = plugin.path {
                                if path.is_dir() {
                                    std::fs::remove_dir_all(path)?;
                                }
                            }
                            deployer::remove_gemini_extension_entry(
                                &adapter.base_dir().join("extensions"),
                                &plugin.name,
                            )?;
                        } else if adapter.name() == "copilot" {
                            // Remove folder + state.vscdb entry (if VS Code plugin)
                            if let Some(ref path) = plugin.path {
                                if path.is_dir() {
                                    std::fs::remove_dir_all(path)?;
                                }
                            }
                            if let (Some(uri), Some(vscode_dir)) =
                                (&plugin.uri, adapter.vscode_user_dir())
                            {
                                // Best-effort: VS Code may hold a lock on state.vscdb
                                if let Err(e) = deployer::remove_vscode_plugin_entry(&vscode_dir, uri) {
                                    eprintln!("Warning: failed to clean up VS Code plugin entry: {e}");
                                }
                            }
                        } else {
                            // Cursor, etc. — just remove folder
                            if let Some(ref path) = plugin.path {
                                if path.is_dir() {
                                    std::fs::remove_dir_all(path)?;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Remove from database (only after successful disk/config deletion)
        let store = store.lock();
        store.delete_extension(&id)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}

/// Remove a CLI binary file. Called by frontend only during full CLI uninstall.
#[tauri::command]
pub fn uninstall_cli_binary(binary_path: String) -> Result<(), HkError> {
    let path = std::path::Path::new(&binary_path);
    if path.exists() && path.is_file() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// List files in a skill directory as a shallow tree (2 levels deep).
#[tauri::command]
pub fn list_skill_files(state: State<AppState>, path: String) -> Result<Vec<FileEntry>, HkError> {
    let root = std::path::Path::new(&path);
    if !root.is_dir() {
        return Err(HkError::Validation("Path is not a directory".into()));
    }
    if !is_path_within_allowed_dirs(root, &state)? {
        return Err(HkError::PathNotAllowed(
            "Path is not within a known agent or project directory".into(),
        ));
    }
    list_dir_entries(root, 0)
}

/// Open a file or directory in the system's default application.
#[tauri::command]
pub fn open_in_system(state: State<AppState>, path: String) -> Result<(), HkError> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err(HkError::NotFound("Path does not exist".into()));
    }
    if !is_path_within_allowed_dirs(file_path, &state)? {
        return Err(HkError::PathNotAllowed(
            "Path is not within a known agent or project directory".into(),
        ));
    }
    if file_path.is_file() {
        let allowed_extensions = [
            "md", "txt", "json", "toml", "yaml", "yml", "xml", "js", "ts", "py", "rs", "go", "css",
            "html", "csv", "log", "conf", "cfg", "ini", "env",
        ];
        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !allowed_extensions.contains(&ext) {
            return Err(HkError::Validation(format!(
                "Cannot open files with extension '.{}' — use Reveal in Finder instead",
                ext
            )));
        }
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to open: {}", e)))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to open: {}", e)))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to open: {}", e)))?;
    }
    Ok(())
}

/// Reveal a file or directory in the system file manager (Finder / Explorer).
#[tauri::command]
pub fn reveal_in_file_manager(state: State<AppState>, path: String) -> Result<(), HkError> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err(HkError::NotFound("Path does not exist".into()));
    }
    if !is_path_within_allowed_dirs(file_path, &state)? {
        return Err(HkError::PathNotAllowed(
            "Path is not within a known agent or project directory".into(),
        ));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to reveal: {}", e)))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path))
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to reveal: {}", e)))?;
    }
    #[cfg(target_os = "linux")]
    {
        let parent = file_path.parent().unwrap_or(file_path);
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to reveal: {}", e)))?;
    }
    Ok(())
}

/// For a given skill name, find all physical paths where it exists across all agents.
/// Returns Vec<(agent_name, path)> for display in the detail panel.
#[tauri::command]
pub fn get_skill_locations(state: State<AppState>, name: String) -> Vec<(String, String, Option<String>)> {
    let adapters = &*state.adapters;
    scanner::skill_locations(&name, adapters)
        .into_iter()
        .map(|(agent, path)| {
            // Check if the path itself or its parent skill_dir is a symlink
            let symlink_target = if path
                .symlink_metadata()
                .map(|m| m.is_symlink())
                .unwrap_or(false)
            {
                std::fs::read_link(&path)
                    .ok()
                    .map(|t| t.to_string_lossy().to_string())
            } else {
                path.parent()
                    .filter(|p| {
                        p.symlink_metadata()
                            .map(|m| m.is_symlink())
                            .unwrap_or(false)
                    })
                    .and_then(|p| std::fs::read_link(p).ok())
                    .map(|t| {
                        t.join(path.file_name().unwrap_or_default())
                            .to_string_lossy()
                            .to_string()
                    })
            };
            (agent, path.to_string_lossy().to_string(), symlink_target)
        })
        .collect()
}

#[tauri::command]
pub fn get_extension_content(
    state: State<AppState>,
    id: String,
) -> Result<ExtensionContent, HkError> {
    // Read extension metadata and release lock before file I/O
    let ext = {
        let store = state.store.lock();
        store
            .get_extension(&id)?
            .ok_or_else(|| HkError::NotFound("Extension not found".into()))?
    };

    let adapters = &*state.adapters;
    match ext.kind {
        ExtensionKind::Skill => {
            if let Some(loc) = find_skill_by_id(adapters, &id, &ext.agents) {
                let dir = if loc.entry_path.is_dir() {
                    loc.entry_path.to_string_lossy().to_string()
                } else {
                    loc.skill_file
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                };
                // Detect symlink: check entry itself, then parent skill_dir
                let dir_symlink_target = if loc
                    .skill_dir
                    .symlink_metadata()
                    .map(|m| m.is_symlink())
                    .unwrap_or(false)
                {
                    std::fs::read_link(&loc.skill_dir).ok()
                } else {
                    None
                };
                let symlink_target = if loc
                    .entry_path
                    .symlink_metadata()
                    .map(|m| m.is_symlink())
                    .unwrap_or(false)
                {
                    // Entry itself is a symlink
                    std::fs::read_link(&loc.entry_path)
                        .ok()
                        .map(|t| t.to_string_lossy().to_string())
                } else if let Some(ref resolved_dir) = dir_symlink_target {
                    // Entry is real but accessed through a symlinked parent dir
                    let entry_name = loc.entry_path.file_name().unwrap_or_default();
                    Some(resolved_dir.join(entry_name).to_string_lossy().to_string())
                } else {
                    None
                };
                let content = std::fs::read_to_string(&loc.skill_file)?;
                Ok(ExtensionContent {
                    content,
                    path: Some(dir),
                    symlink_target,
                })
            } else {
                Err(HkError::NotFound("Skill file not found".into()))
            }
        }
        ExtensionKind::Mcp => {
            // Pull actual config from the adapter for rich detail
            let mut fallback_config_path = None;
            for adapter in adapters {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let config_path = adapter.mcp_config_path();
                if fallback_config_path.is_none() {
                    fallback_config_path = Some(config_path.to_string_lossy().to_string());
                }
                for server in adapter.read_mcp_servers() {
                    if scanner::stable_id_for(&server.name, "mcp", adapter.name()) == id {
                        let mut lines = vec![format!("Command: {}", server.command)];
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
            let mut fallback_config_path = None;
            for adapter in adapters {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let config_path = adapter.hook_config_path();
                if fallback_config_path.is_none() {
                    fallback_config_path = Some(config_path.to_string_lossy().to_string());
                }
                for hook in adapter.read_hooks() {
                    let hook_name = format!(
                        "{}:{}:{}",
                        hook.event,
                        hook.matcher.as_deref().unwrap_or("*"),
                        hook.command
                    );
                    if scanner::stable_id_for(&hook_name, "hook", adapter.name()) == id {
                        let mut lines = vec![format!("Event: {}", hook.event)];
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
            for adapter in adapters {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                for plugin in adapter.read_plugins() {
                    if scanner::stable_id_for(
                        &format!("{}:{}", plugin.name, plugin.source),
                        "plugin",
                        adapter.name(),
                    ) == id
                    {
                        let path_str = plugin
                            .path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string());
                        // Try to read README.md from plugin directory for documentation
                        let content = plugin.path.as_ref()
                            .and_then(|p| {
                                // Check plugin dir itself, then parent repo root
                                for candidate in [p.join("README.md"), p.join("readme.md")] {
                                    if let Ok(text) = std::fs::read_to_string(&candidate) {
                                        return Some(text);
                                    }
                                }
                                // Walk up to find README in repo root (for git-cloned plugins)
                                let mut dir = p.clone();
                                while dir.pop() {
                                    if dir.join(".git").exists() {
                                        for name in ["README.md", "readme.md"] {
                                            if let Ok(text) = std::fs::read_to_string(dir.join(name)) {
                                                return Some(text);
                                            }
                                        }
                                        break;
                                    }
                                }
                                None
                            })
                            .unwrap_or(ext.description);
                        return Ok(ExtensionContent {
                            content,
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
        ExtensionKind::Cli => Ok(ExtensionContent {
            content: ext.description,
            path: None,
            symlink_target: None,
        }),
    }
}

#[tauri::command]
pub async fn scan_and_sync(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<usize, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();

    // Phase 1+2: Scan filesystem and sync to DB.
    let (count, unlinked) = tauri::async_runtime::spawn_blocking(move || {
        let extensions = scanner::scan_all(&adapters);
        let count = extensions.len();

        let store = store.lock();

        let pre_ids: std::collections::HashSet<String> = store
            .list_extensions(Some(ExtensionKind::Skill), None)
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.id)
            .collect();

        store.sync_extensions(&extensions)?;

        let unlinked: Vec<(String, String)> = store
            .list_extensions(Some(ExtensionKind::Skill), None)?
            .into_iter()
            .filter(|e| e.install_meta.is_none() && !pre_ids.contains(&e.id))
            .map(|e| (e.id, e.name))
            .collect();

        Ok::<_, HkError>((count, unlinked))
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))??;

    // Phase 3+4: Marketplace matching runs in a background task so the
    // command returns immediately and the frontend can load data.
    if !unlinked.is_empty() {
        let store = state.store.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let unique_names: std::collections::HashSet<String> =
                unlinked.iter().map(|(_, n)| n.clone()).collect();
            let mut matched: std::collections::HashMap<String, (String, String, Option<String>)> =
                std::collections::HashMap::new();
            for name in &unique_names {
                if let Ok(results) = hk_core::marketplace::search_skills(name, 5) {
                    let exact: Vec<_> = results.iter().filter(|r| r.name.eq_ignore_ascii_case(name)).collect();
                    if exact.len() == 1 {
                        let item = exact[0];
                        let git_url = hk_core::marketplace::git_url_for_source(&item.source);
                        let remote_rev = manager::get_remote_head(&git_url).ok();
                        matched.insert(name.to_string(), (git_url, item.skill_id.clone(), remote_rev));
                    }
                }
            }

            if !matched.is_empty() {
                let store = store.lock();
                let now = chrono::Utc::now();
                for (id, name) in &unlinked {
                    if let Some((git_url, skill_id, remote_rev)) = matched.get(name.as_str()) {
                        let meta = InstallMeta {
                            install_type: "marketplace".into(),
                            url: Some(format!("{}/{}", git_url.trim_end_matches(".git"), skill_id)),
                            url_resolved: Some(git_url.clone()),
                            branch: None,
                            subpath: if skill_id.is_empty() { None } else { Some(skill_id.clone()) },
                            revision: remote_rev.clone(),
                            remote_revision: remote_rev.clone(),
                            checked_at: Some(now),
                            check_error: None,
                        };
                        let _ = store.set_install_meta(id, &meta);
                    }
                }
                let _ = store.run_backfill_packs();
            }
            let _ = app.emit("extensions-changed", ());
        });
    }

    Ok(count)
}

#[tauri::command]
pub fn get_cached_update_statuses(
    state: State<AppState>,
) -> Result<Vec<(String, UpdateStatus)>, HkError> {
    let store = state.store.lock();
    let extensions = store.list_extensions(None, None)?;
    let mut results = Vec::new();
    for ext in extensions {
        // Only skills support updates
        if ext.kind != ExtensionKind::Skill {
            continue;
        }
        let Some(meta) = ext.install_meta else {
            continue;
        };
        // Only include extensions that have been checked before
        if meta.checked_at.is_none() {
            continue;
        }
        let status = match (meta.revision.as_deref(), meta.remote_revision.as_deref()) {
            (Some(local), Some(remote)) => {
                if local.starts_with(remote) || remote.starts_with(local) {
                    UpdateStatus::UpToDate {
                        remote_hash: remote.to_string(),
                    }
                } else {
                    UpdateStatus::UpdateAvailable {
                        remote_hash: remote.to_string(),
                    }
                }
            }
            (None, Some(remote)) => {
                // No local revision (pre-existing skill) — treat as update available
                UpdateStatus::UpdateAvailable {
                    remote_hash: remote.to_string(),
                }
            }
            _ => {
                if let Some(ref err) = meta.check_error {
                    if err == "removed_from_repo" {
                        UpdateStatus::RemovedFromRepo
                    } else {
                        UpdateStatus::Error { message: err.clone() }
                    }
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
pub async fn check_updates(
    state: State<'_, AppState>,
) -> Result<CheckUpdatesResult, HkError> {
    let store_clone = state.store.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<CheckUpdatesResult, HkError> {
        // Read all extensions and release the lock before doing slow network calls
        type Updatable = Vec<(String, String, InstallMeta)>; // (id, name, meta)
        type Unlinked = Vec<(String, String)>;
        let (updatable, unlinked): (Updatable, Unlinked) = {
            let store = store_clone.lock();
            let extensions = store.list_extensions(None, None)?;
            let mut has_meta = Vec::new();
            let mut no_meta = Vec::new();
            for e in extensions {
                // Only skills support update via git clone + deploy
                if e.kind != ExtensionKind::Skill {
                    continue;
                }
                if let Some(meta) = e.install_meta {
                    match meta.install_type.as_str() {
                        "git" | "marketplace" => has_meta.push((e.id, e.name, meta)),
                        _ => {}
                    }
                } else {
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

            let mut matched: std::collections::HashMap<String, (String, String, Option<String>)> =
                std::collections::HashMap::new();
            for name in &unique_names {
                if let Ok(results) = hk_core::marketplace::search_skills(name, 5) {
                    let exact: Vec<_> = results
                        .iter()
                        .filter(|r| r.name.eq_ignore_ascii_case(name))
                        .collect();
                    if exact.len() == 1 {
                        let item = exact[0];
                        let git_url = hk_core::marketplace::git_url_for_source(&item.source);
                        let remote_rev = manager::get_remote_head(&git_url).ok();
                        matched.insert(
                            name.to_string(),
                            (git_url, item.skill_id.clone(), remote_rev),
                        );
                    }
                }
            }

            if !matched.is_empty() {
                let store = store_clone.lock();
                let now = chrono::Utc::now();
                for (id, name) in &unlinked {
                    if let Some((git_url, skill_id, remote_rev)) = matched.get(name.as_str()) {
                        let meta = InstallMeta {
                            install_type: "marketplace".into(),
                            url: Some(format!(
                                "{}/{}",
                                &git_url.trim_end_matches(".git"),
                                skill_id
                            )),
                            url_resolved: Some(git_url.clone()),
                            branch: None,
                            subpath: if skill_id.is_empty() {
                                None
                            } else {
                                Some(skill_id.clone())
                            },
                            revision: remote_rev.clone(),
                            remote_revision: remote_rev.clone(),
                            checked_at: Some(now),
                            check_error: None,
                        };
                        if let Err(e) = store.set_install_meta(id, &meta) {
                            eprintln!("[hk] warning: {e}");
                        }
                    }
                }
            }
        }

        // Check each extension for updates — cache git ls-remote results per URL
        // so extensions sharing the same repo only trigger one network call.
        let mut remote_cache = std::collections::HashMap::new();
        let mut statuses: Vec<_> = updatable
            .iter()
            .map(|(id, name, meta)| {
                let status = manager::check_update_with_cache(meta, &mut remote_cache);
                (id.clone(), name.clone(), meta.clone(), status)
            })
            .collect();

        // For all skills marked UpdateAvailable, clone the repo to:
        // 1. Verify existing skills still exist (RemovedFromRepo detection)
        // 2. Discover new skills not yet installed
        // Group by URL so we clone each repo at most once.
        let mut new_skills: Vec<NewRepoSkill> = Vec::new();
        {
            use std::collections::HashMap;

            // Collect all UpdateAvailable indices grouped by resolved URL
            let mut url_to_indices: HashMap<String, Vec<usize>> = HashMap::new();
            for (idx, (_, _, meta, status)) in statuses.iter().enumerate() {
                if !matches!(status, UpdateStatus::UpdateAvailable { .. }) {
                    continue;
                }
                let url = meta
                    .url_resolved
                    .as_deref()
                    .or(meta.url.as_deref())
                    .unwrap_or("");
                if !url.is_empty() {
                    url_to_indices
                        .entry(url.to_string())
                        .or_default()
                        .push(idx);
                }
            }

            for (url, indices) in &url_to_indices {
                // Clone once per URL
                let temp = match tempfile::tempdir() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let clone_path = temp.path().join("repo");
                let output = std::process::Command::new("git")
                    .args(["clone", "--depth", "1", "--", url, &clone_path.to_string_lossy()])
                    .output();
                let ok = output.map(|o| o.status.success()).unwrap_or(false);
                if !ok {
                    continue;
                }

                // 1. Verify existing skills with subpath
                for &idx in indices {
                    let (_, name, meta, _) = &statuses[idx];
                    if meta.subpath.is_some()
                        && manager::find_skill_in_repo(&clone_path, name).is_none()
                    {
                        eprintln!(
                            "[hk] Skill '{}' no longer exists in repository",
                            name
                        );
                        statuses[idx].3 = UpdateStatus::RemovedFromRepo;
                    }
                }

                // 2. Discover new skills in this repo
                let repo_skills = manager::scan_repo_skills(&clone_path);
                if repo_skills.len() <= 1 {
                    // Single-skill repo or empty — no new skills to discover
                    continue;
                }

                // Collect installed skill names from DB
                // (not just from statuses — covers skills without install_meta too)
                let installed_names: std::collections::HashSet<String> = {
                    let store = store_clone.lock();
                    let all_exts = store.list_extensions(Some(ExtensionKind::Skill), None)
                        .unwrap_or_default();
                    let mut names = std::collections::HashSet::new();
                    for ext in &all_exts {
                        let matches_url = ext.install_meta.as_ref().map_or(false, |m| {
                            m.url_resolved.as_deref().or(m.url.as_deref())
                                == Some(url.as_str())
                        });
                        if matches_url {
                            names.insert(ext.name.clone());
                        }
                    }
                    names
                };

                let pack = hk_core::scanner::extract_pack_from_url(url);

                for skill in &repo_skills {
                    if !installed_names.contains(skill.name.as_str()) {
                        new_skills.push(NewRepoSkill {
                            repo_url: url.clone(),
                            pack: pack.clone(),
                            skill_id: skill.skill_id.clone(),
                            name: skill.name.clone(),
                            description: skill.description.clone(),
                        });
                    }
                }
            }
        }

        // Persist check state
        let store = store_clone.lock();
        let now = chrono::Utc::now();
        for (id, _name, _meta, status) in &statuses {
            let (remote_rev, check_err) = match status {
                UpdateStatus::UpToDate { remote_hash } => (Some(remote_hash.as_str()), None),
                UpdateStatus::UpdateAvailable { remote_hash } => (Some(remote_hash.as_str()), None),
                UpdateStatus::RemovedFromRepo => (None, Some("removed_from_repo")),
                UpdateStatus::Error { message } => (None, Some(message.as_str())),
            };
            if let Err(e) = store.update_check_state(id, remote_rev, now, check_err) {
                eprintln!("[hk] warning: {e}");
            }
        }

        Ok(CheckUpdatesResult {
            statuses: statuses
                .into_iter()
                .map(|(id, _, _, status)| (id, status))
                .collect(),
            new_skills,
        })
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn update_extension(
    state: State<'_, AppState>,
    id: String,
) -> Result<manager::InstallResult, HkError> {
    let store_clone = state.store.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<manager::InstallResult, HkError> {
        let (ext, install_meta) = {
            let store = store_clone.lock();
            let ext = store
                .get_extension(&id)?
                .ok_or_else(|| HkError::NotFound(format!("Extension '{}' not found", id)))?;
            let meta = ext.install_meta.clone().ok_or_else(|| {
                HkError::NotFound("Extension has no install metadata — cannot update".into())
            })?;
            match meta.install_type.as_str() {
                "git" | "marketplace" => {}
                _ => {
                    return Err(HkError::Validation(format!(
                        "Extensions with install type '{}' cannot be updated",
                        meta.install_type
                    )));
                }
            }
            (ext, meta)
        };
        let url = install_meta
            .url_resolved
            .as_deref()
            .or(install_meta.url.as_deref())
            .ok_or_else(|| HkError::NotFound("Extension has no remote URL".into()))?;

        // Clone the repo once
        let temp = tempfile::tempdir().map_err(|e| HkError::Internal(e.to_string()))?;
        let clone_dir = temp.path().join("repo");
        let output = std::process::Command::new("git")
            .args(["clone", "--depth", "1", "--", url, &clone_dir.to_string_lossy()])
            .output()
            .map_err(|e| HkError::CommandFailed(format!("Failed to run git clone: {}", e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(HkError::CommandFailed(format!(
                "git clone failed: {}",
                stderr.trim()
            )));
        }
        let revision = manager::capture_git_revision_pub(&clone_dir);

        let skill_name = &ext.name;
        let skill_source = match manager::find_skill_in_repo(&clone_dir, skill_name) {
            Some(path) => path,
            None => {
                eprintln!(
                    "[hk] Skill '{}' no longer exists in repository — skipping update",
                    skill_name
                );
                // Persist removed_from_repo state so UI shows it after restart
                let store = store_clone.lock();
                let now = chrono::Utc::now();
                if let Err(e) = store.update_check_state(&id, None, now, Some("removed_from_repo")) {
                    eprintln!("[hk] warning: {e}");
                }
                return Ok(manager::InstallResult {
                    name: skill_name.clone(),
                    was_update: false,
                    revision,
                    skipped: true,
                });
            }
        };

        // Find all installed paths (deduplicated) and copy the latest version to each
        let all_siblings: Vec<Extension> = {
            let store = store_clone.lock();
            let all = store.list_extensions(Some(ext.kind), None)?;
            all.into_iter()
                .filter(|e| e.name == ext.name && e.source_path.is_some())
                .collect()
        };

        let mut updated_dirs = std::collections::HashSet::new();
        for sibling in &all_siblings {
            let source_path = sibling
                .source_path
                .as_deref()
                .ok_or_else(|| HkError::Internal("Sibling extension has no source_path".into()))?;
            let skill_dir = std::path::Path::new(source_path).parent().ok_or_else(|| {
                HkError::Internal("Cannot determine skill directory from source path".into())
            })?;
            if !updated_dirs.insert(skill_dir.to_string_lossy().to_string()) {
                continue;
            }
            hk_core::deployer::deploy_skill(
                &skill_source,
                skill_dir.parent().unwrap_or(skill_dir),
            )?;
        }

        // Update install metadata for all siblings
        {
            let store = store_clone.lock();
            let updated_meta = InstallMeta {
                revision: revision.clone().or(install_meta.revision.clone()),
                remote_revision: None,
                checked_at: None,
                check_error: None,
                ..install_meta
            };
            for sibling in &all_siblings {
                if let Err(e) = store.set_install_meta(&sibling.id, &updated_meta) {
                    eprintln!("[hk] warning: {e}");
                }
            }
        }

        // Skip partial rescan here — caller triggers a full scan_and_sync
        // to avoid inconsistency with CLI sub-extension merging.
        Ok(manager::InstallResult {
            name: skill_name.clone(),
            was_update: true,
            revision,
            skipped: false,
        })
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}
