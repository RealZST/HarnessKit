use hk_core::{adapter, manager, marketplace, models::*, scanner};
use tauri::State;
use super::AppState;
use super::helpers::audit_extension_by_name;

#[tauri::command]
pub async fn search_marketplace(query: String, kind: String, limit: Option<usize>) -> Result<Vec<marketplace::MarketplaceItem>, String> {
    let lim = limit.unwrap_or(20);
    match kind.as_str() {
        "mcp" => marketplace::search_servers_async(&query, lim).await,
        _ => marketplace::search_skills_async(&query, lim).await,
    }.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trending_marketplace(kind: String, limit: Option<usize>) -> Result<Vec<marketplace::MarketplaceItem>, String> {
    let lim = limit.unwrap_or(10);
    match kind.as_str() {
        "mcp" => marketplace::trending_servers_async(lim).await,
        _ => marketplace::trending_skills_async(lim).await,
    }.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_skill_preview(source: String, skill_id: String, git_url: Option<String>) -> Result<String, String> {
    marketplace::fetch_skill_content_async(&source, &skill_id, git_url.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_cli_readme(source: String) -> Result<String, String> {
    marketplace::fetch_cli_readme_async(&source).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_skill_audit(source: String, skill_id: String) -> Result<Option<marketplace::SkillAuditInfo>, String> {
    marketplace::fetch_audit_info_async(&source, &skill_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_from_marketplace(state: State<AppState>, source: String, skill_id: String, target_agent: Option<String>) -> Result<manager::InstallResult, String> {
    let adapters = adapter::all_adapters();
    let (target_dir, agent_name) = if let Some(ref agent) = target_agent {
        let a = adapters.iter()
            .find(|a| a.name() == agent.as_str())
            .ok_or_else(|| format!("Agent '{}' not found", agent))?;
        let dir = a.skill_dirs().into_iter().next()
            .ok_or_else(|| format!("No skill directory for agent '{}'", agent))?;
        (dir, agent.clone())
    } else {
        let a = adapters.iter().find(|a| a.detect())
            .ok_or_else(|| "No detected agent found".to_string())?;
        let name = a.name().to_string();
        let dir = a.skill_dirs().into_iter().next()
            .ok_or_else(|| "No agent skill directory found".to_string())?;
        (dir, name)
    };
    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    let git_url = marketplace::git_url_for_source(&source);
    let sid = if skill_id.is_empty() { None } else { Some(skill_id.as_str()) };
    let result = manager::install_from_git_with_id(&git_url, &target_dir, sid).map_err(|e| e.to_string())?;
    // Re-scan affected agent only and persist
    let extensions: Vec<Extension> = if let Some(a) = adapters.iter().find(|a| a.name() == agent_name) {
        scanner::scan_adapter(a.as_ref())
    } else {
        Vec::new()
    };
    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.sync_extensions_for_agent(&agent_name, &extensions).map_err(|e| e.to_string())?;
        // Persist install source metadata
        let ext_id = scanner::stable_id_for(&result.name, "skill", &agent_name);
        let meta = InstallMeta {
            install_type: "marketplace".into(),
            url: Some(source.clone()),
            url_resolved: Some(git_url),
            branch: None,
            subpath: if skill_id.is_empty() { None } else { Some(skill_id.clone()) },
            revision: result.revision.clone(),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        let _ = store.set_install_meta(&ext_id, &meta);
    } // Lock released before slow file I/O

    // Audit the newly installed extension (no lock held)
    let audit_results = audit_extension_by_name(&result.name, &extensions, &adapters);
    if !audit_results.is_empty() {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        for r in &audit_results {
            let _ = store.insert_audit_result(r);
        }
    }
    Ok(result)
}
