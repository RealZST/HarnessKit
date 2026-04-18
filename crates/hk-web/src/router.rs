use axum::{
    Router,
    body::Body,
    middleware,
    routing::{get, post},
    response::{Html, IntoResponse},
    http::{Method, StatusCode, Uri, header},
    Json,
};
use hk_core::HkError;
use rust_embed::RustEmbed;
use tower_http::cors::{CorsLayer, Any};

use crate::auth::require_token;
use crate::handlers;
use crate::state::WebState;

#[derive(RustEmbed)]
#[folder = "../../dist/"]
struct FrontendAssets;

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
        .route("/api/read_config_file_preview", post(handlers::settings::read_config_file_preview))
        // Agents
        .route("/api/list_agents", post(handlers::agents::list_agents))
        .route("/api/set_agent_enabled", post(handlers::agents::set_agent_enabled))
        .route("/api/update_agent_order", post(handlers::agents::update_agent_order))
        .route("/api/update_agent_path", post(handlers::agents::update_agent_path))
        .route("/api/list_agent_configs", post(handlers::agents::list_agent_configs))
        .route("/api/add_custom_config_path", post(handlers::agents::add_custom_config_path))
        .route("/api/update_custom_config_path", post(handlers::agents::update_custom_config_path))
        .route("/api/remove_custom_config_path", post(handlers::agents::remove_custom_config_path))
        // Audit
        .route("/api/list_audit_results", post(handlers::audit::list_audit_results))
        .route("/api/run_audit", post(handlers::audit::run_audit))
        // Projects
        .route("/api/list_projects", post(handlers::projects::list_projects))
        .route("/api/add_project", post(handlers::projects::add_project))
        .route("/api/remove_project", post(handlers::projects::remove_project))
        .route("/api/discover_projects", post(handlers::projects::discover_projects))
        // Marketplace
        .route("/api/search_marketplace", post(handlers::marketplace::search_marketplace))
        .route("/api/trending_marketplace", post(handlers::marketplace::trending_marketplace))
        .route("/api/list_cli_marketplace", post(handlers::marketplace::list_cli_marketplace))
        .route("/api/fetch_skill_preview", post(handlers::marketplace::fetch_skill_preview))
        .route("/api/fetch_cli_readme", post(handlers::marketplace::fetch_cli_readme))
        .route("/api/fetch_skill_audit", post(handlers::marketplace::fetch_skill_audit))
        // Install
        .route("/api/install_from_git", post(handlers::install::install_from_git))
        .route("/api/install_from_marketplace", post(handlers::install::install_from_marketplace))
        .route("/api/install_from_local", post(handlers::install::install_from_local))
        .route("/api/install_to_agent", post(handlers::install::install_to_agent))
        .route("/api/update_extension", post(handlers::install::update_extension))
        .route("/api/check_updates", post(handlers::install::check_updates))
        .route("/api/get_cached_update_statuses", post(handlers::install::get_cached_update_statuses))
        .route("/api/get_cli_with_children", post(handlers::install::get_cli_with_children))
        .route("/api/get_skill_locations", post(handlers::install::get_skill_locations));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    // Layer order matters: Axum applies layers inside-out.
    // Auth must be INNER (applied first), CORS must be OUTER (applied last)
    // so that OPTIONS preflight requests get CORS headers without hitting auth.
    Router::new()
        .merge(api)
        .fallback(serve_frontend)
        .layer(middleware::from_fn_with_state(state.clone(), require_token))
        .layer(cors)
        .with_state(state)
}

async fn health() -> Html<&'static str> {
    Html("ok")
}

async fn serve_frontend(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    // Try exact path first, then fall back to index.html (SPA routing)
    let (file, mime_path) = match FrontendAssets::get(path) {
        Some(f) => (Some(f), path),
        None => (FrontendAssets::get("index.html"), "index.html"),
    };

    match file {
        Some(content) => {
            let mime = mime_guess::from_path(mime_path)
                .first_or_octet_stream()
                .to_string();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime)],
                Body::from(content.data.to_vec()),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
