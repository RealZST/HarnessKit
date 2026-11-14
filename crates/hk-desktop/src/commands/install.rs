use super::AppState;
use hk_core::{HkError, deployer, manager, marketplace, models::*, scanner, service};
use tauri::State;

#[tauri::command]
pub async fn list_hermes_categories(state: State<'_, AppState>) -> Result<Vec<String>, HkError> {
    let adapters = state.adapters.clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok(adapters
            .iter()
            .find(|a| a.name() == "hermes")
            .map(|a| a.list_skill_categories())
            .unwrap_or_default())
    })
    .await
    .map_err(|e| HkError::Internal(e.to_string()))?
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
    tauri::async_runtime::spawn_blocking(move || {
        let source_path = std::path::Path::new(&path);
        if !source_path.is_dir() {
            return Err(HkError::Validation(
                "Selected path is not a directory".into(),
            ));
        }
        // Must contain SKILL.md at root or be a parent of skill subdirectories
        let skill_md = source_path.join("SKILL.md");
        if !skill_md.exists() {
            return Err(HkError::Validation(
                "Selected directory does not contain a SKILL.md file".into(),
            ));
        }

        let skill_name = scanner::parse_skill_name(&skill_md).unwrap_or_else(|| {
            source_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

        let agents: Vec<String> = if target_agents.is_empty() {
            manager::enabled_detected_agents(&store.lock(), &adapters)
        } else {
            target_agents
        };

        for agent_name in &agents {
            let a = adapters
                .iter()
                .find(|a| a.name() == agent_name.as_str())
                .ok_or_else(|| HkError::NotFound(format!("Agent '{}' not found", agent_name)))?;
            // A category only resolves a dir for category-aware agents (Hermes);
            // everything else returns None here and falls back to skill_dir_for.
            let target_dir = hermes_category
                .as_deref()
                .and_then(|cat| a.skill_dir_for_category(&target_scope, cat))
                .or_else(|| a.skill_dir_for(&target_scope))
                .ok_or_else(|| {
                    HkError::Internal(format!(
                        "Agent '{}' has no skill directory for scope {:?}",
                        agent_name, target_scope
                    ))
                })?;
            std::fs::create_dir_all(&target_dir)?;
            deployer::deploy_skill(source_path, &target_dir)?;
        }

        let result = manager::InstallResult {
            name: skill_name.clone(),
            was_update: false,
            revision: None,
            ..Default::default()
        };

        // Post-install: scan, sync, set meta, audit
        let git_source = scanner::detect_source_for(source_path);
        let meta = InstallMeta {
            install_type: "local".into(),
            url: git_source.url.clone().or_else(|| Some(path.clone())),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: git_source.commit_hash.clone(),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        let pack = git_source
            .url
            .as_deref()
            .and_then(hk_core::scanner::extract_pack_from_url);
        {
            let store = store.lock();
            service::post_install_sync(
                &store,
                &adapters,
                &agents,
                &skill_name,
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

#[tauri::command]
pub async fn install_from_git(
    state: State<'_, AppState>,
    url: String,
    target_agent: Option<String>,
    skill_id: Option<String>,
    target_scope: ConfigScope,
) -> Result<manager::InstallResult, HkError> {
    let store = state.store.clone();
    let adapters = state.adapters.clone();
    tauri::async_runtime::spawn_blocking(move || {
        hk_core::sanitize::validate_git_url(&url)
            .map_err(|e| HkError::Validation(e.to_string()))?;
        let (target_dir, agent_name) = if let Some(ref agent) = target_agent {
            let a = adapters
                .iter()
                .find(|a| a.name() == agent.as_str())
                .ok_or_else(|| HkError::NotFound(format!("Agent '{}' not found", agent)))?;
            let dir = a.skill_dir_for(&target_scope).ok_or_else(|| {
                HkError::Internal(format!(
                    "Agent '{}' has no skill directory for scope {:?}",
                    agent, target_scope
                ))
            })?;
            (dir, agent.clone())
        } else {
            // Fallback: first detected agent
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
        let sid = skill_id.as_deref().filter(|s| !s.is_empty());
        let result = manager::install_from_git_with_id(&url, &target_dir, sid)?;

        // Post-install: scan, sync, set meta, audit
        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some(url.clone()),
            url_resolved: None,
            branch: None,
            subpath: sid.map(|s| s.to_string()),
            revision: result.revision.clone(),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        let pack = meta
            .url
            .as_deref()
            .and_then(hk_core::scanner::extract_pack_from_url);
        let agents = vec![agent_name];
        {
            let store = store.lock();
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
    tauri::async_runtime::spawn_blocking(move || {
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
    .map_err(|e| HkError::Internal(e.to_string()))?
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
