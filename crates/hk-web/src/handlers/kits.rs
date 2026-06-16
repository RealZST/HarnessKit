use axum::extract::State;
use axum::Json;
use hk_core::kits::install_records;
use hk_core::kits::service as kit_service;
use hk_core::kits::types::*;
use serde::Deserialize;

use crate::router::{blocking, ApiError};
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

// The frontend transport posts every command to `/api/{command}` with a JSON
// body shaped exactly like the Tauri `invoke` args object. Tauri matches body
// keys to command parameter names, so single-object commands arrive wrapped as
// `{ "req": {...} }` and id-only commands as `{ "id": "..." }`. These extractor
// structs mirror that shape so web mode and desktop share one contract.
#[derive(Deserialize)]
pub struct ReqBody<T> {
    pub req: T,
}

#[derive(Deserialize)]
pub struct IdBody {
    pub id: String,
}

pub async fn list_kits(State(state): State<WebState>) -> Result<Vec<KitSummary>> {
    blocking(move || kit_service::list_kits(&state.store)).await
}

pub async fn list_candidates(State(state): State<WebState>) -> Result<KitAssetCandidates> {
    blocking(move || kit_service::list_kit_asset_candidates(&state.store, &state.adapters)).await
}

pub async fn get_details(
    State(state): State<WebState>,
    Json(body): Json<IdBody>,
) -> Result<KitDetails> {
    blocking(move || kit_service::get_kit_details(&state.store, &state.adapters, &body.id)).await
}

pub async fn create_kit(
    State(state): State<WebState>,
    Json(body): Json<ReqBody<CreateKitRequest>>,
) -> Result<KitSummary> {
    blocking(move || kit_service::create_kit(&state.store, &state.adapters, body.req)).await
}

pub async fn update_kit(
    State(state): State<WebState>,
    Json(body): Json<ReqBody<UpdateKitRequest>>,
) -> Result<KitSummary> {
    blocking(move || kit_service::update_kit(&state.store, &state.adapters, body.req)).await
}

pub async fn delete_kit(
    State(state): State<WebState>,
    Json(body): Json<IdBody>,
) -> Result<()> {
    blocking(move || kit_service::delete_kit(&state.store, &body.id)).await
}

pub async fn preview_conflicts(
    State(state): State<WebState>,
    Json(body): Json<ReqBody<PreviewKitConflictsRequest>>,
) -> Result<KitConflictPreview> {
    blocking(move || {
        kit_service::preview_kit_project_conflicts(&state.store, &state.adapters, body.req)
    })
    .await
}

pub async fn sync_kit(
    State(state): State<WebState>,
    Json(body): Json<ReqBody<SyncKitRequest>>,
) -> Result<KitSyncResult> {
    blocking(move || kit_service::sync_kit_to_project(&state.store, &state.adapters, body.req)).await
}

pub async fn unsync_kit(
    State(state): State<WebState>,
    Json(body): Json<ReqBody<UnsyncKitRequest>>,
) -> Result<()> {
    blocking(move || {
        kit_service::unsync_kit_from_project(&state.store, &state.adapters, body.req)
    })
    .await
}

#[derive(Deserialize)]
pub struct ExportBody {
    pub id: String,
    pub target_path: String,
}

pub async fn export_kit(
    State(state): State<WebState>,
    Json(body): Json<ExportBody>,
) -> Result<()> {
    blocking(move || kit_service::export_kit(&state.store, &body.id, &body.target_path)).await
}

#[derive(Deserialize)]
pub struct ImportBody {
    pub source_zip_path: String,
}

pub async fn import_kit(
    State(state): State<WebState>,
    Json(body): Json<ImportBody>,
) -> Result<KitSummary> {
    blocking(move || kit_service::import_kit(&state.store, &body.source_zip_path)).await
}

pub async fn list_install_records(
    State(state): State<WebState>,
) -> Result<Vec<ProjectInstallRecords>> {
    blocking(move || install_records::list_project_install_records(&state.store)).await
}
