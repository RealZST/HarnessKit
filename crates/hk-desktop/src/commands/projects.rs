use super::AppState;
use chrono::Utc;
use hk_core::{HkError, models::*, scanner};
use tauri::State;

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> Result<Vec<Project>, HkError> {
    let store = state.store.lock();
    let mut projects = store.list_projects()?;
    for p in &mut projects {
        p.exists = std::path::Path::new(&p.path).exists();
    }
    Ok(projects)
}

#[tauri::command]
pub fn add_project(state: State<AppState>, path: String) -> Result<Project, HkError> {
    // Canonicalize to prevent duplicates via symlinks/relative paths
    let project_path = std::path::Path::new(&path)
        .canonicalize()
        .map_err(|e| HkError::CommandFailed(format!("Invalid path: {}", e)))?;
    let path = project_path.to_string_lossy().to_string();

    // Validate the path contains project markers for any supported agent
    let has_agent_config = project_path.join(".claude").is_dir()
        || project_path.join(".mcp.json").is_file()
        || project_path.join(".codex").is_dir()
        || project_path.join(".gemini").is_dir()
        || project_path.join(".cursor").join("rules").is_dir()
        || project_path.join(".cursorrules").is_file()
        || project_path
            .join(".github")
            .join("copilot-instructions.md")
            .is_file()
        || project_path.join(".github").join("instructions").is_dir()
        || project_path.join(".agent").join("rules").is_dir()
        || project_path.join(".agent").join("skills").is_dir();
    if !has_agent_config {
        return Err(HkError::Validation(
            "Directory does not contain any recognized agent configuration".into(),
        ));
    }

    // Check for duplicate before insert
    let store = state.store.lock();
    let existing = store.list_projects()?;
    if existing.iter().any(|p| p.path == path) {
        return Err(HkError::Conflict("Project already added".into()));
    }

    // Generate stable ID from path hash
    let id = format!("proj-{:016x}", scanner::fnv1a(path.as_bytes()));

    let name = project_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let project = Project {
        id: id.clone(),
        name,
        path,
        created_at: Utc::now(),
        exists: true,
    };

    store.insert_project(&project)?;
    Ok(project)
}

#[tauri::command]
pub fn remove_project(state: State<AppState>, id: String) -> Result<(), HkError> {
    let store = state.store.lock();
    store.delete_project(&id)
}

#[tauri::command]
pub fn discover_projects(root_path: String) -> Result<Vec<DiscoveredProject>, HkError> {
    let root = std::path::Path::new(&root_path);
    if root == std::path::Path::new("/") || root.parent().is_none() {
        return Err(HkError::Validation(
            "Cannot scan root directory — choose a more specific path".into(),
        ));
    }
    if !root.is_dir() {
        return Err(HkError::Validation(format!(
            "Not a directory: {}",
            root_path
        )));
    }
    Ok(scanner::discover_projects(root, 4))
}
