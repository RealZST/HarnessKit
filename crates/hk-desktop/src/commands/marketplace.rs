use super::AppState;
use hk_core::{HkError, manager, marketplace, models::*, service};
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
) -> Result<manager::InstallResult, HkError> {
    let store_clone = state.store.clone();
    let adapters = state.adapters.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<manager::InstallResult, HkError> {
        let (target_dir, agent_name) = if let Some(ref agent) = target_agent {
            let a = adapters
                .iter()
                .find(|a| a.name() == agent.as_str())
                .ok_or_else(|| HkError::Internal(format!("Agent '{}' not found", agent)))?;
            let dir = a.skill_dir_for(&target_scope).ok_or_else(|| {
                HkError::Internal(format!(
                    "Agent '{}' has no skill directory for scope {:?}",
                    agent, target_scope
                ))
            })?;
            (dir, agent.clone())
        } else {
            let a = adapters
                .iter()
                .find(|a| a.detect())
                .ok_or_else(|| HkError::Internal("No detected agent found".into()))?;
            let name = a.name().to_string();
            let dir = a.skill_dir_for(&target_scope).ok_or_else(|| {
                HkError::Internal(format!(
                    "Agent '{}' has no skill directory for scope {:?}",
                    name, target_scope
                ))
            })?;
            (dir, name)
        };
        std::fs::create_dir_all(&target_dir)?;
        let git_url = marketplace::git_url_for_source(&source);
        let sid = if skill_id.is_empty() {
            None
        } else {
            Some(skill_id.as_str())
        };

        // This is the blocking network call (git clone) — now safely in spawn_blocking
        let result = manager::install_from_git_with_id(&git_url, &target_dir, sid)?;

        // Post-install: scan, sync, set meta, audit
        let meta = InstallMeta {
            install_type: "marketplace".into(),
            url: Some(source.clone()),
            url_resolved: Some(git_url),
            branch: None,
            subpath: if skill_id.is_empty() {
                None
            } else {
                Some(skill_id.clone())
            },
            revision: result.revision.clone(),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        let pack = meta
            .url
            .as_deref()
            .and_then(hk_core::scanner::extract_pack_from_url)
            .or_else(|| {
                meta.url_resolved
                    .as_deref()
                    .and_then(hk_core::scanner::extract_pack_from_url)
            });
        let agents = vec![agent_name];
        {
            let store = store_clone.lock();
            service::post_install_sync(
                &store,
                &adapters,
                &agents,
                &result.name,
                Some(meta),
                pack.as_deref(),
                &target_scope,
            )?;
        }
        Ok(result)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}
