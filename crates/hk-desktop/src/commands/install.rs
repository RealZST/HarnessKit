use super::AppState;
use super::helpers::blocking;
use hk_core::install::{self, ScanResult, Target};
use hk_core::{HkError, manager, marketplace, models::*, repositories, service};
use tauri::State;

#[tauri::command]
pub async fn list_hermes_categories(state: State<'_, AppState>) -> Result<Vec<String>, HkError> {
    let adapters = state.adapters.clone();
    blocking(move || {
        Ok(adapters
            .iter()
            .find(|a| a.name() == "hermes")
            .map(|a| a.list_skill_categories())
            .unwrap_or_default())
    })
    .await
}

#[tauri::command]
pub async fn install_from_local(
    state: State<'_, AppState>,
    path: String,
    target_agents: Vec<String>,
    target_scope: ConfigScope,
    hermes_category: Option<String>,
) -> Result<manager::InstallResult, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    blocking(move || {
        let target = Target {
            agents: &target_agents,
            scope: &target_scope,
            hermes_category: hermes_category.as_deref(),
        };
        install::install_from_local(&store, &adapters, &path, &target)
    })
    .await
}

#[tauri::command]
pub async fn scan_git_repo(
    state: State<'_, AppState>,
    url: String,
    target_agents: Vec<String>,
    target_scope: ConfigScope,
) -> Result<ScanResult, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    let pending = state.pending_clones.clone();
    blocking(move || {
        let target = Target {
            agents: &target_agents,
            scope: &target_scope,
            hermes_category: None,
        };
        install::scan_git_repo(&store, &adapters, &pending, &url, &target)
    })
    .await
}

#[tauri::command]
pub async fn install_scanned_skills(
    state: State<'_, AppState>,
    clone_id: String,
    skill_ids: Vec<String>,
    target_agents: Vec<String>,
    target_scope: ConfigScope,
    track: bool,
) -> Result<Vec<manager::InstallResult>, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    let pending = state.pending_clones.clone();
    blocking(move || {
        let target = Target {
            agents: &target_agents,
            scope: &target_scope,
            hermes_category: None,
        };
        let cache_root = track.then(repositories::cache_root);
        install::install_scanned_skills(
            &store,
            &adapters,
            &pending,
            &clone_id,
            &skill_ids,
            &target,
            cache_root.as_deref(),
        )
    })
    .await
}

// --- Cross-agent deploy command ---

#[tauri::command]
pub async fn install_to_agent(
    state: State<'_, AppState>,
    extension_id: String,
    target_agent: String,
    hermes_category: Option<String>,
    target_scope: ConfigScope,
) -> Result<String, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    blocking(move || {
        service::install_to_agent(
            &store,
            &adapters,
            &extension_id,
            &target_agent,
            hermes_category.as_deref(),
            &target_scope,
        )
    })
    .await
}

// --- CLI commands ---

#[tauri::command]
pub fn get_cli_with_children(
    state: State<AppState>,
    cli_id: String,
) -> Result<(Extension, Vec<Extension>), HkError> {
    let store = state.store.lock();
    let cli = store
        .get_extension(&cli_id)?
        .ok_or_else(|| HkError::NotFound(format!("CLI not found: {}", cli_id)))?;
    let children = store.get_child_skills(&cli_id)?;
    Ok((cli, children))
}

#[tauri::command]
pub async fn list_cli_marketplace() -> Result<Vec<marketplace::MarketplaceItem>, HkError> {
    tauri::async_runtime::spawn_blocking(marketplace::list_cli_registry)
        .await
        .map_err(|e| HkError::Internal(e.to_string()))
}
