use axum::{
    Router,
    middleware,
    routing::{get, post},
    response::{Html, IntoResponse},
    http::StatusCode,
    Json,
};
use hk_core::HkError;

use crate::auth::require_token;
use crate::handlers;
use crate::state::WebState;

pub struct ApiError(StatusCode, HkError);

impl ApiError {
    pub fn not_found(msg: &str) -> Self {
        Self(StatusCode::NOT_FOUND, HkError::NotFound(msg.into()))
    }

    pub fn forbidden(msg: &str) -> Self {
        Self(StatusCode::FORBIDDEN, HkError::PermissionDenied(msg.into()))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.0, Json(self.1)).into_response()
    }
}

impl From<HkError> for ApiError {
    fn from(e: HkError) -> Self {
        let status = match &e {
            HkError::NotFound(_) => StatusCode::NOT_FOUND,
            HkError::Network(_) => StatusCode::BAD_GATEWAY,
            HkError::PermissionDenied(_) => StatusCode::FORBIDDEN,
            HkError::ConfigCorrupted(_) => StatusCode::INTERNAL_SERVER_ERROR,
            HkError::Conflict(_) => StatusCode::CONFLICT,
            HkError::PathNotAllowed(_) => StatusCode::FORBIDDEN,
            HkError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            HkError::CommandFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            HkError::Validation(_) => StatusCode::BAD_REQUEST,
            HkError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self(status, e)
    }
}

pub fn build_router(state: WebState) -> Router {
    let api = Router::new()
        // Health
        .route("/api/health", get(health))
        // Extensions
        .route("/api/list_extensions", post(handlers::extensions::list_extensions))
        .route("/api/toggle_extension", post(handlers::extensions::toggle_extension))
        .route("/api/delete_extension", post(handlers::extensions::delete_extension))
        .route("/api/get_extension_content", post(handlers::extensions::get_extension_content))
        .route("/api/scan_and_sync", post(handlers::extensions::scan_and_sync))
        .route("/api/list_skill_files", post(handlers::extensions::list_skill_files))
        // Settings / Dashboard
        .route("/api/get_dashboard_stats", post(handlers::settings::get_dashboard_stats))
        .route("/api/update_tags", post(handlers::settings::update_tags))
        .route("/api/batch_update_tags", post(handlers::settings::batch_update_tags))
        .route("/api/get_all_tags", post(handlers::settings::get_all_tags))
        .route("/api/update_pack", post(handlers::settings::update_pack))
        .route("/api/batch_update_pack", post(handlers::settings::batch_update_pack))
        .route("/api/get_all_packs", post(handlers::settings::get_all_packs))
        .route("/api/toggle_by_pack", post(handlers::settings::toggle_by_pack))
        .route("/api/read_config_file_preview", post(handlers::settings::read_config_file_preview));

    Router::new()
        .merge(api)
        .layer(middleware::from_fn_with_state(state.clone(), require_token))
        .with_state(state)
}

async fn health() -> Html<&'static str> {
    Html("ok")
}
