use super::AppState;
use hk_core::{HkError, models::*, service};
use tauri::State;

#[tauri::command]
pub fn list_audit_results(state: State<AppState>) -> Result<Vec<AuditResult>, HkError> {
    let store = state.store.lock();
    store.list_latest_audit_results()
}

#[tauri::command]
pub async fn run_audit(state: State<'_, AppState>) -> Result<Vec<AuditResult>, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = store.lock();
        service::run_full_audit(&store, &adapters)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}
