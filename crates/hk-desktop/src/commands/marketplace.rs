use super::AppState;
use super::helpers::audit_extension_by_name;
use hk_core::{HkError, adapter, manager, marketplace, models::*, scanner};
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
) -> Result<manager::InstallResult, HkError> {
    let store_clone = state.store.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<manager::InstallResult, HkError> {
        let adapters = adapter::all_adapters();
        let (target_dir, agent_name) = if let Some(ref agent) = target_agent {
            let a = adapters
                .iter()
                .find(|a| a.name() == agent.as_str())
                .ok_or_else(|| HkError::Internal(format!("Agent '{}' not found", agent)))?;
            let dir = a.skill_dirs().into_iter().next().ok_or_else(|| {
                HkError::Internal(format!("No skill directory for agent '{}'", agent))
            })?;
            (dir, agent.clone())
        } else {
            let a = adapters
                .iter()
                .find(|a| a.detect())
                .ok_or_else(|| HkError::Internal("No detected agent found".into()))?;
            let name = a.name().to_string();
            let dir = a
                .skill_dirs()
                .into_iter()
                .next()
                .ok_or_else(|| HkError::Internal("No agent skill directory found".into()))?;
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

        // Re-scan affected agent only and persist
        let extensions: Vec<Extension> =
            if let Some(a) = adapters.iter().find(|a| a.name() == agent_name) {
                scanner::scan_adapter(a.as_ref())
            } else {
                Vec::new()
            };
        {
            let store = store_clone.lock();
            store.sync_extensions_for_agent(&agent_name, &extensions)?;
            let ext_id = scanner::stable_id_for(&result.name, "skill", &agent_name);
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
            if let Err(e) = store.set_install_meta(&ext_id, &meta) {
                eprintln!("[hk] warning: {e}");
            }
            let pack = meta
                .url
                .as_deref()
                .and_then(hk_core::scanner::extract_pack_from_url)
                .or_else(|| {
                    meta.url_resolved
                        .as_deref()
                        .and_then(hk_core::scanner::extract_pack_from_url)
                });
            if let Some(ref p) = pack
                && let Err(e) = store.update_pack(&ext_id, Some(p))
            {
                eprintln!("[hk] warning: {e}");
            }
        }

        // Audit the newly installed extension (no lock held)
        let audit_results = audit_extension_by_name(&result.name, &extensions, &adapters);
        if !audit_results.is_empty() {
            let store = store_clone.lock();
            for r in &audit_results {
                if let Err(e) = store.insert_audit_result(r) {
                    eprintln!("[hk] warning: {e}");
                }
            }
        }
        Ok(result)
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
}
