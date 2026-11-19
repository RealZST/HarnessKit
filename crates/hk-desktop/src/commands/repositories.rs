use super::AppState;
use super::helpers::blocking;
use hk_core::HkError;
use hk_core::models::{
    IgnoreRepositoryItemRequest, InstallRepositoryItemRequest, RepositoryWithItems,
};
use hk_core::repositories;
use tauri::State;

#[tauri::command]
pub fn list_repositories(state: State<AppState>) -> Result<Vec<RepositoryWithItems>, HkError> {
    state.store.lock().list_repositories_with_items()
}

#[tauri::command]
pub async fn add_repository(state: State<'_, AppState>, url: String) -> Result<String, HkError> {
    let store = state.store.clone();
    blocking(move || repositories::add_repository(&store, &repositories::cache_root(), &url)).await
}

#[tauri::command]
pub async fn sync_repository(state: State<'_, AppState>, id: String) -> Result<(), HkError> {
    let store = state.store.clone();
    blocking(move || repositories::sync_repository(&store, &repositories::cache_root(), &id)).await
}

#[tauri::command]
pub async fn delete_repository(state: State<'_, AppState>, id: String) -> Result<(), HkError> {
    let store = state.store.clone();
    blocking(move || repositories::delete_repository(&store, &repositories::cache_root(), &id))
        .await
}

#[tauri::command]
pub fn ignore_repository_item(
    state: State<AppState>,
    req: IgnoreRepositoryItemRequest,
) -> Result<(), HkError> {
    state
        .store
        .lock()
        .set_repository_item_ignored(&req.repo_id, &req.item_id, req.ignored)
}

#[tauri::command]
pub async fn install_repository_item(
    state: State<'_, AppState>,
    req: InstallRepositoryItemRequest,
) -> Result<(), HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    blocking(move || {
        repositories::install_item(&store, &adapters, &repositories::cache_root(), &req)
    })
    .await
}
