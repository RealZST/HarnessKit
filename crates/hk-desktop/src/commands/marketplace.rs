use super::AppState;
use hk_core::{HkError, install, manager, marketplace, models::*};
use tauri::State;

#[tauri::command]
pub async fn search_marketplace(
    query: String,
    kind: String,
    limit: Option<usize>,
) -> Result<Vec<marketplace::MarketplaceItem>, HkError> {
    let lim = limit.unwrap_or(20);
    match kind.as_str() {
        "mcp" => marketplace::search_servers_async(&query, lim).await,
        _ => marketplace::search_skills_async(&query, lim).await,
    }
}

#[tauri::command]
pub async fn trending_marketplace(
    kind: String,
    limit: Option<usize>,
) -> Result<Vec<marketplace::MarketplaceItem>, HkError> {
    let lim = limit.unwrap_or(10);
    match kind.as_str() {
        "mcp" => marketplace::trending_servers_async(lim).await,
        _ => marketplace::trending_skills_async(lim).await,
    }
}

#[tauri::command]
pub async fn fetch_skill_preview(
    source: String,
    skill_id: String,
    git_url: Option<String>,
) -> Result<String, HkError> {
    marketplace::fetch_skill_content_async(&source, &skill_id, git_url.as_deref()).await
}

#[tauri::command]
pub async fn fetch_cli_readme(source: String) -> Result<String, HkError> {
    marketplace::fetch_cli_readme_async(&source).await
}

#[tauri::command]
pub async fn fetch_skill_audit(
    source: String,
    skill_id: String,
) -> Result<Option<marketplace::SkillAuditInfo>, HkError> {
    marketplace::fetch_audit_info_async(&source, &skill_id).await
}

#[tauri::command]
pub async fn install_from_marketplace(
    state: State<'_, AppState>,
    source: String,
    skill_id: String,
    target_agent: Option<String>,
    target_scope: ConfigScope,
    hermes_category: Option<String>,
) -> Result<manager::InstallResult, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let agents = Vec::from_iter(target_agent);
        let target = install::Target {
            agents: &agents,
            scope: &target_scope,
            hermes_category: hermes_category.as_deref(),
        };
        install::install_from_marketplace(&store, &adapters, &source, &skill_id, &target)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}
