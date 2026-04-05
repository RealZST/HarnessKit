use hk_core::{adapter, models::*, service};
use tauri::State;
use super::AppState;

#[tauri::command]
pub fn list_audit_results(state: State<AppState>) -> Result<Vec<AuditResult>, String> {
    let store = state.store.lock();
    store.list_latest_audit_results().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn run_audit(state: State<AppState>) -> Result<Vec<AuditResult>, String> {
    let adapters = adapter::all_adapters();
    let store = state.store.lock();
    service::run_full_audit(&store, &adapters).map_err(|e| e.to_string())
}
