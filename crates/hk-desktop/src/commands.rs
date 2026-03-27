use hk_core::{adapter, auditor::Auditor, deployer, manager, models::*, scanner, store::Store};
use chrono::Utc;
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    pub store: Mutex<Store>,
}

#[tauri::command]
pub fn list_extensions(
    state: State<AppState>,
    kind: Option<String>,
    agent: Option<String>,
) -> Result<Vec<Extension>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let kind_filter = kind.as_deref().and_then(|k| k.parse().ok());
    store.list_extensions(kind_filter, agent.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_agents() -> Vec<AgentInfo> {
    let adapters = adapter::all_adapters();
    adapters.iter().map(|a| AgentInfo {
        name: a.name().to_string(),
        detected: a.detect(),
        extension_count: 0,
    }).collect()
}

#[tauri::command]
pub fn get_dashboard_stats(state: State<AppState>) -> Result<DashboardStats, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let all = store.list_extensions(None, None).map_err(|e| e.to_string())?;

    // Count issues from latest audit results
    let mut critical_issues = 0usize;
    let mut high_issues = 0usize;
    let mut medium_issues = 0usize;
    let mut low_issues = 0usize;
    for ext in &all {
        if let Ok(audits) = store.get_audit_results(&ext.id) {
            if let Some(latest) = audits.first() {
                for finding in &latest.findings {
                    match finding.severity {
                        Severity::Critical => critical_issues += 1,
                        Severity::High => high_issues += 1,
                        Severity::Medium => medium_issues += 1,
                        Severity::Low => low_issues += 1,
                    }
                }
            }
        }
    }

    Ok(DashboardStats {
        total_extensions: all.len(),
        skill_count: all.iter().filter(|e| e.kind == ExtensionKind::Skill).count(),
        mcp_count: all.iter().filter(|e| e.kind == ExtensionKind::Mcp).count(),
        plugin_count: all.iter().filter(|e| e.kind == ExtensionKind::Plugin).count(),
        hook_count: all.iter().filter(|e| e.kind == ExtensionKind::Hook).count(),
        critical_issues,
        high_issues,
        medium_issues,
        low_issues,
        updates_available: 0, // Populated by explicit check_updates call
    })
}

#[tauri::command]
pub fn toggle_extension(state: State<AppState>, id: String, enabled: bool) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.set_enabled(&id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn run_audit(state: State<AppState>) -> Result<Vec<AuditResult>, String> {
    // Read extensions and release the lock before doing slow file I/O
    let extensions = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.list_extensions(None, None).map_err(|e| e.to_string())?
    };

    let adapters = adapter::all_adapters();
    let auditor = Auditor::new();
    let mut results = Vec::new();

    for ext in &extensions {
        let (content, mcp_command, mcp_args, mcp_env, file_path) = match ext.kind {
            ExtensionKind::Skill => {
                let mut skill_content = String::new();
                let mut skill_path = ext.name.clone();
                'outer: for a in &adapters {
                    if !ext.agents.contains(&a.name().to_string()) { continue; }
                    for skill_dir in a.skill_dirs() {
                        let Ok(entries) = std::fs::read_dir(&skill_dir) else { continue };
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let skill_file = if path.is_dir() {
                                path.join("SKILL.md")
                            } else if path.extension().is_some_and(|e| e == "md") {
                                path.clone()
                            } else { continue };
                            if !skill_file.exists() { continue; }
                            let name = scanner::parse_skill_name(&skill_file).unwrap_or_else(||
                                path.file_stem().unwrap_or_default().to_string_lossy().to_string()
                            );
                            if stable_id(&name, "skill", a.name()) == ext.id {
                                skill_content = std::fs::read_to_string(&skill_file).unwrap_or_default();
                                skill_path = skill_file.to_string_lossy().to_string();
                                break 'outer;
                            }
                        }
                    }
                }
                (skill_content, None, vec![], Default::default(), skill_path)
            }
            ExtensionKind::Mcp => {
                let mut cmd = None;
                let mut args = vec![];
                let mut env = std::collections::HashMap::new();
                for a in &adapters {
                    if !ext.agents.contains(&a.name().to_string()) { continue; }
                    for server in a.read_mcp_servers() {
                        if stable_id(&server.name, "mcp", a.name()) == ext.id {
                            cmd = Some(server.command);
                            args = server.args;
                            env = server.env;
                            break;
                        }
                    }
                }
                (String::new(), cmd, args, env, ext.name.clone())
            }
            ExtensionKind::Hook => {
                (ext.description.clone(), None, vec![], Default::default(), ext.name.clone())
            }
            ExtensionKind::Plugin => {
                (String::new(), None, vec![], Default::default(), ext.name.clone())
            }
        };

        let input = hk_core::auditor::AuditInput {
            extension_id: ext.id.clone(),
            kind: ext.kind,
            name: ext.name.clone(),
            content,
            source: ext.source.clone(),
            file_path,
            mcp_command,
            mcp_args,
            mcp_env,
            installed_at: ext.installed_at,
            updated_at: ext.updated_at,
        };
        let result = auditor.audit(&input);
        results.push(result);
    }

    // Re-acquire lock briefly to store results
    let store = state.store.lock().map_err(|e| e.to_string())?;
    for result in &results {
        let _ = store.insert_audit_result(result);
    }
    Ok(results)
}

