use super::AppState;
use hk_core::{HkError, manager, models::*};
use tauri::State;

#[tauri::command]
pub async fn list_repositories(
    state: State<'_, AppState>,
) -> Result<Vec<RepositoryWithItems>, HkError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        store.list_repositories_with_items()
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn add_repository(
    state: State<'_, AppState>,
    url: String,
    name: Option<String>,
    branch: Option<String>,
    auth_token: Option<String>,
) -> Result<Repository, HkError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        let new_repo = NewRepository {
            id: None,
            url,
            name,
            branch,
            auth_token,
        };
        store.add_repository(&new_repo)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn delete_repository(state: State<'_, AppState>, id: String) -> Result<bool, HkError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        manager::delete_repository_with_cache(&store, &id)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn sync_repository(
    state: State<'_, AppState>,
    id: String,
) -> Result<RepoSyncResult, HkError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        manager::sync_repository(&store, &id)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn sync_all_repositories(
    state: State<'_, AppState>,
) -> Result<Vec<RepoSyncResult>, HkError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        manager::sync_all_repositories(&store)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn set_repository_item_status(
    state: State<'_, AppState>,
    repo_id: String,
    item_id: String,
    kind: Option<String>,
    status: String,
) -> Result<(), HkError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        let kind = kind
            .as_deref()
            .and_then(|k| k.parse().ok())
            .unwrap_or(ExtensionKind::Skill);
        store.set_repository_item_status(&repo_id, kind, &item_id, &status)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn install_from_repository(
    state: State<'_, AppState>,
    repo_id: String,
    item_id: String,
    target_agents: Option<Vec<String>>,
    target_scope: Option<ConfigScope>,
) -> Result<Vec<manager::InstallResult>, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        let scope = target_scope.unwrap_or(ConfigScope::Global);
        let agents = target_agents.unwrap_or_default();
        manager::install_from_repo_item(&store, &adapters, &repo_id, &item_id, &agents, scope)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}
