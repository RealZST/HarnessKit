use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::state::WebState;

/// Middleware that checks Bearer token when auth is enabled.
/// Skips auth for static asset requests (no /api/ prefix).
pub async fn require_token(
    axum::extract::State(state): axum::extract::State<WebState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(expected) = &state.token else {
        return Ok(next.run(request).await);
    };

    let path = request.uri().path();
    if !path.starts_with("/api/") {
        return Ok(next.run(request).await);
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(h) if h.strip_prefix("Bearer ").is_some_and(|t| t == expected) => {
            Ok(next.run(request).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
