use axum::extract::State;
use axum::Json;
use hk_core::models::{DiscoveredProject, Project};
use hk_core::scanner;
use serde::Deserialize;

use crate::router::{blocking, ApiError};
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

pub async fn list_projects(
    State(state): State<WebState>,
) -> Result<Vec<Project>> {
    blocking(move || {
        let store = state.store.lock();
        let mut projects = store.list_projects()?;
        for p in &mut projects {
            p.exists = std::path::Path::new(&p.path).exists();
        }
        Ok(projects)
    }).await
}

#[derive(Deserialize)]
pub struct AddProjectParams {
    pub path: String,
}

pub async fn add_project(
    State(state): State<WebState>,
    Json(params): Json<AddProjectParams>,
) -> Result<Project> {
    blocking(move || {
        // Canonicalize to prevent duplicates via symlinks/relative paths
        let project_path = std::path::Path::new(&params.path)
            .canonicalize()
            .map_err(|e| hk_core::HkError::CommandFailed(format!("Invalid path: {}", e)))?;
        let project_path = super::normalize(&project_path);
        let path = project_path.to_string_lossy().to_string();

        // Validate the path contains project markers for any supported agent
        let has_agent_config = project_path.join(".claude").is_dir()
            || project_path.join(".mcp.json").is_file()
            || project_path.join(".codex").is_dir()
            || project_path.join(".gemini").is_dir()
            || project_path.join(".cursor").join("rules").is_dir()
            || project_path.join(".cursorrules").is_file()
            || project_path.join(".github").join("copilot-instructions.md").is_file()
            || project_path.join(".github").join("instructions").is_dir()
            || project_path.join(".agent").join("rules").is_dir()
            || project_path.join(".agent").join("skills").is_dir();
        if !has_agent_config {
            return Err(hk_core::HkError::Validation(
                "Directory does not contain any recognized agent configuration".into(),
            ));
        }

        // Check for duplicate before insert
        let store = state.store.lock();
        let existing = store.list_projects()?;
        if existing.iter().any(|p| p.path == path) {
            return Err(hk_core::HkError::Conflict("Project already added".into()));
        }

        let id = format!("proj-{:016x}", scanner::fnv1a(path.as_bytes()));
        let name = project_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let project = Project {
            id,
            name,
            path,
            created_at: chrono::Utc::now(),
            exists: true,
        };
        store.insert_project(&project)?;
        Ok(project)
    }).await
}

#[derive(Deserialize)]
pub struct RemoveProjectParams {
    pub id: String,
}

pub async fn remove_project(
    State(state): State<WebState>,
    Json(params): Json<RemoveProjectParams>,
) -> Result<()> {
    blocking(move || {
        let store = state.store.lock();
        store.delete_project(&params.id)?;
        Ok(())
    }).await
}

#[derive(Deserialize)]
pub struct DiscoverProjectsParams {
    pub root_path: String,
}

pub async fn discover_projects(
    State(_state): State<WebState>,
    Json(params): Json<DiscoverProjectsParams>,
) -> Result<Vec<DiscoveredProject>> {
    blocking(move || {
        let root = std::path::Path::new(&params.root_path);
        // Reject root directories: "/" on Unix, "C:\" on Windows
        let root_str = params.root_path.as_str();
        let is_drive_root = hk_core::sanitize::is_windows_abs_path(root_str) && root_str.len() <= 3;
        if root == std::path::Path::new("/") || root.parent().is_none() || is_drive_root {
            return Err(hk_core::HkError::Validation(
                "Cannot scan root directory — choose a more specific path".into(),
            ));
        }
        if !root.is_dir() {
            return Err(hk_core::HkError::Validation(format!(
                "Not a directory: {}", params.root_path
            )));
        }
        Ok(scanner::discover_projects(root, 4))
    }).await
}
