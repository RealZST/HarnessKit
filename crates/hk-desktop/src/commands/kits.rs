use crate::commands::AppState;
use hk_core::kits::install_records;
use hk_core::kits::service as kit_service;
use hk_core::kits::types::*;
use tauri::State;

fn err_string<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn list_kits(state: State<AppState>) -> Result<Vec<KitSummary>, String> {
    kit_service::list_kits(&state.store).map_err(err_string)
}

#[tauri::command]
pub fn get_kit_details(state: State<AppState>, id: String) -> Result<KitDetails, String> {
    kit_service::get_kit_details(&state.store, &state.adapters, &id).map_err(err_string)
}

#[tauri::command]
pub fn list_kit_asset_candidates(state: State<AppState>) -> Result<KitAssetCandidates, String> {
    kit_service::list_kit_asset_candidates(&state.store, &state.adapters).map_err(err_string)
}

#[tauri::command]
pub fn create_kit(state: State<AppState>, req: CreateKitRequest) -> Result<KitSummary, String> {
    kit_service::create_kit(&state.store, &state.adapters, req).map_err(err_string)
}

#[tauri::command]
pub fn update_kit(state: State<AppState>, req: UpdateKitRequest) -> Result<KitSummary, String> {
    kit_service::update_kit(&state.store, &state.adapters, req).map_err(err_string)
}

#[tauri::command]
pub fn delete_kit(state: State<AppState>, id: String) -> Result<(), String> {
    kit_service::delete_kit(&state.store, &id).map_err(err_string)
}

#[tauri::command]
pub fn preview_kit_project_conflicts(
    state: State<AppState>,
    req: PreviewKitConflictsRequest,
) -> Result<KitConflictPreview, String> {
    kit_service::preview_kit_project_conflicts(&state.store, &state.adapters, req)
        .map_err(err_string)
}

#[tauri::command]
pub fn sync_kit_to_project(
    state: State<AppState>,
    req: SyncKitRequest,
) -> Result<KitSyncResult, String> {
    kit_service::sync_kit_to_project(&state.store, &state.adapters, req).map_err(err_string)
}

#[tauri::command]
pub fn unsync_kit_from_project(
    state: State<AppState>,
    req: UnsyncKitRequest,
) -> Result<(), String> {
    kit_service::unsync_kit_from_project(&state.store, &state.adapters, req).map_err(err_string)
}

#[tauri::command]
pub fn export_kit(
    state: State<AppState>,
    id: String,
    target_path: String,
) -> Result<(), String> {
    kit_service::export_kit(&state.store, &id, &target_path).map_err(err_string)
}

#[tauri::command]
pub fn import_kit(
    state: State<AppState>,
    source_zip_path: String,
) -> Result<KitSummary, String> {
    kit_service::import_kit(&state.store, &source_zip_path).map_err(err_string)
}

#[tauri::command]
pub fn list_project_install_records(
    state: State<AppState>,
) -> Result<Vec<ProjectInstallRecords>, String> {
    install_records::list_project_install_records(&state.store).map_err(err_string)
}
