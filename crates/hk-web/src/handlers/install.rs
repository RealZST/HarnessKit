use axum::extract::State;
use axum::Json;
use hk_core::models::*;
use hk_core::{deployer, manager, marketplace, sanitize, scanner, service};
use serde::Deserialize;

use crate::router::{blocking, ApiError};
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

type ExtensionUpdateBuckets = (Vec<(String, String, InstallMeta)>, Vec<(String, String)>);

#[derive(Deserialize)]
pub struct InstallFromGitParams {
    pub url: String,
    pub target_agent: Option<String>,
    pub skill_id: Option<String>,
}

pub async fn install_from_git(
    State(state): State<WebState>,
    Json(params): Json<InstallFromGitParams>,
) -> Result<manager::InstallResult> {
    sanitize::validate_git_url(&params.url)
        .map_err(|e| ApiError::from(hk_core::HkError::Validation(e.to_string())))?;

    blocking(move || {
        let (target_dir, agent_name) = if let Some(ref agent) = params.target_agent {
            let a = state.adapters.iter()
                .find(|a| a.name() == agent.as_str())
                .ok_or_else(|| hk_core::HkError::NotFound(format!("Agent '{}' not found", agent)))?;
            let dir = a.skill_dirs().into_iter().next().ok_or_else(|| {
                hk_core::HkError::Internal(format!("No skill directory for agent '{}'", agent))
            })?;
            (dir, agent.clone())
        } else {
            let a = state.adapters.iter().find(|a| a.detect())
                .ok_or_else(|| hk_core::HkError::Internal("No detected agent found".into()))?;
            let name = a.name().to_string();
            let dir = a.skill_dirs().into_iter().next()
                .ok_or_else(|| hk_core::HkError::Internal("No agent skill directory found".into()))?;
            (dir, name)
        };

        std::fs::create_dir_all(&target_dir)?;
        let sid = params.skill_id.as_deref().filter(|s| !s.is_empty());
        let result = manager::install_from_git_with_id(&params.url, &target_dir, sid)?;

        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some(params.url.clone()),
            url_resolved: None,
            branch: None,
            subpath: sid.map(|s| s.to_string()),
            revision: result.revision.clone(),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        let pack = meta.url.as_deref().and_then(scanner::extract_pack_from_url);
        let agents = vec![agent_name];
        {
            let store = state.store.lock();
            service::post_install_sync(
                &store, &state.adapters, &agents, &result.name,
                Some(meta), pack.as_deref(),
            )?;
        }

        Ok(result)
    }).await
}

#[derive(Deserialize)]
pub struct InstallFromMarketplaceParams {
    pub source: String,
    pub skill_id: String,
    pub target_agent: Option<String>,
}