#[tauri::command]
pub fn delete_extension(state: State<AppState>, id: String) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.delete_extension(&id).map_err(|e| e.to_string())
}

fn fnv1a(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn stable_id(name: &str, kind: &str, agent: &str) -> String {
    let key = format!("{}:{}:{}", kind, agent, name);
    format!("{:016x}", fnv1a(key.as_bytes()))
}

#[derive(serde::Serialize)]
pub struct ExtensionContent {
    pub content: String,
    pub path: Option<String>,
}

#[tauri::command]
pub fn get_extension_content(state: State<AppState>, id: String) -> Result<ExtensionContent, String> {
    // Read extension metadata and release lock before file I/O
    let ext = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.get_extension(&id).map_err(|e| e.to_string())?
            .ok_or_else(|| "Extension not found".to_string())?
    };

    match ext.kind {
        ExtensionKind::Skill => {
            let adapters = adapter::all_adapters();
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for skill_dir in adapter.skill_dirs() {
                    let Ok(entries) = std::fs::read_dir(&skill_dir) else { continue };
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let skill_file = if path.is_dir() {
                            path.join("SKILL.md")
                        } else if path.extension().is_some_and(|e| e == "md") {
                            path.clone()
                        } else { continue };
                        if !skill_file.exists() { continue; }
                        let name = scanner::parse_skill_name(&skill_file).unwrap_or_else(||
                            path.file_stem().unwrap_or_default().to_string_lossy().to_string()
                        );
                        if stable_id(&name, "skill", adapter.name()) == id {
                            let dir = if path.is_dir() {
                                path.to_string_lossy().to_string()
                            } else {
                                skill_file.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
                            };
                            let content = std::fs::read_to_string(&skill_file).map_err(|e| e.to_string())?;
                            return Ok(ExtensionContent { content, path: Some(dir) });
                        }
                    }
                }
            }
            Err("Skill file not found".into())
        }
        ExtensionKind::Mcp => {
            // Pull actual config from the adapter for rich detail
            let adapters = adapter::all_adapters();
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for server in adapter.read_mcp_servers() {
                    if stable_id(&server.name, "mcp", adapter.name()) == id {
                        let config_path = adapter.mcp_config_path();
                        let mut lines = vec![
                            format!("Command: {}", server.command),
                        ];
                        if !server.args.is_empty() {
                            lines.push(format!("Args: {}", server.args.join(" ")));
                        }
                        if !server.env.is_empty() {
                            lines.push("Environment:".into());
                            for (k, _v) in &server.env {
                                lines.push(format!("  {} = ****", k));
                            }
                        }
                        return Ok(ExtensionContent {
                            content: lines.join("\n"),
                            path: Some(config_path.to_string_lossy().to_string()),
                        });
                    }
                }
            }
            Ok(ExtensionContent {
                content: ext.description,
                path: None,
            })
        }
        ExtensionKind::Hook => {
            let adapters = adapter::all_adapters();
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for hook in adapter.read_hooks() {
                    let hook_name = format!("{}:{}", hook.event, hook.matcher.as_deref().unwrap_or("*"));
                    if stable_id(&hook_name, "hook", adapter.name()) == id {
                        let config_path = adapter.hook_config_path();
                        let mut lines = vec![
                            format!("Event: {}", hook.event),
                        ];
                        if let Some(m) = &hook.matcher {
                            lines.push(format!("Matcher: {}", m));
                        }
                        lines.push(format!("Command: {}", hook.command));
                        return Ok(ExtensionContent {
                            content: lines.join("\n"),
                            path: Some(config_path.to_string_lossy().to_string()),
                        });
                    }
                }
            }
            Ok(ExtensionContent {
                content: ext.description,
                path: None,
            })
        }
        ExtensionKind::Plugin => {
            let adapters = adapter::all_adapters();
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for plugin in adapter.read_plugins() {
                    if stable_id(&plugin.name, "plugin", adapter.name()) == id {
                        let path_str = plugin.path.as_ref()
                            .map(|p| p.to_string_lossy().to_string());
                        return Ok(ExtensionContent {
                            content: ext.description,
                            path: path_str,
                        });
                    }
                }
            }
            Ok(ExtensionContent {
                content: ext.description,
                path: None,
            })
        }
    }
}

