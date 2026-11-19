use axum::Json;
use axum::extract::State;
use hk_core::models::{
    IgnoreRepositoryItemRequest, InstallRepositoryItemRequest, RepositoryWithItems,
};
use hk_core::repositories;
use serde::Deserialize;

use super::{IdBody, ReqBody};
use crate::router::{ApiError, blocking};
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

#[derive(Deserialize)]
pub struct UrlBody {
    pub url: String,
}

pub async fn list_repositories(State(state): State<WebState>) -> Result<Vec<RepositoryWithItems>> {
    blocking(move || state.store.lock().list_repositories_with_items()).await
}

pub async fn add_repository(
    State(state): State<WebState>,
    Json(body): Json<UrlBody>,
) -> Result<String> {
    blocking(move || {
        repositories::add_repository(&state.store, &repositories::cache_root(), &body.url)
    })
    .await
}

pub async fn sync_repository(
    State(state): State<WebState>,
    Json(body): Json<IdBody>,
) -> Result<()> {
    blocking(move || {
        repositories::sync_repository(&state.store, &repositories::cache_root(), &body.id)
    })
    .await
}

pub async fn delete_repository(
    State(state): State<WebState>,
    Json(body): Json<IdBody>,
) -> Result<()> {
    blocking(move || {
        repositories::delete_repository(&state.store, &repositories::cache_root(), &body.id)
    })
    .await
}

pub async fn ignore_repository_item(
    State(state): State<WebState>,
    Json(body): Json<ReqBody<IgnoreRepositoryItemRequest>>,
) -> Result<()> {
    blocking(move || {
        let req = body.req;
        state
            .store
            .lock()
            .set_repository_item_ignored(&req.repo_id, &req.item_id, req.ignored)
    })
    .await
}

pub async fn install_repository_item(
    State(state): State<WebState>,
    Json(body): Json<ReqBody<InstallRepositoryItemRequest>>,
) -> Result<()> {
    blocking(move || {
        repositories::install_item(
            &state.store,
            &state.adapters,
            &repositories::cache_root(),
            &body.req,
        )
    })
    .await
}