pub async fn install_from_marketplace(
    State(state): State<WebState>,
    Json(params): Json<InstallFromMarketplaceParams>,
) -> Result<manager::InstallResult> {
    blocking(move || {
        let git_url = marketplace::git_url_for_source(&params.source);
        let (target_dir, agent_name) = if let Some(ref agent) = params.target_agent {
            let a = state.adapters.iter()
                .find(|a| a.name() == agent.as_str())
                .ok_or_else(|| hk_core::HkError::Internal(format!("Agent '{}' not found", agent)))?;
            let dir = a.skill_dirs().into_iter().next().ok_or_else(|| {
                hk_core::HkError::Internal(format!("No skill directory for agent '{}'", agent))
            })?;
            (dir, agent.clone())
        } else {
            let a = state.adapters.iter().find(|a| a.detect())
                .ok_or_else(|| hk_core::HkError::Internal("No detected agent found".into()))?;
            let name = a.name().to_string();
            let dir = a.skill_dirs().into_iter().next()
                .ok_or_else(|| hk_core::HkError::Internal("No agent skill directory found".into()))?;
            (dir, name)
        };
        std::fs::create_dir_all(&target_dir)?;
        let sid = if params.skill_id.is_empty() { None } else { Some(params.skill_id.as_str()) };
        let result = manager::install_from_git_with_id(&git_url, &target_dir, sid)?;

        let meta = InstallMeta {
            install_type: "marketplace".into(),
            url: Some(params.source.clone()),
            url_resolved: Some(git_url),
            branch: None,
            subpath: if params.skill_id.is_empty() { None } else { Some(params.skill_id.clone()) },
            revision: result.revision.clone(),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        let pack = meta.url.as_deref().and_then(scanner::extract_pack_from_url)
            .or_else(|| meta.url_resolved.as_deref().and_then(scanner::extract_pack_from_url));
        let agents = vec![agent_name];
        {
            let store = state.store.lock();
            service::post_install_sync(
                &store, &state.adapters, &agents, &result.name,
                Some(meta), pack.as_deref(),
            )?;
        }

        Ok(result)
    }).await
}

#[derive(Deserialize)]
pub struct InstallFromLocalParams {
    pub path: String,
    pub target_agents: Vec<String>,
}

pub async fn install_from_local(
    State(state): State<WebState>,
    Json(params): Json<InstallFromLocalParams>,
) -> Result<manager::InstallResult> {
    blocking(move || {
        let source_path = std::path::Path::new(&params.path);
        if !source_path.is_dir() {
            return Err(hk_core::HkError::Validation("Selected path is not a directory".into()));
        }
        let skill_md = source_path.join("SKILL.md");
        if !skill_md.exists() {
            return Err(hk_core::HkError::Validation(
                "Selected directory does not contain a SKILL.md file".into(),
            ));
        }

        let skill_name = scanner::parse_skill_name(&skill_md).unwrap_or_else(|| {
            source_path.file_name().unwrap_or_default().to_string_lossy().to_string()
        });

        let agents: Vec<String> = if params.target_agents.is_empty() {
            state.adapters.iter().filter(|a| a.detect())
                .map(|a| a.name().to_string()).collect()
        } else {
            params.target_agents
        };

        for agent_name in &agents {
            let a = state.adapters.iter()
                .find(|a| a.name() == agent_name.as_str())
                .ok_or_else(|| hk_core::HkError::NotFound(format!("Agent '{}' not found", agent_name)))?;
            let target_dir = a.skill_dirs().into_iter().next().ok_or_else(|| {
                hk_core::HkError::Internal(format!("No skill directory for agent '{}'", agent_name))
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
            url: git_source.url.clone().or_else(|| Some(params.path.clone())),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: git_source.commit_hash.clone(),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        let pack = git_source.url.as_deref()
            .and_then(scanner::extract_pack_from_url);
        {
            let store = state.store.lock();
            service::post_install_sync(
                &store, &state.adapters, &agents, &skill_name,
                Some(meta), pack.as_deref(),
            )?;
        }

        Ok(result)
    }).await
}

#[derive(Deserialize)]
pub struct InstallToAgentParams {
    pub extension_id: String,
    pub target_agent: String,
}

pub async fn install_to_agent(
    State(state): State<WebState>,
    Json(params): Json<InstallToAgentParams>,
) -> Result<String> {
    blocking(move || {
        // Capture (name, kind) up front so the return value is bit-perfect
        // parity with the pre-extraction code: the original bound `ext`
        // BEFORE the deploy and reused it for the stable_id, so we do the
        // same. A re-fetch after sync would change error behavior in the
        // (vanishingly rare) case where the source extension disappears
        // mid-deploy.
        let (ext_name, ext_kind) = {
            let store = state.store.lock();
            let ext = store.get_extension(&params.extension_id)?
                .ok_or_else(|| hk_core::HkError::NotFound("Extension not found".into()))?;
            (ext.name, ext.kind)
        };

        // Run the cross-agent deploy via the shared service helper. We
        // discard its returned `deployed_name` because the web frontend
        // wants the canonical extension ID instead — that's what it uses
        // for the "navigate to the new extension" handoff.
        service::install_to_agent(
            &state.store,
            &state.adapters,
            &params.extension_id,
            &params.target_agent,
        )?;

        // Web-only: re-scan + sync after a successful deploy so the new
        // extension shows up in the next list_extensions response without
        // the user having to manually refresh. Desktop relies on a separate
        // scan_and_sync round-trip from the frontend.
        let store = state.store.lock();
        let projects = store.list_project_tuples();
        let scanned = scanner::scan_all(&state.adapters, &projects);
        store.sync_extensions(&scanned)?;

        Ok(scanner::stable_id_for(&ext_name, ext_kind.as_str(), &params.target_agent))
    }).await
}

#[derive(Deserialize)]
pub struct UpdateExtensionParams {
    pub id: String,
}

pub async fn update_extension(
    State(state): State<WebState>,
    Json(params): Json<UpdateExtensionParams>,
) -> Result<manager::InstallResult> {
    blocking(move || {
        let (ext, install_meta) = {
            let store = state.store.lock();
            let ext = store.get_extension(&params.id)?
                .ok_or_else(|| hk_core::HkError::NotFound(format!("Extension '{}' not found", params.id)))?;
            let meta = ext.install_meta.clone().ok_or_else(|| {
                hk_core::HkError::NotFound("Extension has no install metadata — cannot update".into())
            })?;
            match meta.install_type.as_str() {
                "git" | "marketplace" => {}
                _ => {
                    return Err(hk_core::HkError::Validation(format!(
                        "Extensions with install type '{}' cannot be updated", meta.install_type
                    )));
                }
            }
            (ext, meta)
        };

        let url = install_meta.url_resolved.as_deref()
            .or(install_meta.url.as_deref())
            .ok_or_else(|| hk_core::HkError::NotFound("Extension has no remote URL".into()))?;

        // Clone the repo once
        let temp = tempfile::tempdir().map_err(|e| hk_core::HkError::Internal(e.to_string()))?;
        let clone_dir = temp.path().join("repo");
        let output = std::process::Command::new("git")
            .args(["clone", "--depth", "1", "--", url, &clone_dir.to_string_lossy()])
            .output()
            .map_err(|e| hk_core::HkError::CommandFailed(format!("Failed to run git clone: {}", e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(hk_core::HkError::CommandFailed(format!("git clone failed: {}", stderr.trim())));
        }
        let revision = manager::capture_git_revision_pub(&clone_dir);

        let skill_name = &ext.name;
        let skill_source = match manager::find_skill_in_repo(&clone_dir, skill_name) {
            Some(path) => path,
            None => {
                eprintln!("[hk] Skill '{}' no longer exists in repository — skipping update", skill_name);
                let store = state.store.lock();
                let now = chrono::Utc::now();
                if let Err(e) = store.update_check_state(&params.id, None, now, Some("removed_from_repo")) {
                    eprintln!("[hk] warning: {e}");
                }
                return Ok(manager::InstallResult {
                    name: skill_name.clone(),
                    was_update: false,
                    revision,
                    skipped: true,
                });
            }
        };

        // Find all installed paths (deduplicated) and copy the latest version
        // to each. Restrict to global-scope siblings so the update flow doesn't
        // overwrite user-managed project copies of the same name.
        let all_siblings: Vec<Extension> = {
            let store = state.store.lock();
            let all = store.list_extensions(Some(ext.kind), None)?;
            all.into_iter()
                .filter(|e| {
                    e.name == ext.name
                        && e.source_path.is_some()
                        && matches!(e.scope, ConfigScope::Global)
                })
                .collect()
        };

        let mut updated_dirs = std::collections::HashSet::new();
        for sibling in &all_siblings {
            let source_path = sibling.source_path.as_deref()
                .ok_or_else(|| hk_core::HkError::Internal("Sibling extension has no source_path".into()))?;
            let skill_dir = std::path::Path::new(source_path).parent().ok_or_else(|| {
                hk_core::HkError::Internal("Cannot determine skill directory from source path".into())
            })?;
            if !updated_dirs.insert(skill_dir.to_string_lossy().to_string()) { continue; }
            deployer::deploy_skill(&skill_source, skill_dir.parent().unwrap_or(skill_dir))?;
        }

        // Update install metadata for all siblings
        {
            let store = state.store.lock();
            let updated_meta = InstallMeta {
                revision: revision.clone().or(install_meta.revision.clone()),
                remote_revision: None,
                checked_at: None,
                check_error: None,
                ..install_meta
            };
            for sibling in &all_siblings {
                if let Err(e) = store.set_install_meta(&sibling.id, &updated_meta) {
                    eprintln!("[hk] warning: {e}");
                }
            }
        }

        Ok(manager::InstallResult {
            name: skill_name.clone(),
            was_update: true,
            revision,
            skipped: false,
        })
    }).await
}

// --- Multi-skill git install flow ---

#[derive(serde::Serialize)]
#[serde(tag = "type")]
pub enum ScanResult {
    Installed { result: manager::InstallResult },
    MultipleSkills { clone_id: String, skills: Vec<manager::DiscoveredSkill> },
    NoSkills,
}

#[derive(Deserialize)]
pub struct ScanGitRepoParams {
    pub url: String,
    pub target_agents: Vec<String>,
}

pub async fn scan_git_repo(
    State(state): State<WebState>,
    Json(params): Json<ScanGitRepoParams>,
) -> Result<ScanResult> {
    // Clean up stale pending clones (older than 10 minutes)
    {
        let mut clones = state.pending_clones.lock();
        clones.retain(|_, v| v.created_at.elapsed().as_secs() < 600);
    }

    blocking(move || {
        let temp = tempfile::tempdir()?;
        let clone_dir = temp.path().join("repo");
        let output = std::process::Command::new("git")
            .args(["clone", "--depth", "1", "--", &params.url, &clone_dir.to_string_lossy()])
            .output()
            .map_err(|e| hk_core::HkError::CommandFailed(format!("Failed to run git clone: {}", e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(hk_core::HkError::CommandFailed(format!("git clone failed: {}", stderr.trim())));
        }

        let skills = manager::scan_repo_skills(&clone_dir);
        match skills.len() {
            0 => Ok(ScanResult::NoSkills),
            1 => {
                let agents = if params.target_agents.is_empty() {
                    vec![state.adapters.iter().find(|a| a.detect())
                        .map(|a| a.name().to_string())
                        .ok_or_else(|| hk_core::HkError::NotFound("No detected agent found".into()))?]
                } else {
                    params.target_agents
                };
                let skill_id = if skills[0].skill_id.is_empty() { None } else { Some(skills[0].skill_id.as_str()) };
                let mut last_result = None;
                let mut installed_agents = Vec::new();
                for agent_name in &agents {
                    let a = state.adapters.iter().find(|a| a.name() == agent_name.as_str())
                        .ok_or_else(|| hk_core::HkError::NotFound(format!("Agent '{}' not found", agent_name)))?;
                    let target_dir = a.skill_dirs().into_iter().next().ok_or_else(|| {
                        hk_core::HkError::Internal(format!("No skill directory for agent '{}'", agent_name))
                    })?;
                    std::fs::create_dir_all(&target_dir)?;
                    let result = manager::install_from_git_with_id(&params.url, &target_dir, skill_id)?;
                    installed_agents.push(agent_name.clone());
                    last_result = Some(result);
                }
                if let Some(ref result) = last_result {
                    let meta = InstallMeta {
                        install_type: "git".into(),
                        url: Some(params.url.clone()),
                        url_resolved: None,
                        branch: None,
                        subpath: skill_id.map(|s| s.to_string()),
                        revision: result.revision.clone(),
                        remote_revision: None,
                        checked_at: None,
                        check_error: None,
                    };
                    let pack = meta.url.as_deref().and_then(scanner::extract_pack_from_url);
                    let store = state.store.lock();
                    service::post_install_sync(&store, &state.adapters, &installed_agents, &result.name, Some(meta), pack.as_deref())?;
                }
                Ok(ScanResult::Installed {
                    result: last_result.ok_or_else(|| hk_core::HkError::Internal("No install results produced".into()))?,
                })
            }
            _ => {
                let clone_id = uuid::Uuid::new_v4().to_string();
                let mut clones = state.pending_clones.lock();
                clones.insert(clone_id.clone(), crate::state::PendingClone {
                    _temp_dir: temp, clone_dir, url: params.url, created_at: std::time::Instant::now(),
                });
                Ok(ScanResult::MultipleSkills { clone_id, skills })
            }
        }
    }).await
}

#[derive(Deserialize)]
pub struct InstallScannedSkillsParams {
    pub clone_id: String,
    pub skill_ids: Vec<String>,
    pub target_agents: Vec<String>,
}

pub async fn install_scanned_skills(
    State(state): State<WebState>,
    Json(params): Json<InstallScannedSkillsParams>,
) -> Result<Vec<manager::InstallResult>> {
    let pending = {
        let mut clones = state.pending_clones.lock();
        clones.remove(&params.clone_id)
            .ok_or_else(|| ApiError::from(hk_core::HkError::Internal("Clone session expired. Please try again.".into())))?
    };

    blocking(move || {
        let mut results = Vec::new();
        for agent_name in &params.target_agents {
            let a = state.adapters.iter().find(|a| a.name() == agent_name.as_str())
                .ok_or_else(|| hk_core::HkError::NotFound(format!("Agent '{}' not found", agent_name)))?;
            let target_dir = a.skill_dirs().into_iter().next().ok_or_else(|| {
                hk_core::HkError::Internal(format!("No skill directory for agent '{}'", agent_name))
            })?;
            std::fs::create_dir_all(&target_dir)?;
            for sid in &params.skill_ids {
                let skill_id_opt = if sid.is_empty() { None } else { Some(sid.as_str()) };
                let result = manager::install_from_clone(&pending.clone_dir, &target_dir, skill_id_opt, &pending.url)?;
                results.push((agent_name.clone(), sid.clone(), result));
            }
        }

        {
            let store = state.store.lock();
            let install_pack = scanner::extract_pack_from_url(&pending.url);
            let mut synced_skills = std::collections::HashSet::new();
            for (_agent_name, sid, result) in &results {
                if !synced_skills.insert(result.name.clone()) {
                    let meta = InstallMeta {
                        install_type: "git".into(), url: Some(pending.url.clone()),
                        url_resolved: None, branch: None,
                        subpath: if sid.is_empty() { None } else { Some(sid.clone()) },
                        revision: result.revision.clone(), remote_revision: None,
                        checked_at: None, check_error: None,
                    };
                    let ext_id = scanner::stable_id_for(&result.name, "skill", _agent_name);
                    let _ = store.set_install_meta(&ext_id, &meta);
                    if let Some(ref p) = install_pack { let _ = store.update_pack(&ext_id, Some(p)); }
                    continue;
                }
                let meta = InstallMeta {
                    install_type: "git".into(), url: Some(pending.url.clone()),
                    url_resolved: None, branch: None,
                    subpath: if sid.is_empty() { None } else { Some(sid.clone()) },
                    revision: result.revision.clone(), remote_revision: None,
                    checked_at: None, check_error: None,
                };
                service::post_install_sync(
                    &store, &state.adapters, &params.target_agents,
                    &result.name, Some(meta), install_pack.as_deref(),
                )?;
            }
        }

        Ok(results.into_iter().map(|(_, _, r)| r).collect())
    }).await
}

#[derive(Deserialize)]
pub struct InstallNewRepoSkillsParams {
    pub url: String,
    pub skill_ids: Vec<String>,
    pub target_agents: Vec<String>,
}

pub async fn install_new_repo_skills(
    State(state): State<WebState>,
    Json(params): Json<InstallNewRepoSkillsParams>,
) -> Result<Vec<manager::InstallResult>> {
    blocking(move || {
        let temp = tempfile::tempdir()
            .map_err(|e| hk_core::HkError::Internal(format!("Failed to create temp directory: {e}")))?;
        let clone_dir = temp.path().join("repo");
        let output = std::process::Command::new("git")
            .args(["clone", "--depth", "1", "--", &params.url, &clone_dir.to_string_lossy()])
            .output()
            .map_err(|e| hk_core::HkError::CommandFailed(format!("Failed to run git clone: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(hk_core::HkError::CommandFailed(format!("git clone failed: {}", stderr.trim())));
        }

        let mut results = Vec::new();
        for agent_name in &params.target_agents {
            let a = state.adapters.iter().find(|a| a.name() == agent_name.as_str())
                .ok_or_else(|| hk_core::HkError::NotFound(format!("Agent '{}' not found", agent_name)))?;
            let target_dir = a.skill_dirs().into_iter().next().ok_or_else(|| {
                hk_core::HkError::Internal(format!("No skill directory for agent '{}'", agent_name))
            })?;
            std::fs::create_dir_all(&target_dir)?;
            for sid in &params.skill_ids {
                let skill_id_opt = if sid.is_empty() { None } else { Some(sid.as_str()) };
                let result = manager::install_from_clone(&clone_dir, &target_dir, skill_id_opt, &params.url)?;
                results.push((agent_name.clone(), sid.clone(), result));
            }
        }

        {
            let store = state.store.lock();
            let install_pack = scanner::extract_pack_from_url(&params.url);
            let mut synced_skills = std::collections::HashSet::new();
            for (_agent_name, sid, result) in &results {
                if !synced_skills.insert(result.name.clone()) {
                    let meta = InstallMeta {
                        install_type: "git".into(), url: Some(params.url.clone()),
                        url_resolved: None, branch: None,
                        subpath: if sid.is_empty() { None } else { Some(sid.clone()) },
                        revision: result.revision.clone(), remote_revision: None,
                        checked_at: None, check_error: None,
                    };
                    let ext_id = scanner::stable_id_for(&result.name, "skill", _agent_name);
                    let _ = store.set_install_meta(&ext_id, &meta);
                    if let Some(ref p) = install_pack { let _ = store.update_pack(&ext_id, Some(p)); }
                    continue;
                }
                let meta = InstallMeta {
                    install_type: "git".into(), url: Some(params.url.clone()),
                    url_resolved: None, branch: None,
                    subpath: if sid.is_empty() { None } else { Some(sid.clone()) },
                    revision: result.revision.clone(), remote_revision: None,
                    checked_at: None, check_error: None,
                };
                service::post_install_sync(
                    &store, &state.adapters, &params.target_agents,
                    &result.name, Some(meta), install_pack.as_deref(),
                )?;
            }
        }

        Ok(results.into_iter().map(|(_, _, r)| r).collect())
    }).await
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
        let cli = store.get_extension(&params.cli_id)?
            .ok_or_else(|| hk_core::HkError::NotFound("CLI extension not found".into()))?;
        let children = store.get_child_skills(&params.cli_id)?;
        Ok((cli, children))
    }).await
}

pub async fn check_updates(
    State(state): State<WebState>,
) -> Result<CheckUpdatesResult> {
    blocking(move || {
        // Read all extensions and release the lock before doing slow network calls
        let (updatable, unlinked): ExtensionUpdateBuckets = {
            let store = state.store.lock();
            let extensions = store.list_extensions(None, None)?;
            let mut has_meta = Vec::new();
            let mut no_meta = Vec::new();
            for e in extensions {
                if e.kind != ExtensionKind::Skill { continue; }
                // Project-scoped skills are owned by the project's own version
                // control (the user's git repo or hand-authored files), not by
                // HK's marketplace/update flow. Skip them so we don't auto-link
                // them to a marketplace skill that just happens to share a name.
                if !matches!(e.scope, ConfigScope::Global) { continue; }
                if let Some(meta) = e.install_meta {
                    match meta.install_type.as_str() {
                        "git" | "marketplace" => has_meta.push((e.id, e.name, meta)),
                        _ => {}
                    }
                } else {
                    no_meta.push((e.id, e.name));
                }
            }
            (has_meta, no_meta)
        };

        // Try to match unlinked skills against marketplace by name
        if !unlinked.is_empty() {
            let unique_names: std::collections::HashSet<&str> =
                unlinked.iter().map(|(_, name)| name.as_str()).collect();
            let mut matched = std::collections::HashMap::<String, (String, String, Option<String>)>::new();
            for name in &unique_names {
                if let Ok(results) = marketplace::search_skills(name, 5) {
                    let exact: Vec<_> = results.iter().filter(|r| r.name.eq_ignore_ascii_case(name)).collect();
                    if exact.len() == 1 {
                        let item = exact[0];
                        let git_url = marketplace::git_url_for_source(&item.source);
                        let remote_rev = manager::get_remote_head(&git_url).ok();
                        matched.insert(name.to_string(), (git_url, item.skill_id.clone(), remote_rev));
                    }
                }
            }
            if !matched.is_empty() {
                let store = state.store.lock();
                let now = chrono::Utc::now();
                for (id, name) in &unlinked {
                    if let Some((git_url, skill_id, remote_rev)) = matched.get(name.as_str()) {
                        let meta = InstallMeta {
                            install_type: "marketplace".into(),
                            url: Some(format!("{}/{}", git_url.trim_end_matches(".git"), skill_id)),
                            url_resolved: Some(git_url.clone()),
                            branch: None,
                            subpath: if skill_id.is_empty() { None } else { Some(skill_id.clone()) },
                            revision: remote_rev.clone(),
                            remote_revision: remote_rev.clone(),
                            checked_at: Some(now),
                            check_error: None,
                        };
                        if let Err(e) = store.set_install_meta(id, &meta) {
                            eprintln!("[hk] warning: {e}");
                        }
                    }
                }
            }
        }

        // Check each extension for updates with URL caching
        let mut remote_cache = std::collections::HashMap::new();
        let mut statuses: Vec<_> = updatable.iter()
            .map(|(id, name, meta)| {
                let status = manager::check_update_with_cache(meta, &mut remote_cache);
                (id.clone(), name.clone(), meta.clone(), status)
            })
            .collect();

        // For UpdateAvailable skills, clone repos to verify + discover new skills
        let mut new_skills: Vec<NewRepoSkill> = Vec::new();
        {
            let mut url_to_indices = std::collections::HashMap::<String, Vec<usize>>::new();
            for (idx, (_, _, meta, status)) in statuses.iter().enumerate() {
                if !matches!(status, UpdateStatus::UpdateAvailable { .. }) { continue; }
                let url = meta.url_resolved.as_deref().or(meta.url.as_deref()).unwrap_or("");
                if !url.is_empty() {
                    url_to_indices.entry(url.to_string()).or_default().push(idx);
                }
            }
            for (url, indices) in &url_to_indices {
                let temp = match tempfile::tempdir() { Ok(t) => t, Err(_) => continue };
                let clone_path = temp.path().join("repo");
                let ok = std::process::Command::new("git")
                    .args(["clone", "--depth", "1", "--", url, &clone_path.to_string_lossy()])
                    .output().map(|o| o.status.success()).unwrap_or(false);
                if !ok { continue; }

                // Verify existing skills with subpath
                for &idx in indices {
                    let (_, name, meta, _) = &statuses[idx];
                    if meta.subpath.is_some() && manager::find_skill_in_repo(&clone_path, name).is_none() {
                        eprintln!("[hk] Skill '{}' no longer exists in repository", name);
                        statuses[idx].3 = UpdateStatus::RemovedFromRepo;
                    }
                }

                // Discover new skills in this repo
                let repo_skills = manager::scan_repo_skills(&clone_path);
                if repo_skills.len() <= 1 { continue; }

                let installed_names: std::collections::HashSet<String> = {
                    let store = state.store.lock();
                    let all_exts = store.list_extensions(Some(ExtensionKind::Skill), None).unwrap_or_default();
                    all_exts.into_iter()
                        .filter(|ext| ext.install_meta.as_ref().is_some_and(|m| {
                            m.url_resolved.as_deref().or(m.url.as_deref()) == Some(url.as_str())
                        }))
                        .map(|ext| ext.name)
                        .collect()
                };
                let pack = scanner::extract_pack_from_url(url);
                for skill in &repo_skills {
                    if !installed_names.contains(skill.name.as_str()) {
                        new_skills.push(NewRepoSkill {
                            repo_url: url.clone(),
                            pack: pack.clone(),
                            skill_id: skill.skill_id.clone(),
                            name: skill.name.clone(),
                            description: skill.description.clone(),
                        });
                    }
                }
            }
        }

        // Persist check state
        let store = state.store.lock();
        let now = chrono::Utc::now();
        for (id, _name, _meta, status) in &statuses {
            let (remote_rev, check_err) = match status {
                UpdateStatus::UpToDate { remote_hash } => (Some(remote_hash.as_str()), None),
                UpdateStatus::UpdateAvailable { remote_hash } => (Some(remote_hash.as_str()), None),
                UpdateStatus::RemovedFromRepo => (None, Some("removed_from_repo")),
                UpdateStatus::Error { message } => (None, Some(message.as_str())),
            };
            if let Err(e) = store.update_check_state(id, remote_rev, now, check_err) {
                eprintln!("[hk] warning: {e}");
            }
        }

        Ok(CheckUpdatesResult {
            statuses: statuses.into_iter().map(|(id, _, _, status)| (id, status)).collect(),
            new_skills,
        })
    }).await
}

pub async fn get_cached_update_statuses(
    State(state): State<WebState>,
) -> Result<Vec<(String, UpdateStatus)>> {
    blocking(move || {
        let store = state.store.lock();
        let extensions = store.list_extensions(None, None)?;
        let mut results = Vec::new();
        for ext in extensions {
            if ext.kind != ExtensionKind::Skill { continue; }
            let Some(meta) = ext.install_meta else { continue; };
            if meta.checked_at.is_none() { continue; }
            let status = match (meta.revision.as_deref(), meta.remote_revision.as_deref()) {
                (Some(local), Some(remote)) => {
                    if local.starts_with(remote) || remote.starts_with(local) {
                        UpdateStatus::UpToDate { remote_hash: remote.to_string() }
                    } else {
                        UpdateStatus::UpdateAvailable { remote_hash: remote.to_string() }
                    }
                }
                (None, Some(remote)) => {
                    UpdateStatus::UpdateAvailable { remote_hash: remote.to_string() }
                }
                _ => {
                    if let Some(ref err) = meta.check_error {
                        if err == "removed_from_repo" {
                            UpdateStatus::RemovedFromRepo
                        } else {
                            UpdateStatus::Error { message: err.clone() }
                        }
                    } else {
                        continue;
                    }
                }
            };
            results.push((ext.id, status));
        }
        Ok(results)
    }).await
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
    }).await
}
