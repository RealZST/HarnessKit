use axum::{
    Router,
    middleware,
    routing::get,
    response::Html,
};
use crate::auth::require_token;
use crate::state::WebState;

pub fn build_router(state: WebState) -> Router {
    let api = Router::new()
        .route("/api/health", get(health));

    Router::new()
        .merge(api)
        .layer(middleware::from_fn_with_state(state.clone(), require_token))
        .with_state(state)
}

async fn health() -> Html<&'static str> {
    Html("ok")
}
