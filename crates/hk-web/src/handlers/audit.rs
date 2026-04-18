use axum::extract::State;
use axum::Json;
use hk_core::models::AuditResult;
use hk_core::service;

use crate::router::{blocking, ApiError};
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

pub async fn list_audit_results(
    State(state): State<WebState>,
) -> Result<Vec<AuditResult>> {
    blocking(move || {
        let store = state.store.lock();
        Ok(store.list_latest_audit_results()?)
    }).await
}

pub async fn run_audit(
    State(state): State<WebState>,
) -> Result<Vec<AuditResult>> {
    blocking(move || {
        let store = state.store.lock();
        Ok(service::run_full_audit(&store, &state.adapters)?)
    }).await
}