#[tauri::command]
pub fn scan_and_sync(state: State<AppState>) -> Result<usize, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let adapters = adapter::all_adapters();
    let extensions = scanner::scan_all(&adapters);
    let scanned_ids: std::collections::HashSet<String> = extensions.iter().map(|e| e.id.clone()).collect();
    let count = extensions.len();
    // Upsert scanned extensions
    for ext in &extensions {
        let _ = store.insert_extension(ext);
    }
    // Remove stale extensions that no longer exist on disk
    if let Ok(existing) = store.list_extensions(None, None) {
        for ext in &existing {
            if !scanned_ids.contains(&ext.id) {
                let _ = store.delete_extension(&ext.id);
            }
        }
    }
    Ok(count)
}

#[tauri::command]
pub fn check_updates(state: State<AppState>) -> Result<Vec<(String, UpdateStatus)>, String> {
    // Read extensions and release the lock before doing slow git ls-remote calls
    let git_extensions: Vec<_> = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let extensions = store.list_extensions(None, None).map_err(|e| e.to_string())?;
        extensions.into_iter()
            .filter(|e| e.source.origin == SourceOrigin::Git)
            .collect()
    };
    let results: Vec<(String, UpdateStatus)> = git_extensions
        .iter()
        .map(|e| {
            let status = manager::check_update(&e.source);
            (e.id.clone(), status)
        })
        .collect();
    Ok(results)
}

#[tauri::command]
pub fn install_from_git(state: State<AppState>, url: String) -> Result<String, String> {
    // Find the first detected agent's skill directory as the install target
    let adapters = adapter::all_adapters();
    let target_dir = adapters
        .iter()
        .filter(|a| a.detect())
        .flat_map(|a| a.skill_dirs())
        .next()
        .ok_or_else(|| "No agent skill directory found".to_string())?;

    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    let name = manager::install_from_git(&url, &target_dir).map_err(|e| e.to_string())?;

    // Re-scan to pick up the new extension
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let extensions = scanner::scan_all(&adapters);
    for ext in &extensions {
        let _ = store.insert_extension(ext);
    }

    Ok(name)
}

// --- Tags & Category commands ---

#[tauri::command]
pub fn update_tags(state: State<AppState>, id: String, tags: Vec<String>) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.update_tags(&id, &tags).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_tags(state: State<AppState>) -> Result<Vec<String>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.get_all_tags().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_category(state: State<AppState>, id: String, category: Option<String>) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.update_category(&id, category.as_deref()).map_err(|e| e.to_string())
}

// --- Marketplace commands ---

