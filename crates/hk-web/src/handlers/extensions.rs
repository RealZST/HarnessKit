use axum::extract::State;
use axum::Json;
use hk_core::models::{Extension, ExtensionKind};
use hk_core::{manager, scanner};
use serde::Deserialize;

use crate::router::ApiError;
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
    let store = state.store.lock();
    let kind = params.kind.as_deref().and_then(|s| s.parse::<ExtensionKind>().ok());
    let exts = store.list_extensions(kind, params.agent.as_deref())?;
    Ok(Json(exts))
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
    let store = state.store.lock();
    manager::toggle_extension_with_adapters(
        &store,
        &state.adapters,
        &params.id,
        params.enabled,
    )?;
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct IdParams {
    pub id: String,
}

pub async fn get_extension_content(
    State(state): State<WebState>,
    Json(params): Json<IdParams>,
) -> Result<ExtensionContentResponse> {
    let store = state.store.lock();
    let ext = store.get_extension(&params.id)?
        .ok_or_else(|| ApiError::not_found("Extension not found"))?;
    let content = ext
        .source_path
        .as_ref()
        .and_then(|p| {
            let path = std::path::Path::new(p);
            if path.is_dir() {
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    return std::fs::read_to_string(&skill_file).ok();
                }
                std::fs::read_dir(path).ok().and_then(|mut entries| {
                    entries.find_map(|e| {
                        let e = e.ok()?;
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.ends_with(".md") {
                            std::fs::read_to_string(e.path()).ok()
                        } else {
                            None
                        }
                    })
                })
            } else {
                std::fs::read_to_string(path).ok()
            }
        })
        .unwrap_or_default();
    let symlink_target = ext.source_path.as_ref().and_then(|p| {
        std::fs::read_link(p).ok().map(|t| t.to_string_lossy().to_string())
    });
    Ok(Json(ExtensionContentResponse {
        content,
        path: ext.source_path.clone(),
        symlink_target,
    }))
}

#[derive(serde::Serialize)]
pub struct ExtensionContentResponse {
    pub content: String,
    pub path: Option<String>,
    pub symlink_target: Option<String>,
}

pub async fn delete_extension(
    State(state): State<WebState>,
    Json(params): Json<IdParams>,
) -> Result<()> {
    use hk_core::deployer;
    let store = state.store.lock();
    let ext = store.get_extension(&params.id)?
        .ok_or_else(|| ApiError::not_found("Extension not found"))?;

    match ext.kind {
        ExtensionKind::Skill => {
            if let Some(source_path) = &ext.source_path {
                let path = std::path::Path::new(source_path);
                if path.exists() {
                    if path.is_dir() {
                        std::fs::remove_dir_all(path).ok();
                    } else {
                        std::fs::remove_file(path).ok();
                    }
                }
            }
        }
        ExtensionKind::Mcp => {
            for agent_name in &ext.agents {
                if let Some(adapter) = state.adapters.iter().find(|a| a.name() == agent_name) {
                    let config_path = adapter.mcp_config_path();
                    deployer::remove_mcp_server(
                        &config_path,
                        &ext.name,
                        adapter.mcp_format(),
                    ).ok();
                }
            }
        }
        ExtensionKind::Hook => {
            for agent_name in &ext.agents {
                if let Some(adapter) = state.adapters.iter().find(|a| a.name() == agent_name) {
                    let config_path = adapter.hook_config_path();
                    let _ = &config_path;
                }
            }
        }
        ExtensionKind::Plugin => {
            if let Some(source_path) = &ext.source_path {
                let path = std::path::Path::new(source_path);
                if path.exists() {
                    if path.is_dir() {
                        std::fs::remove_dir_all(path).ok();
                    } else {
                        std::fs::remove_file(path).ok();
                    }
                }
            }
        }
        ExtensionKind::Cli => {}
    }

    store.delete_extension(&params.id)?;
    Ok(Json(()))
}

pub async fn scan_and_sync(
    State(state): State<WebState>,
) -> Result<usize> {
    let scanned = scanner::scan_all(&state.adapters);
    let count = scanned.len();
    let store = state.store.lock();
    store.sync_extensions(&scanned)?;
    store.run_backfill_packs()?;
    Ok(Json(count))
}

pub async fn list_skill_files(
    State(_state): State<WebState>,
    Json(params): Json<ListSkillFilesParams>,
) -> Result<Vec<FileEntry>> {
    let path = std::path::Path::new(&params.path);
    if !path.exists() || !path.is_dir() {
        return Err(ApiError::not_found("Directory not found"));
    }
    let entries = list_dir_entries(path, 0);
    Ok(Json(entries))
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
