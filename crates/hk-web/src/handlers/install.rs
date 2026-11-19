use axum::Json;
use axum::extract::State;
use hk_core::install::{self, ScanResult, Target};
use hk_core::models::*;
use hk_core::{manager, repositories, scanner, service, updates};
use serde::Deserialize;

use crate::router::{ApiError, blocking};
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

#[derive(Deserialize)]
pub struct InstallFromMarketplaceParams {
    pub source: String,
    pub skill_id: String,
    pub target_agent: String,
    pub target_scope: ConfigScope,
    pub hermes_category: Option<String>,
}

pub async fn install_from_marketplace(
    State(state): State<WebState>,
    Json(params): Json<InstallFromMarketplaceParams>,
) -> Result<manager::InstallResult> {
    blocking(move || {
        let agents = [params.target_agent];
        let target = Target {
            agents: &agents,
            scope: &params.target_scope,
            hermes_category: params.hermes_category.as_deref(),
        };
        install::install_from_marketplace(
            &state.store,
            &state.adapters,
            &params.source,
            &params.skill_id,
            &target,
        )
    })
    .await
}

#[derive(Deserialize)]
pub struct InstallFromLocalParams {
    pub path: String,
    pub target_agents: Vec<String>,
    pub target_scope: ConfigScope,
    pub hermes_category: Option<String>,
}

pub async fn install_from_local(
    State(state): State<WebState>,
    Json(params): Json<InstallFromLocalParams>,
) -> Result<manager::InstallResult> {
    blocking(move || {
        let target = Target {
            agents: &params.target_agents,
            scope: &params.target_scope,
            hermes_category: params.hermes_category.as_deref(),
        };
        install::install_from_local(&state.store, &state.adapters, &params.path, &target)
    })
    .await
}

#[derive(Deserialize)]
pub struct InstallToAgentParams {
    pub extension_id: String,
    pub target_agent: String,
    pub hermes_category: Option<String>,
    /// Required, like every sibling install endpoint: an implicit Global
    /// default could silently misroute a project-scope install.
    pub target_scope: ConfigScope,
}

pub async fn install_to_agent(
    State(state): State<WebState>,
    Json(params): Json<InstallToAgentParams>,
) -> Result<String> {
    blocking(move || {
        service::install_to_agent(
            &state.store,
            &state.adapters,
            &params.extension_id,
            &params.target_agent,
            params.hermes_category.as_deref(),
            &params.target_scope,
        )
    })
    .await
}

#[derive(Deserialize)]
pub struct UpdateExtensionParams {
    pub id: String,
}

pub async fn update_extension(
    State(state): State<WebState>,
    Json(params): Json<UpdateExtensionParams>,
) -> Result<manager::InstallResult> {
    blocking(move || updates::update_extension(&state.store, &params.id)).await
}

#[derive(Deserialize)]
pub struct ScanGitRepoParams {
    pub url: String,
    pub target_agents: Vec<String>,
    pub target_scope: ConfigScope,
    pub hermes_category: Option<String>,
}

pub async fn scan_git_repo(
    State(state): State<WebState>,
    Json(params): Json<ScanGitRepoParams>,
) -> Result<ScanResult> {
    blocking(move || {
        let target = Target {
            agents: &params.target_agents,
            scope: &params.target_scope,
            hermes_category: params.hermes_category.as_deref(),
        };
        install::scan_git_repo(
            &state.store,
            &state.adapters,
            &state.pending_clones,
            &params.url,
            &target,
        )
    })
    .await
}

#[derive(Deserialize)]
pub struct InstallScannedSkillsParams {
    pub clone_id: String,
    pub skill_ids: Vec<String>,
    pub target_agents: Vec<String>,
    pub target_scope: ConfigScope,
    pub hermes_category: Option<String>,
    /// Follow the repository, with this clone as its checkout.
    pub track: bool,
}

pub async fn install_scanned_skills(
    State(state): State<WebState>,
    Json(params): Json<InstallScannedSkillsParams>,
) -> Result<Vec<manager::InstallResult>> {
    blocking(move || {
        let target = Target {
            agents: &params.target_agents,
            scope: &params.target_scope,
            hermes_category: params.hermes_category.as_deref(),
        };
        let cache_root = params.track.then(repositories::cache_root);
        install::install_scanned_skills(
            &state.store,
            &state.adapters,
            &state.pending_clones,
            &params.clone_id,
            &params.skill_ids,
            &target,
            cache_root.as_deref(),
        )
    })
    .await
}

#[derive(Deserialize)]
pub struct GetCliWithChildrenParams {
    pub cli_id: String,
}

pub async fn get_cli_with_children(
    State(state): State<WebState>,
    Json(params): Json<GetCliWithChildrenParams>,
) -> Result<(Extension, Vec<Extension>)> {
    blocking(move || {
        let store = state.store.lock();
        let cli = store
            .get_extension(&params.cli_id)?
            .ok_or_else(|| hk_core::HkError::NotFound("CLI extension not found".into()))?;
        let children = store.get_child_skills(&params.cli_id)?;
        Ok((cli, children))
    })
    .await
}

pub async fn check_updates(State(state): State<WebState>) -> Result<Vec<(String, UpdateStatus)>> {
    blocking(move || updates::check_updates(&state.store)).await
}

pub async fn get_cached_update_statuses(
    State(state): State<WebState>,
) -> Result<Vec<(String, UpdateStatus)>> {
    blocking(move || updates::cached_update_statuses(&state.store.lock())).await
}

#[derive(Deserialize)]
pub struct GetSkillLocationsParams {
    pub name: String,
}

pub async fn get_skill_locations(
    State(state): State<WebState>,
    Json(params): Json<GetSkillLocationsParams>,
) -> Result<Vec<(String, String, Option<String>)>> {
    blocking(move || {
        let projects = state.store.lock().list_project_tuples();
        // UI listing — every scope.
        let locations = scanner::skill_locations(&params.name, &state.adapters, &projects, None);
        let result: Vec<(String, String, Option<String>)> = locations
            .into_iter()
            .map(|(agent, path)| {
                let symlink = std::fs::read_link(&path)
                    .ok()
                    .map(|t| t.to_string_lossy().to_string());
                (agent, path.to_string_lossy().to_string(), symlink)
            })
            .collect();
        Ok(result)
    })
    .await
}

pub async fn list_hermes_categories(
    State(state): State<WebState>,
    Json(_): Json<serde_json::Value>,
) -> Result<Vec<String>> {
    blocking(move || {
        Ok(state
            .adapters
            .iter()
            .find(|a| a.name() == "hermes")
            .map(|a| a.list_skill_categories())
            .unwrap_or_default())
    })
    .await
}
