use super::AppState;
use super::helpers::{FileEntry, list_dir_entries};
use hk_core::service::ExtensionContent;
use hk_core::{HkError, manager, models::*, scanner, service, updates};
use tauri::{Emitter, State};

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
) -> Result<manager::ToggleOutcome, HkError> {
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
    tauri::async_runtime::spawn_blocking(move || service::delete_extension(&store, &adapters, &id))
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
pub fn list_skill_files(_state: State<AppState>, path: String) -> Result<Vec<FileEntry>, HkError> {
    let root = std::path::Path::new(&path);
    if !root.exists() {
        return Err(HkError::Validation("Path does not exist".into()));
    }
    if !root.is_dir() {
        // Single-file extensions (e.g. Oh My Pi `.ts` plugins) live directly
        // in the agent's extension directory. Surface them as a one-entry
        // tree so the Documentation panel previews their content instead of
        // reporting "No files found".
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        return Ok(vec![FileEntry {
            name,
            path,
            is_dir: false,
            children: None,
        }]);
    }
    list_dir_entries(root, 0)
}

/// Open a file or directory in the system's default application.
#[tauri::command]
pub fn open_in_system(_state: State<AppState>, path: String) -> Result<(), HkError> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err(HkError::NotFound("Path does not exist".into()));
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
        hk_core::command("open")
            .arg(&path)
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to open: {}", e)))?;
    }
    #[cfg(target_os = "windows")]
    {
        hk_core::command("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to open: {}", e)))?;
    }
    #[cfg(target_os = "linux")]
    {
        hk_core::command("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to open: {}", e)))?;
    }
    Ok(())
}

/// Reveal a file or directory in the system file manager (Finder / Explorer).
#[tauri::command]
pub fn reveal_in_file_manager(_state: State<AppState>, path: String) -> Result<(), HkError> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err(HkError::NotFound("Path does not exist".into()));
    }
    #[cfg(target_os = "macos")]
    {
        hk_core::command("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to reveal: {}", e)))?;
    }
    #[cfg(target_os = "windows")]
    {
        hk_core::command("explorer")
            .arg(format!("/select,{}", path))
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to reveal: {}", e)))?;
    }
    #[cfg(target_os = "linux")]
    {
        let parent = file_path.parent().unwrap_or(file_path);
        hk_core::command("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| HkError::CommandFailed(format!("Failed to reveal: {}", e)))?;
    }
    Ok(())
}

/// For a given skill name, find all physical paths where it exists across all agents.
/// Returns Vec<(agent_name, path)> for display in the detail panel.
#[tauri::command]
pub fn get_skill_locations(
    state: State<AppState>,
    name: String,
) -> Vec<(String, String, Option<String>)> {
    let adapters = &*state.adapters;
    let projects = state.store.lock().list_project_tuples();
    // UI listing — surface every place this skill exists, regardless of scope.
    scanner::skill_locations(&name, adapters, &projects, None)
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
    service::get_extension_content(&state.store, &state.adapters, &id)
}

#[tauri::command]
pub async fn scan_and_sync(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<usize, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();

    // Phase 1+2: Scan filesystem and sync to DB.
    let (count, unlinked) = tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        let projects = store.list_project_tuples();
        let scan = scanner::scan_all(&adapters, &projects);
        let count = scan.extensions.len();

        let pre_ids: std::collections::HashSet<String> = store
            .list_extensions(Some(ExtensionKind::Skill), None)
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.id)
            .collect();

        store.sync_scan(&scan)?;

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
                let store = store.lock();
                let now = chrono::Utc::now();
                for (id, name) in &unlinked {
                    if let Some((git_url, skill_id, remote_rev)) = matched.get(name.as_str()) {
                        let meta = InstallMeta {
                            install_type: "marketplace".into(),
                            url: Some(format!("{}/{}", git_url.trim_end_matches(".git"), skill_id)),
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
    updates::cached_update_statuses(&state.store.lock())
}

#[tauri::command]
pub async fn check_updates(
    state: State<'_, AppState>,
) -> Result<Vec<(String, UpdateStatus)>, HkError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || updates::check_updates(&store))
        .await
        .map_err(|e| HkError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn update_extension(
    state: State<'_, AppState>,
    id: String,
) -> Result<manager::InstallResult, HkError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || updates::update_extension(&store, &id))
        .await
        .map_err(|e| HkError::Internal(e.to_string()))?
}
