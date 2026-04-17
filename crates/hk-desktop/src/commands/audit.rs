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
        // Read extensions with a brief lock
        let extensions = {
            let store = store.lock();
            store.list_extensions(None, None)?
        };

        // Build inputs + run audit (no lock needed, disk + CPU work)
        let results = service::audit_extensions(&extensions, &adapters);

        // Persist results with a brief lock
        {
            let store = store.lock();
            for result in &results {
                let _ = store.insert_audit_result(result);
            }
        }

        Ok(results)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}
