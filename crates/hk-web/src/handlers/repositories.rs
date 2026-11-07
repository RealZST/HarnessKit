use axum::Json;
use axum::extract::State;
use hk_core::{manager, models::*};
use serde::Deserialize;

use crate::router::{ApiError, blocking};
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

#[derive(Deserialize)]
pub struct IdBody {
    pub id: String,
}

#[derive(Deserialize)]
pub struct AddRepositoryBody {
    pub url: String,
    pub name: Option<String>,
    pub branch: Option<String>,
    pub auth_token: Option<String>,
}

#[derive(Deserialize)]
pub struct SetRepositoryItemStatusBody {
    pub repo_id: String,
    pub item_id: String,
    pub kind: Option<String>,
    pub status: String,
}

#[derive(Deserialize)]
pub struct InstallFromRepositoryBody {
    pub repo_id: String,
    pub item_id: String,
    pub target_agents: Option<Vec<String>>,
    pub target_scope: Option<ConfigScope>,
    pub hermes_category: Option<String>,
}

pub async fn list_repositories(State(state): State<WebState>) -> Result<Vec<RepositoryWithItems>> {
    blocking(move || {
        let store = state.store.lock();
        store.list_repositories_with_items()
    })
    .await
}

pub async fn add_repository(
    State(state): State<WebState>,
    Json(body): Json<AddRepositoryBody>,
) -> Result<Repository> {
    blocking(move || {
        let store = state.store.lock();
        let new_repo = NewRepository {
            id: None,
            url: body.url,
            name: body.name,
            branch: body.branch,
            auth_token: body.auth_token,
        };
        store.add_repository(&new_repo)
    })
    .await
}

pub async fn delete_repository(
    State(state): State<WebState>,
    Json(body): Json<IdBody>,
) -> Result<bool> {
    blocking(move || {
        let store = state.store.lock();
        manager::delete_repository_with_cache(&store, &body.id)
    })
    .await
}

pub async fn sync_repository(
    State(state): State<WebState>,
    Json(body): Json<IdBody>,
) -> Result<RepoSyncResult> {
    blocking(move || {
        let store = state.store.lock();
        manager::sync_repository(&store, &body.id)
    })
    .await
}

pub async fn sync_all_repositories(State(state): State<WebState>) -> Result<Vec<RepoSyncResult>> {
    blocking(move || {
        let store = state.store.lock();
        manager::sync_all_repositories(&store)
    })
    .await
}

pub async fn set_repository_item_status(
    State(state): State<WebState>,
    Json(body): Json<SetRepositoryItemStatusBody>,
) -> Result<()> {
    blocking(move || {
        let store = state.store.lock();
        let kind = body
            .kind
            .as_deref()
            .and_then(|k| k.parse().ok())
            .unwrap_or(ExtensionKind::Skill);
        store.set_repository_item_status(&body.repo_id, kind, &body.item_id, &body.status)
    })
    .await
}

pub async fn install_from_repository(
    State(state): State<WebState>,
    Json(body): Json<InstallFromRepositoryBody>,
) -> Result<Vec<manager::InstallResult>> {
    blocking(move || {
        let store = state.store.lock();
        let scope = body.target_scope.unwrap_or(ConfigScope::Global);
        let agents = body.target_agents.unwrap_or_default();
        manager::install_from_repo_item(
            &store,
            &state.adapters,
            &body.repo_id,
            &body.item_id,
            &agents,
            scope,
            body.hermes_category.as_deref(),
        )
    })
    .await
}
