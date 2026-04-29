use axum::extract::State;
use axum::Json;
use hk_core::models::{Extension, ExtensionKind};
use hk_core::service::ExtensionContent;
use hk_core::{manager, scanner, service};
use serde::Deserialize;

use crate::router::{blocking, ApiError};
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

#[derive(Deserialize, Default)]
pub struct ListParams {
    pub kind: Option<String>,
    pub agent: Option<String>,
}

pub async fn list_extensions(
    State(state): State<WebState>,
    Json(params): Json<ListParams>,
) -> Result<Vec<Extension>> {
    blocking(move || {
        let store = state.store.lock();
        let kind = params.kind.as_deref().and_then(|s| s.parse::<ExtensionKind>().ok());
        store.list_extensions(kind, params.agent.as_deref())
    }).await
}

#[derive(Deserialize)]
pub struct ToggleParams {
    pub id: String,
    pub enabled: bool,
}

pub async fn toggle_extension(
    State(state): State<WebState>,
    Json(params): Json<ToggleParams>,
) -> Result<()> {
    blocking(move || {
        let store = state.store.lock();
        manager::toggle_extension_with_adapters(
            &store,
            &state.adapters,
            &params.id,
            params.enabled,
        )?;
        Ok(())
    }).await
}

#[derive(Deserialize)]
pub struct IdParams {
    pub id: String,
}

pub async fn get_extension_content(
    State(state): State<WebState>,
    Json(params): Json<IdParams>,
) -> Result<ExtensionContent> {
    blocking(move || service::get_extension_content(&state.store, &state.adapters, &params.id))
        .await
}

pub async fn delete_extension(
    State(state): State<WebState>,
    Json(params): Json<IdParams>,
) -> Result<()> {
    blocking(move || service::delete_extension(&state.store, &state.adapters, &params.id)).await
}

#[derive(Deserialize)]
pub struct UninstallCliBinaryParams {
    pub binary_path: String,
}

pub async fn uninstall_cli_binary(
    State(_state): State<WebState>,
    Json(params): Json<UninstallCliBinaryParams>,
) -> Result<()> {
    blocking(move || {
        let path = std::path::Path::new(&params.binary_path);
        if path.exists() && path.is_file() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }).await
}

pub async fn scan_and_sync(
    State(state): State<WebState>,
) -> std::result::Result<Json<usize>, ApiError> {
    let state_bg = state.clone();

    // Phase 1+2: Scan filesystem and sync to DB
    let (count, unlinked) = tokio::task::spawn_blocking(move || {
        let store = state.store.lock();
        let projects = store.list_project_tuples();
        let extensions = scanner::scan_all(&state.adapters, &projects);
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

        store.run_backfill_packs()?;
        Ok::<_, hk_core::HkError>((count, unlinked))
    })
    .await
    .map_err(|e| ApiError::from(hk_core::HkError::Internal(e.to_string())))??;

    // Phase 3+4: Marketplace matching in background (no Tauri event, but data updates)
    if !unlinked.is_empty() {
        let store = state_bg.store;
        tokio::task::spawn_blocking(move || {
            let unique_names: std::collections::HashSet<String> =
                unlinked.iter().map(|(_, n)| n.clone()).collect();
            let mut matched = std::collections::HashMap::<String, (String, String, Option<String>)>::new();
            for name in &unique_names {
                if let Ok(results) = hk_core::marketplace::search_skills(name, 5) {
                    let exact: Vec<_> = results.iter().filter(|r| r.name.eq_ignore_ascii_case(name)).collect();
                    if exact.len() == 1 {
                        let item = exact[0];
                        let git_url = hk_core::marketplace::git_url_for_source(&item.source);
                        let remote_rev = hk_core::manager::get_remote_head(&git_url).ok();
                        matched.insert(name.to_string(), (git_url, item.skill_id.clone(), remote_rev));
                    }
                }
            }
            if !matched.is_empty() {
                let store = store.lock();
                let now = chrono::Utc::now();
                for (id, name) in &unlinked {
                    if let Some((git_url, skill_id, remote_rev)) = matched.get(name.as_str()) {
                        let meta = hk_core::models::InstallMeta {
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
        });
    }

    Ok(Json(count))
}

pub async fn list_skill_files(
    State(state): State<WebState>,
    Json(params): Json<ListSkillFilesParams>,
) -> Result<Vec<FileEntry>> {
    blocking(move || {
        let path = std::path::Path::new(&params.path);
        if !path.exists() || !path.is_dir() {
            return Err(hk_core::HkError::NotFound("Directory not found".into()));
        }
        // Validate path is within an allowed agent directory
        let canonical = path.canonicalize()
            .map_err(|_| hk_core::HkError::NotFound("Cannot resolve path".into()))?;
        let normalized = super::normalize(&canonical);
        let allowed = state.adapters.iter().any(|a| {
            a.skill_dirs().iter().any(|d| normalized.starts_with(d))
                || normalized.starts_with(a.base_dir())
        });
        if !allowed {
            let store = state.store.lock();
            if !super::is_path_allowed(&canonical, &store) {
                return Err(hk_core::HkError::PathNotAllowed(
                    "Path is not within a known agent directory".into(),
                ));
            }
        }
        Ok(list_dir_entries(path, 0))
    }).await
}

#[derive(Deserialize)]
pub struct ListSkillFilesParams {
    pub path: String,
}

#[derive(serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileEntry>>,
}

fn list_dir_entries(dir: &std::path::Path, depth: u8) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(dir) else { return entries };
    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let children = if is_dir && depth < 1 {
            Some(list_dir_entries(&path, depth + 1))
        } else {
            if is_dir { Some(Vec::new()) } else { None }
        };
        entries.push(FileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
            children,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
    });
    entries
}
