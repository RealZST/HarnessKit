use hk_core::{adapter, deployer, manager, marketplace, models::*, scanner};
use tauri::State;
use super::{AppState, PendingClone};
use super::helpers::{audit_extension_by_name, find_skill_by_id};

// --- Multi-skill git install flow ---

#[derive(serde::Serialize)]
#[serde(tag = "type")]
pub enum ScanResult {
    Installed { result: manager::InstallResult },
    MultipleSkills { clone_id: String, skills: Vec<manager::DiscoveredSkill> },
    NoSkills,
}

#[tauri::command]
pub fn install_from_local(state: State<AppState>, path: String, target_agents: Vec<String>) -> Result<manager::InstallResult, String> {
    let source_path = std::path::Path::new(&path);
    if !source_path.is_dir() {
        return Err("Selected path is not a directory".into());
    }
    // Must contain SKILL.md at root or be a parent of skill subdirectories
    let skill_md = source_path.join("SKILL.md");
    if !skill_md.exists() {
        return Err("Selected directory does not contain a SKILL.md file".into());
    }

    let skill_name = scanner::parse_skill_name(&skill_md)
        .unwrap_or_else(|| source_path.file_name().unwrap_or_default().to_string_lossy().to_string());

    let adapters = adapter::all_adapters();
    let agents: Vec<String> = if target_agents.is_empty() {
        adapters.iter().filter(|a| a.detect()).map(|a| a.name().to_string()).collect()
    } else {
        target_agents
    };

    for agent_name in &agents {
        let a = adapters.iter()
            .find(|a| a.name() == agent_name.as_str())
            .ok_or_else(|| format!("Agent '{}' not found", agent_name))?;
        let target_dir = a.skill_dirs().into_iter().next()
            .ok_or_else(|| format!("No skill directory for agent '{}'", agent_name))?;
        std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
        deployer::deploy_skill(source_path, &target_dir).map_err(|e| e.to_string())?;
    }

    let result = manager::InstallResult {
        name: skill_name.clone(),
        was_update: false,
        revision: None,
    };

    // Re-scan affected agents only and persist
    let mut extensions = Vec::new();
    {
        let store = state.store.lock();
        for a in &adapters {
            if agents.contains(&a.name().to_string()) {
                let exts = scanner::scan_adapter(a.as_ref());
                store.sync_extensions_for_agent(a.name(), &exts).map_err(|e| e.to_string())?;
                extensions.extend(exts);
            }
        }
        // Save install metadata for each agent
        // If the local folder is inside a git repo, record the git URL
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
        let pack = git_source.url.as_deref()
            .and_then(hk_core::scanner::extract_pack_from_url);
        for agent_name in &agents {
            let ext_id = scanner::stable_id_for(&skill_name, "skill", agent_name);
            let _ = store.set_install_meta(&ext_id, &meta);
            if let Some(ref p) = pack {
                let _ = store.update_pack(&ext_id, Some(p));
            }
        }
    }

    // Audit
    let audit_results = audit_extension_by_name(&result.name, &extensions, &adapters);
    if !audit_results.is_empty() {
        let store = state.store.lock();
        for r in &audit_results {
            let _ = store.insert_audit_result(r);
        }
    }

    Ok(result)
}