#[tauri::command]
pub fn search_marketplace(query: String, kind: String, limit: Option<usize>) -> Result<Vec<hk_core::marketplace::MarketplaceItem>, String> {
    let lim = limit.unwrap_or(20);
    match kind.as_str() {
        "mcp" => hk_core::marketplace::search_servers(&query, lim).map_err(|e| e.to_string()),
        _ => hk_core::marketplace::search_skills(&query, lim).map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub fn trending_marketplace(kind: String, limit: Option<usize>) -> Result<Vec<hk_core::marketplace::MarketplaceItem>, String> {
    let lim = limit.unwrap_or(10);
    match kind.as_str() {
        "mcp" => hk_core::marketplace::trending_servers(lim).map_err(|e| e.to_string()),
        _ => hk_core::marketplace::trending_skills(lim).map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub fn fetch_skill_preview(source: String, skill_id: String) -> Result<String, String> {
    hk_core::marketplace::fetch_skill_content(&source, &skill_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fetch_skill_audit(source: String, skill_id: String) -> Result<Option<hk_core::marketplace::SkillAuditInfo>, String> {
    hk_core::marketplace::fetch_audit_info(&source, &skill_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_from_marketplace(state: State<AppState>, source: String, _skill_id: String) -> Result<String, String> {
    let adapters = adapter::all_adapters();
    let target_dir = adapters.iter()
        .filter(|a| a.detect())
        .flat_map(|a| a.skill_dirs())
        .next()
        .ok_or_else(|| "No agent skill directory found".to_string())?;
    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    let git_url = hk_core::marketplace::git_url_for_source(&source);
    let name = manager::install_from_git(&git_url, &target_dir).map_err(|e| e.to_string())?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let extensions = scanner::scan_all(&adapters);
    for ext in &extensions { let _ = store.insert_extension(ext); }
    Ok(name)
}

// --- Cross-agent deploy command ---

#[tauri::command]
pub fn deploy_to_agent(state: State<AppState>, extension_id: String, target_agent: String) -> Result<String, String> {
    let ext = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
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
            let mut source_path = None;
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for skill_dir in adapter.skill_dirs() {
                    let Ok(entries) = std::fs::read_dir(&skill_dir) else { continue };
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let skill_file = if path.is_dir() {
                            path.join("SKILL.md")
                        } else if path.extension().is_some_and(|e| e == "md") {
                            path.clone()
                        } else { continue };
                        if !skill_file.exists() { continue; }
                        let name = scanner::parse_skill_name(&skill_file).unwrap_or_else(||
                            path.file_stem().unwrap_or_default().to_string_lossy().to_string()
                        );
                        if stable_id(&name, "skill", adapter.name()) == extension_id {
                            source_path = Some(path);
                            break;
                        }
                    }
                    if source_path.is_some() { break; }
                }
                if source_path.is_some() { break; }
            }
            let source_path = source_path.ok_or_else(|| "Could not find source skill files".to_string())?;
            let target_dir = target_adapter.skill_dirs().into_iter().next()
                .ok_or_else(|| format!("No skill directory for agent '{}'", target_agent))?;
            let deployed_name = deployer::deploy_skill(&source_path, &target_dir).map_err(|e| e.to_string())?;

            // Re-scan to pick up the deployed extension
            let store = state.store.lock().map_err(|e| e.to_string())?;
            let extensions = scanner::scan_all(&adapters);
            for e in &extensions { let _ = store.insert_extension(e); }
            Ok(deployed_name)
        }
        "mcp" => {
            // Find the source MCP server entry
            let mut source_entry = None;
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for server in adapter.read_mcp_servers() {
                    if stable_id(&server.name, "mcp", adapter.name()) == extension_id {
                        source_entry = Some(server);
                        break;
                    }
                }
                if source_entry.is_some() { break; }
            }
            let entry = source_entry.ok_or_else(|| "Could not find source MCP server config".to_string())?;
            let config_path = target_adapter.mcp_config_path();
            deployer::deploy_mcp_server(&config_path, &entry).map_err(|e| e.to_string())?;

            // Re-scan
            let store = state.store.lock().map_err(|e| e.to_string())?;
            let extensions = scanner::scan_all(&adapters);
            for e in &extensions { let _ = store.insert_extension(e); }
            Ok(entry.name)
        }
        "hook" => {
            // Find the source hook entry
            let mut source_entry = None;
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for hook in adapter.read_hooks() {
                    let hook_name = format!("{}:{}", hook.event, hook.command);
                    if stable_id(&hook_name, "hook", adapter.name()) == extension_id {
                        source_entry = Some(hook);
                        break;
                    }
                }
                if source_entry.is_some() { break; }
            }
            let entry = source_entry.ok_or_else(|| "Could not find source hook config".to_string())?;
            let config_path = target_adapter.hook_config_path();
            deployer::deploy_hook(&config_path, &entry).map_err(|e| e.to_string())?;

            // Re-scan
            let store = state.store.lock().map_err(|e| e.to_string())?;
            let extensions = scanner::scan_all(&adapters);
            for e in &extensions { let _ = store.insert_extension(e); }
            Ok(format!("{}:{}", entry.event, entry.command))
        }
        other => Err(format!("Cross-agent deploy not supported for '{}' extensions", other)),
    }
}

// --- Project commands ---

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> Result<Vec<Project>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.list_projects().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_project(state: State<AppState>, path: String) -> Result<Project, String> {
    // Canonicalize to prevent duplicates via symlinks/relative paths
    let project_path = std::path::Path::new(&path)
        .canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;
    let path = project_path.to_string_lossy().to_string();

    // Validate the path contains project markers
    let has_claude = project_path.join(".claude").exists();
    let has_mcp = project_path.join(".mcp.json").exists();
    if !has_claude && !has_mcp {
        return Err("Directory does not contain .claude/ or .mcp.json".to_string());
    }

    // Check for duplicate before insert
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let existing = store.list_projects().map_err(|e| e.to_string())?;
    if existing.iter().any(|p| p.path == path) {
        return Err("Project already added".to_string());
    }

    // Generate stable ID from path hash
    let id = format!("proj-{:016x}", fnv1a(path.as_bytes()));

    let name = project_path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let project = Project {
        id: id.clone(),
        name,
        path,
        created_at: Utc::now(),
    };

    store.insert_project(&project).map_err(|e| e.to_string())?;
    Ok(project)
}

#[tauri::command]
pub fn remove_project(state: State<AppState>, id: String) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.delete_project(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn discover_projects(root_path: String) -> Result<Vec<DiscoveredProject>, String> {
    let root = std::path::Path::new(&root_path);
    if !root.is_dir() {
        return Err(format!("Not a directory: {}", root_path));
    }
    Ok(scanner::discover_projects(root, 4))
}

#[tauri::command]
pub fn get_project_extensions(project_path: String) -> Result<Vec<Extension>, String> {
    let path = std::path::Path::new(&project_path);
    if !path.is_dir() {
        return Err(format!("Not a directory: {}", project_path));
    }
    Ok(scanner::scan_project(path))
}
