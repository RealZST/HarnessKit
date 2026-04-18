use axum::extract::State;
use axum::Json;
use hk_core::models::AuditResult;
use hk_core::service;

use crate::router::ApiError;
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

pub async fn list_audit_results(
    State(state): State<WebState>,
) -> Result<Vec<AuditResult>> {
    let store = state.store.lock();
    let results = store.list_latest_audit_results()?;
    Ok(Json(results))
}

pub async fn run_audit(
    State(state): State<WebState>,
) -> Result<Vec<AuditResult>> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    let results = tokio::task::spawn_blocking(move || {
        let store = store.lock();
        service::run_full_audit(&store, &adapters)
    })
    .await
    .map_err(|e| ApiError::from(hk_core::HkError::Internal(e.to_string())))??;
    Ok(Json(results))
}
