use super::AppState;
use super::helpers::{FileEntry, is_path_within_allowed_dirs, list_dir_entries};
use hk_core::service::ExtensionContent;
use hk_core::{HkError, manager, models::*, scanner, service};
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
pub async fn scan_and_sync(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<usize, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();

    // Phase 1+2: Scan filesystem and sync to DB.
    let (count, unlinked) = tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        let projects = store.list_project_tuples();
        let extensions = scanner::scan_all(&adapters, &projects);
        let count = extensions.len();

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
                // Project-scoped skills are owned by the project's own version
                // control, not by HK's marketplace/update flow. Skip them so we
                // don't auto-link them to a marketplace skill that just happens
                // to share a name.
                if !matches!(e.scope, ConfigScope::Global) {
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
                        let matches_url = ext.install_meta.as_ref().is_some_and(|m| {
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

        // Find all installed paths (deduplicated) and copy the latest version
        // to each. Restrict to global-scope siblings so the update flow doesn't
        // overwrite user-managed project copies of the same name.
        let all_siblings: Vec<Extension> = {
            let store = store_clone.lock();
            let all = store.list_extensions(Some(ext.kind), None)?;
            all.into_iter()
                .filter(|e| {
                    e.name == ext.name
                        && e.source_path.is_some()
                        && matches!(e.scope, ConfigScope::Global)
                })
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