#[tauri::command]
pub fn install_from_git(state: State<AppState>, url: String, target_agent: Option<String>, skill_id: Option<String>) -> Result<manager::InstallResult, String> {
    hk_core::sanitize::validate_git_url(&url)
        .map_err(|e| e.to_string())?;
    let adapters = adapter::all_adapters();
    let (target_dir, agent_name) = if let Some(ref agent) = target_agent {
        let a = adapters.iter()
            .find(|a| a.name() == agent.as_str())
            .ok_or_else(|| format!("Agent '{}' not found", agent))?;
        let dir = a.skill_dirs().into_iter().next()
            .ok_or_else(|| format!("No skill directory for agent '{}'", agent))?;
        (dir, agent.clone())
    } else {
        // Fallback: first detected agent
        let a = adapters.iter().find(|a| a.detect())
            .ok_or_else(|| "No detected agent found".to_string())?;
        let name = a.name().to_string();
        let dir = a.skill_dirs().into_iter().next()
            .ok_or_else(|| "No agent skill directory found".to_string())?;
        (dir, name)
    };

    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    let sid = skill_id.as_deref().filter(|s| !s.is_empty());
    let result = manager::install_from_git_with_id(&url, &target_dir, sid).map_err(|e| e.to_string())?;

    // Re-scan affected agent only and persist
    let extensions: Vec<Extension> = if let Some(a) = adapters.iter().find(|a| a.name() == agent_name) {
        scanner::scan_adapter(a.as_ref())
    } else {
        Vec::new()
    };
    {
        let store = state.store.lock();
        store.sync_extensions_for_agent(&agent_name, &extensions).map_err(|e| e.to_string())?;
        // Persist install source metadata
        let ext_id = scanner::stable_id_for(&result.name, "skill", &agent_name);
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
        let _ = store.set_install_meta(&ext_id, &meta);
        // Set pack from install URL so Source filter works immediately
        let pack = meta.url.as_deref()
            .and_then(hk_core::scanner::extract_pack_from_url);
        if let Some(ref p) = pack {
            let _ = store.update_pack(&ext_id, Some(p));
        }
    } // Lock released before slow file I/O

    // Audit the newly installed extension (no lock held)
    let audit_results = audit_extension_by_name(&result.name, &extensions, &adapters);
    if !audit_results.is_empty() {
        let store = state.store.lock();
        for r in &audit_results {
            let _ = store.insert_audit_result(r);
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn scan_git_repo(state: State<'_, AppState>, url: String, target_agents: Vec<String>) -> Result<ScanResult, String> {
    // Clean up stale pending clones (older than 10 minutes)
    {
        let mut clones = state.pending_clones.lock();
        clones.retain(|_, v| v.created_at.elapsed().as_secs() < 600);
    }

    let store_clone = state.store.clone();
    let pending_clones = state.pending_clones.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<ScanResult, String> {
        let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let clone_dir = temp.path().join("repo");

        let output = std::process::Command::new("git")
            .args(["clone", "--depth", "1", "--", &url, &clone_dir.to_string_lossy()])
            .output()
            .map_err(|e| format!("Failed to run git clone: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git clone failed: {}", stderr.trim()));
        }

        let skills = manager::scan_repo_skills(&clone_dir);

        match skills.len() {
            0 => Ok(ScanResult::NoSkills),
            1 => {
                // Auto-install single skill
                let adapters = adapter::all_adapters();
                let agents = if target_agents.is_empty() {
                    vec![adapters.iter().find(|a| a.detect())
                        .map(|a| a.name().to_string())
                        .ok_or("No detected agent found")?]
                } else {
                    target_agents
                };

                let skill_id = if skills[0].skill_id.is_empty() { None } else { Some(skills[0].skill_id.as_str()) };
                let mut last_result = None;
                let mut installed_agents = Vec::new();
                for agent_name in &agents {
                    let a = adapters.iter()
                        .find(|a| a.name() == agent_name.as_str())
                        .ok_or_else(|| format!("Agent '{}' not found", agent_name))?;
                    let target_dir = a.skill_dirs().into_iter().next()
                        .ok_or_else(|| format!("No skill directory for agent '{}'", agent_name))?;
                    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
                    let result = manager::install_from_git_with_id(&url, &target_dir, skill_id)
                        .map_err(|e| e.to_string())?;
                    installed_agents.push(agent_name.clone());
                    last_result = Some(result);
                }

                // Re-scan affected agents only and persist
                {
                    let store = store_clone.lock();
                    for a in &adapters {
                        if installed_agents.contains(&a.name().to_string()) {
                            let exts = scanner::scan_adapter(a.as_ref());
                            store.sync_extensions_for_agent(a.name(), &exts).map_err(|e| e.to_string())?;
                        }
                    }
                    // Persist install source metadata for each agent
                    if let Some(ref result) = last_result {
                        let meta = InstallMeta {
                            install_type: "git".into(),
                            url: Some(url.clone()),
                            url_resolved: None,
                            branch: None,
                            subpath: skill_id.map(|s| s.to_string()),
                            revision: result.revision.clone(),
                            remote_revision: None,
                            checked_at: None,
                            check_error: None,
                        };
                        let pack = meta.url.as_deref()
                            .and_then(hk_core::scanner::extract_pack_from_url);
                        for agent_name in &installed_agents {
                            let ext_id = scanner::stable_id_for(&result.name, "skill", agent_name);
                            let _ = store.set_install_meta(&ext_id, &meta);
                            if let Some(ref p) = pack {
                                let _ = store.update_pack(&ext_id, Some(p));
                            }
                        }
                    }
                }

                Ok(ScanResult::Installed { result: last_result.ok_or("No install results produced")? })
            }
            _ => {
                // Multiple skills -- cache the clone and return the list
                let clone_id = uuid::Uuid::new_v4().to_string();
                let mut clones = pending_clones.lock();
                clones.insert(clone_id.clone(), PendingClone {
                    _temp_dir: temp,
                    clone_dir,
                    url,
                    created_at: std::time::Instant::now(),
                });
                Ok(ScanResult::MultipleSkills { clone_id, skills })
            }
        }
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_scanned_skills(
    state: State<'_, AppState>,
    clone_id: String,
    skill_ids: Vec<String>,
    target_agents: Vec<String>,
) -> Result<Vec<manager::InstallResult>, String> {
    let pending = {
        let mut clones = state.pending_clones.lock();
        clones.remove(&clone_id)
            .ok_or_else(|| "Clone session expired. Please try again.".to_string())?
    };

    let store_clone = state.store.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<manager::InstallResult>, String> {
        let adapters = adapter::all_adapters();
        let mut results = Vec::new();

        for agent_name in &target_agents {
            let a = adapters.iter()
                .find(|a| a.name() == agent_name.as_str())
                .ok_or_else(|| format!("Agent '{}' not found", agent_name))?;
            let target_dir = a.skill_dirs().into_iter().next()
                .ok_or_else(|| format!("No skill directory for agent '{}'", agent_name))?;
            std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

            for sid in &skill_ids {
                let skill_id_opt = if sid.is_empty() { None } else { Some(sid.as_str()) };
                let result = manager::install_from_clone(&pending.clone_dir, &target_dir, skill_id_opt, &pending.url)
                    .map_err(|e| e.to_string())?;
                results.push((agent_name.clone(), sid.clone(), result));
            }
        }

        // Re-scan affected agents only and persist
        {
            let store = store_clone.lock();
            for a in &adapters {
                if target_agents.contains(&a.name().to_string()) {
                    let exts = scanner::scan_adapter(a.as_ref());
                    store.sync_extensions_for_agent(a.name(), &exts).map_err(|e| e.to_string())?;
                }
            }
            // Persist install source metadata for each installed skill+agent
            let install_pack = hk_core::scanner::extract_pack_from_url(&pending.url);
            for (agent_name, sid, result) in &results {
                let ext_id = scanner::stable_id_for(&result.name, "skill", agent_name);
                let meta = InstallMeta {
                    install_type: "git".into(),
                    url: Some(pending.url.clone()),
                    url_resolved: None,
                    branch: None,
                    subpath: if sid.is_empty() { None } else { Some(sid.clone()) },
                    revision: result.revision.clone(),
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                };
                let _ = store.set_install_meta(&ext_id, &meta);
                if let Some(ref p) = install_pack {
                    let _ = store.update_pack(&ext_id, Some(p));
                }
            }
        }

        // pending._temp_dir is dropped here, cleaning up the clone
        let results = results.into_iter().map(|(_, _, r)| r).collect();
        Ok(results)
    }).await.map_err(|e| e.to_string())?
}

// --- Cross-agent deploy command ---

#[tauri::command]
pub fn deploy_to_agent(state: State<AppState>, extension_id: String, target_agent: String) -> Result<String, String> {
    let ext = {
        let store = state.store.lock();
        store.get_extension(&extension_id).map_err(|e| e.to_string())?
            .ok_or_else(|| "Extension not found".to_string())?
    };

    let adapters = adapter::all_adapters();
    let target_adapter = adapters.iter()
        .find(|a| a.name() == target_agent)
        .ok_or_else(|| format!("Agent '{}' not found", target_agent))?;

    match ext.kind.as_str() {
        "skill" => {
            // Find source skill path
            let source_path = find_skill_by_id(&adapters, &extension_id, &ext.agents)
                .map(|loc| loc.entry_path)
                .ok_or_else(|| "Could not find source skill files".to_string())?;
            let target_dir = target_adapter.skill_dirs().into_iter().next()
                .ok_or_else(|| format!("No skill directory for agent '{}'", target_agent))?;
            let deployed_name = deployer::deploy_skill(&source_path, &target_dir).map_err(|e| e.to_string())?;

            // Re-scan target agent to pick up the deployed extension
            let store = state.store.lock();
            let exts = scanner::scan_adapter(target_adapter.as_ref());
            store.sync_extensions_for_agent(target_adapter.name(), &exts).map_err(|e| e.to_string())?;
            Ok(deployed_name)
        }
        "mcp" => {
            // Find the source MCP server entry
            let mut source_entry = None;
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for server in adapter.read_mcp_servers() {
                    if scanner::stable_id_for(&server.name, "mcp", adapter.name()) == extension_id {
                        source_entry = Some(server);
                        break;
                    }
                }
                if source_entry.is_some() { break; }
            }
            let entry = source_entry.ok_or_else(|| "Could not find source MCP server config".to_string())?;
            let config_path = target_adapter.mcp_config_path();
            deployer::deploy_mcp_server(&config_path, &entry, target_adapter.mcp_format()).map_err(|e| e.to_string())?;

            // Re-scan target agent
            let store = state.store.lock();
            let exts = scanner::scan_adapter(target_adapter.as_ref());
            store.sync_extensions_for_agent(target_adapter.name(), &exts).map_err(|e| e.to_string())?;
            Ok(entry.name)
        }
        "hook" => {
            // Find the source hook entry
            let mut source_entry = None;
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for hook in adapter.read_hooks() {
                    let hook_name = format!("{}:{}:{}", hook.event, hook.matcher.as_deref().unwrap_or("*"), hook.command);
                    if scanner::stable_id_for(&hook_name, "hook", adapter.name()) == extension_id {
                        source_entry = Some(hook);
                        break;
                    }
                }
                if source_entry.is_some() { break; }
            }
            let mut entry = source_entry.ok_or_else(|| "Could not find source hook config".to_string())?;

            // Translate event name to target agent's convention
            let translated_event = target_adapter.translate_hook_event(&entry.event)
                .ok_or_else(|| format!("Hook event '{}' is not supported by {}", entry.event, target_agent))?;
            entry.event = translated_event;

            let config_path = target_adapter.hook_config_path();
            deployer::deploy_hook(&config_path, &entry, target_adapter.hook_format()).map_err(|e| e.to_string())?;

            // Codex requires hooks feature enabled in config.toml
            if target_adapter.name() == "codex" {
                let _ = deployer::ensure_codex_hooks_enabled(&target_adapter.base_dir());
            }

            // Re-scan target agent
            let store = state.store.lock();
            let exts = scanner::scan_adapter(target_adapter.as_ref());
            store.sync_extensions_for_agent(target_adapter.name(), &exts).map_err(|e| e.to_string())?;
            Ok(format!("{}:{}", entry.event, entry.command))
        }
        "cli" => {
            // Deploy the CLI's associated skill to the target agent
            let binary_name = ext.cli_meta.as_ref()
                .map(|m| m.binary_name.clone())
                .unwrap_or_else(|| ext.name.to_lowercase());
            let locations = scanner::skill_locations(&binary_name, &adapters);
            let source_path = locations.into_iter()
                .next()
                .map(|(_, path)| path)
                .ok_or_else(|| "Could not find source skill files for CLI".to_string())?;
            let target_dir = target_adapter.skill_dirs().into_iter().next()
                .ok_or_else(|| format!("No skill directory for agent '{}'", target_agent))?;
            let deployed_name = deployer::deploy_skill(&source_path, &target_dir).map_err(|e| e.to_string())?;

            // Re-scan to pick up changes
            let store = state.store.lock();
            let exts = scanner::scan_adapter(target_adapter.as_ref());
            store.sync_extensions_for_agent(target_adapter.name(), &exts).map_err(|e| e.to_string())?;
            Ok(deployed_name)
        }
        other => Err(format!("Cross-agent deploy not supported for '{}' extensions", other)),
    }
}

// --- CLI commands ---

#[tauri::command]
pub fn get_cli_with_children(
    state: State<AppState>,
    cli_id: String,
) -> Result<(Extension, Vec<Extension>), String> {
    let store = state.store.lock();
    let cli = store.get_extension(&cli_id).map_err(|e| e.to_string())?
        .ok_or_else(|| format!("CLI not found: {}", cli_id))?;
    let children = store.get_child_skills(&cli_id).map_err(|e| e.to_string())?;
    Ok((cli, children))
}

#[tauri::command]
pub fn list_cli_marketplace() -> Result<Vec<marketplace::MarketplaceItem>, String> {
    Ok(marketplace::list_cli_registry())
}

#[tauri::command]
pub fn install_cli(
    state: State<AppState>,
    binary_name: String,
    _target_agents: Vec<String>,
) -> Result<(), String> {
    // Look up from EMBEDDED registry only — never execute remote commands
    let entry = marketplace::get_embedded_cli_entry(&binary_name)
        .ok_or_else(|| format!("CLI '{}' not found in approved registry", binary_name))?;

    // Step 1: Execute the install command from embedded registry.
    // Prefer structured fields (Command::new + args) to avoid sh -c shell injection.
    let output = if let Some((program, args)) = entry.resolved_command() {
        std::process::Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("Failed to run install command: {}", e))?
    } else {
        // Fallback for piped commands (e.g. curl | sh) that cannot be structured
        std::process::Command::new("sh")
            .arg("-c")
            .arg(&entry.install_command)
            .output()
            .map_err(|e| format!("Failed to run install command: {}", e))?
    };
    if !output.status.success() {
        return Err(format!("CLI install failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    // Step 2: Install skills
    if let Some(skills_cmd) = &entry.skills_install_command {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(skills_cmd)
            .output()
            .map_err(|e| format!("Failed to install skills: {}", e))?;
        if !output.status.success() {
            eprintln!("Warning: skills install had issues: {}", String::from_utf8_lossy(&output.stderr));
        }
    } else {
        let output = std::process::Command::new("npx")
            .args(["-y", "skills", "add", &entry.skills_repo, "-y", "-g"])
            .output()
            .map_err(|e| format!("Failed to install skills: {}", e))?;
        if !output.status.success() {
            eprintln!("Warning: skills install had issues: {}", String::from_utf8_lossy(&output.stderr));
        }
    }

    // Step 3: Trigger re-scan
    let store = state.store.lock();
    let adapters = adapter::all_adapters();
    let exts = scanner::scan_all(&adapters);
    store.sync_extensions(&exts).map_err(|e| e.to_string())?;
    Ok(())
}
