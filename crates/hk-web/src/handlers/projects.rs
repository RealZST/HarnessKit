use axum::extract::State;
use axum::Json;
use hk_core::models::{DiscoveredProject, Project};
use hk_core::scanner;
use serde::Deserialize;

use crate::router::ApiError;
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

pub async fn list_projects(
    State(state): State<WebState>,
) -> Result<Vec<Project>> {
    let store = state.store.lock();
    let projects = store.list_projects()?;
    Ok(Json(projects))
}

#[derive(Deserialize)]
pub struct AddProjectParams {
    pub path: String,
}

pub async fn add_project(
    State(state): State<WebState>,
    Json(params): Json<AddProjectParams>,
) -> Result<Project> {
    let store = state.store.lock();
    let existing = store.list_projects()?;
    if existing.iter().any(|p| p.path == params.path) {
        return Err(ApiError::from(hk_core::HkError::Conflict(
            "Project already registered".into(),
        )));
    }
    let name = std::path::Path::new(&params.path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| params.path.clone());
    let id = format!("{:016x}", scanner::fnv1a(params.path.as_bytes()));
    let project = Project {
        id,
        name,
        path: params.path,
        created_at: chrono::Utc::now(),
        exists: true,
    };
    store.insert_project(&project)?;
    Ok(Json(project))
}

#[derive(Deserialize)]
pub struct RemoveProjectParams {
    pub id: String,
}

pub async fn remove_project(
    State(state): State<WebState>,
    Json(params): Json<RemoveProjectParams>,
) -> Result<()> {
    let store = state.store.lock();
    store.delete_project(&params.id)?;
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct DiscoverProjectsParams {
    pub root_path: String,
}

pub async fn discover_projects(
    State(_state): State<WebState>,
    Json(params): Json<DiscoverProjectsParams>,
) -> Result<Vec<DiscoveredProject>> {
    let path = std::path::Path::new(&params.root_path);
    let projects = scanner::discover_projects(path, 3);
    Ok(Json(projects))
}
