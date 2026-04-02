use hk_core::{adapter, auditor::{self, Auditor}, deployer, manager, marketplace, models::*, scanner, store::Store};
use hk_core::marketplace::MarketplaceItem;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::State;

pub struct PendingClone {
    pub _temp_dir: tempfile::TempDir,
    pub clone_dir: std::path::PathBuf,
    pub url: String,
    pub created_at: std::time::Instant,
}

pub struct AppState {
    pub store: Arc<Mutex<Store>>,
    pub pending_clones: Arc<Mutex<HashMap<String, PendingClone>>>,
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
pub fn list_agents(state: State<AppState>) -> Result<Vec<AgentInfo>, String> {
    let adapters = adapter::all_adapters();
    let store = state.store.lock().map_err(|e| e.to_string())?;

    // Build agent order map from DB (or fall back to adapter iteration order)
    let db_order = store.get_agent_order().unwrap_or_default();
    let order_map: std::collections::HashMap<String, i32> = db_order.into_iter().collect();

    let mut result = Vec::new();
    for a in &adapters {
        let (custom_path, enabled) = store.get_agent_setting(a.name()).unwrap_or((None, true));
        let path = custom_path.unwrap_or_else(|| a.base_dir().to_string_lossy().to_string());
        result.push(AgentInfo {
            name: a.name().to_string(),
            detected: a.detect(),
            extension_count: 0,
            path,
            enabled,
        });
    }

    // Sort by user-defined order; agents without sort_order go to end (index 999)
    result.sort_by_key(|a| *order_map.get(&a.name).unwrap_or(&999));
    Ok(result)
}

#[tauri::command]
pub fn update_agent_path(state: State<AppState>, name: String, path: Option<String>) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.set_agent_path(&name, path.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_agent_enabled(state: State<AppState>, name: String, enabled: bool) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.set_agent_enabled(&name, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_agent_order(state: State<AppState>, names: Vec<String>) -> Result<(), String> {
    let adapters = adapter::all_adapters();
    let valid_names: std::collections::HashSet<&str> =
        adapters.iter().map(|a| a.name()).collect();
    if names.iter().any(|n| !valid_names.contains(n.as_str())) {
        return Err("Invalid agent name in order list".into());
    }
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.set_agent_order(&names).map_err(|e| e.to_string())
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
        cli_count: all.iter().filter(|e| e.kind == ExtensionKind::Cli).count(),
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
    manager::toggle_extension(&store, &id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_audit_results(state: State<AppState>) -> Result<Vec<AuditResult>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.list_latest_audit_results().map_err(|e| e.to_string())
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
    let mut inputs = Vec::new();

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
                            if scanner::stable_id_for(&name, "skill", a.name()) == ext.id {
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
                        if scanner::stable_id_for(&server.name, "mcp", a.name()) == ext.id {
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
                // ext.name format is "event:matcher:command" — extract the raw command for audit
                let raw_command = ext.name.splitn(3, ':').nth(2).unwrap_or(&ext.name).to_string();
                (raw_command, None, vec![], Default::default(), ext.name.clone())
            }
            ExtensionKind::Plugin => {
                (String::new(), None, vec![], Default::default(), ext.name.clone())
            }
            ExtensionKind::Cli => {
                (String::new(), None, vec![], Default::default(), ext.name.clone())
            }
        };

        let mut input = hk_core::auditor::AuditInput {
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
            permissions: ext.permissions.clone(),
            cli_meta: ext.cli_meta.clone(),
            child_permissions: vec![],
        };
        if ext.kind == hk_core::models::ExtensionKind::Cli {
            let store = state.store.lock().map_err(|e| e.to_string())?;
            if let Ok(children) = store.get_child_skills(&ext.id) {
                input.child_permissions = children.into_iter().flat_map(|c| c.permissions).collect();
            }
        }
        inputs.push(input);
    }

    let results = auditor.audit_batch(&inputs);

    // Re-acquire lock briefly to store results
    let store = state.store.lock().map_err(|e| e.to_string())?;
    for result in &results {
        let _ = store.insert_audit_result(result);
    }
    Ok(results)
}

#[tauri::command]
pub fn delete_extension(state: State<AppState>, id: String) -> Result<(), String> {
    // Load extension metadata first
    let ext = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store.get_extension(&id).map_err(|e| e.to_string())?
            .ok_or_else(|| "Extension not found".to_string())?
    };

    let adapters = adapter::all_adapters();

    // Actually remove from disk/config based on extension kind
    match ext.kind {
        ExtensionKind::Skill => {
            // Delete skill file or directory
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
                        if scanner::stable_id_for(&name, "skill", adapter.name()) == id {
                            if path.is_dir() {
                                std::fs::remove_dir_all(&path)
                                    .map_err(|e| format!("Failed to delete skill directory: {}", e))?;
                            } else {
                                std::fs::remove_file(&path)
                                    .map_err(|e| format!("Failed to delete skill file: {}", e))?;
                            }
                        }
                    }
                }
            }
        }
        ExtensionKind::Mcp => {
            // Remove MCP server entry from agent config
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for server in adapter.read_mcp_servers() {
                    if scanner::stable_id_for(&server.name, "mcp", adapter.name()) == id {
                        let config_path = adapter.mcp_config_path();
                        deployer::remove_mcp_server(&config_path, &server.name)
                            .map_err(|e| format!("Failed to remove MCP server config: {}", e))?;
                    }
                }
            }
        }
        ExtensionKind::Hook => {
            // Remove hook entry from agent config
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for hook in adapter.read_hooks() {
                    let hook_name = format!("{}:{}:{}", hook.event, hook.matcher.as_deref().unwrap_or("*"), hook.command);
                    if scanner::stable_id_for(&hook_name, "hook", adapter.name()) == id {
                        let config_path = adapter.hook_config_path();
                        deployer::remove_hook(&config_path, &hook.event, hook.matcher.as_deref(), &hook.command)
                            .map_err(|e| format!("Failed to remove hook config: {}", e))?;
                    }
                }
            }
        }
        ExtensionKind::Cli => {
            // CLI uninstall not yet implemented
        }
        ExtensionKind::Plugin => {
            // Delete plugin files/config from disk
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for plugin in adapter.read_plugins() {
                    if scanner::stable_id_for(&format!("{}:{}", plugin.name, plugin.source), "plugin", adapter.name()) == id {
                        if adapter.name() == "claude" {
                            // For Claude: remove from enabledPlugins in settings.json
                            let config_path = adapter.mcp_config_path();
                            // Reconstruct the key format "name@source"
                            let plugin_key = if plugin.source.is_empty() {
                                plugin.name.clone()
                            } else {
                                format!("{}@{}", plugin.name, plugin.source)
                            };
                            deployer::remove_plugin_entry(&config_path, &plugin_key)
                                .map_err(|e| format!("Failed to remove plugin entry: {}", e))?;
                        } else if let Some(ref path) = plugin.path {
                            // For Codex: path points to a version dir inside
                            // cache/{marketplace}/{plugin}/{version}/ — delete the plugin-name
                            // parent so every version is removed and the plugin doesn't "come
                            // back" on re-scan.
                            // For Cursor/others: path already points to the plugin dir itself,
                            // so we delete it directly.
                            let target = if adapter.name() == "codex" {
                                // Go up one level from the version dir to the plugin-name dir,
                                // but guard against the parent being a root-level directory
                                if let Some(parent) = path.parent() {
                                    if parent.file_name().map(|n| n != "cache" && n != "plugins").unwrap_or(false) {
                                        parent
                                    } else {
                                        path.as_path()
                                    }
                                } else {
                                    path.as_path()
                                }
                            } else {
                                path.as_path()
                            };
                            if target.is_dir() {
                                std::fs::remove_dir_all(target)
                                    .map_err(|e| format!("Failed to delete plugin directory: {}", e))?;
                            } else if target.is_file() {
                                std::fs::remove_file(target)
                                    .map_err(|e| format!("Failed to delete plugin file: {}", e))?;
                            }
                        }
                    }
                }
            }
        }
    }

    // Remove from database (only after successful disk/config deletion)
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.delete_extension(&id).map_err(|e| e.to_string())
}

/// Audit a single extension by name (best-effort).
/// Takes owned extensions list so the caller can release the Mutex before calling this.
/// Returns audit results to be stored by the caller.
fn audit_extension_by_name(name: &str, extensions: &[Extension], adapters: &[Box<dyn adapter::AgentAdapter>]) -> Vec<AuditResult> {
    let auditor = Auditor::new();
    let mut results = Vec::new();
    for ext in extensions {
        if ext.name != name { continue; }
        let input = match ext.kind {
            ExtensionKind::Skill => {
                let mut content = String::new();
                let mut file_path = ext.name.clone();
                'outer: for a in adapters {
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
                            let sname = scanner::parse_skill_name(&skill_file).unwrap_or_else(||
                                path.file_stem().unwrap_or_default().to_string_lossy().to_string()
                            );
                            if scanner::stable_id_for(&sname, "skill", a.name()) == ext.id {
                                content = std::fs::read_to_string(&skill_file).unwrap_or_default();
                                file_path = skill_file.to_string_lossy().to_string();
                                break 'outer;
                            }
                        }
                    }
                }
                auditor::AuditInput {
                    extension_id: ext.id.clone(),
                    kind: ext.kind,
                    name: ext.name.clone(),
                    content,
                    source: ext.source.clone(),
                    file_path,
                    mcp_command: None,
                    mcp_args: vec![],
                    mcp_env: Default::default(),
                    installed_at: ext.installed_at,
                    updated_at: ext.updated_at,
                    permissions: ext.permissions.clone(),
                    cli_meta: ext.cli_meta.clone(),
                    child_permissions: vec![],
                }
            }
            _ => continue,
        };
        results.push(auditor.audit(&input));
    }
    results
}


#[derive(serde::Serialize)]
pub struct ExtensionContent {
    pub content: String,
    pub path: Option<String>,
    /// If the extension directory/file is a symlink, the resolved target path.
    pub symlink_target: Option<String>,
}

// ---------------------------------------------------------------------------
// File tree & system open
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    /// Children of a directory. `None` for files.
    pub children: Option<Vec<FileEntry>>,
}

/// List files in a skill directory as a shallow tree (2 levels deep).
#[tauri::command]
pub fn list_skill_files(state: State<AppState>, path: String) -> Result<Vec<FileEntry>, String> {
    let root = std::path::Path::new(&path);
    if !root.is_dir() {
        return Err("Path is not a directory".into());
    }
    if !is_path_within_allowed_dirs(root, &state)? {
        return Err("Path is not within a known agent or project directory".into());
    }
    list_dir_entries(root, 0)
}

fn list_dir_entries(dir: &std::path::Path, depth: u8) -> Result<Vec<FileEntry>, String> {
    let mut entries = Vec::new();
    let mut read = std::fs::read_dir(dir).map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .collect::<Vec<_>>();
    // Sort: directories first, then files, alphabetically within each group
    // Sort: SKILL.md first, then directories, then files, alphabetically within each group
    read.sort_by(|a, b| {
        let a_name = a.file_name();
        let b_name = b.file_name();
        let a_skill = a_name == "SKILL.md";
        let b_skill = b_name == "SKILL.md";
        if a_skill != b_skill { return b_skill.cmp(&a_skill); }
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| a_name.cmp(&b_name))
    });
    for entry in read {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files/dirs
        if name.starts_with('.') { continue; }
        let path = entry.path();
        let is_dir = path.is_dir();
        let children = if is_dir && depth < 1 {
            Some(list_dir_entries(&path, depth + 1)?)
        } else if is_dir {
            // Beyond depth limit, return empty children (frontend knows it's a dir)
            Some(vec![])
        } else {
            None
        };
        entries.push(FileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
            children,
        });
    }
    Ok(entries)
}

/// Open a file or directory in the system's default application.
#[tauri::command]
pub fn open_in_system(state: State<AppState>, path: String) -> Result<(), String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err("Path does not exist".into());
    }
    if !is_path_within_allowed_dirs(file_path, &state)? {
        return Err("Path is not within a known agent or project directory".into());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }
    Ok(())
}

/// For a given skill name, find all physical paths where it exists across all agents.
/// Returns Vec<(agent_name, path)> for display in the detail panel.
#[tauri::command]
pub fn get_skill_locations(name: String) -> Vec<(String, String)> {
    let adapters = adapter::all_adapters();
    let mut locations = Vec::new();
    for adapter in &adapters {
        if !adapter.detect() { continue; }
        for skill_dir in adapter.skill_dirs() {
            let skill_path = skill_dir.join(&name);
            let has_skill = skill_path.join("SKILL.md").exists()
                || skill_path.join("SKILL.md.disabled").exists();
            if has_skill {
                locations.push((
                    adapter.name().to_string(),
                    skill_path.to_string_lossy().to_string(),
                ));
            }
        }
    }
    locations
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
                    // Check if the skill_dir itself is a symlink
                    // (e.g. ~/.gemini/antigravity/skills → ~/.claude/skills)
                    let dir_symlink_target = if skill_dir.symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false) {
                        std::fs::read_link(&skill_dir).ok()
                    } else {
                        None
                    };
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
                        if scanner::stable_id_for(&name, "skill", adapter.name()) == id {
                            let dir = if path.is_dir() {
                                path.to_string_lossy().to_string()
                            } else {
                                skill_file.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
                            };
                            // Detect symlink: check entry itself, then parent dir
                            let symlink_target = if path.symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false) {
                                // Entry itself is a symlink
                                std::fs::read_link(&path).ok().map(|t| t.to_string_lossy().to_string())
                            } else if let Some(ref resolved_dir) = dir_symlink_target {
                                // Entry is real but accessed through a symlinked parent dir
                                let entry_name = path.file_name().unwrap_or_default();
                                Some(resolved_dir.join(entry_name).to_string_lossy().to_string())
                            } else {
                                None
                            };
                            let content = std::fs::read_to_string(&skill_file).map_err(|e| e.to_string())?;
                            return Ok(ExtensionContent { content, path: Some(dir), symlink_target });
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
                    if scanner::stable_id_for(&server.name, "mcp", adapter.name()) == id {
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
                            symlink_target: None,
                        });
                    }
                }
            }
            Ok(ExtensionContent {
                content: ext.description,
                path: None,
                symlink_target: None,
            })
        }
        ExtensionKind::Hook => {
            let adapters = adapter::all_adapters();
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for hook in adapter.read_hooks() {
                    let hook_name = format!("{}:{}:{}", hook.event, hook.matcher.as_deref().unwrap_or("*"), hook.command);
                    if scanner::stable_id_for(&hook_name, "hook", adapter.name()) == id {
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
                            symlink_target: None,
                        });
                    }
                }
            }
            Ok(ExtensionContent {
                content: ext.description,
                path: None,
                symlink_target: None,
            })
        }
        ExtensionKind::Plugin => {
            let adapters = adapter::all_adapters();
            for adapter in &adapters {
                if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                for plugin in adapter.read_plugins() {
                    if scanner::stable_id_for(&format!("{}:{}", plugin.name, plugin.source), "plugin", adapter.name()) == id {
                        let path_str = plugin.path.as_ref()
                            .map(|p| p.to_string_lossy().to_string());
                        return Ok(ExtensionContent {
                            content: ext.description,
                            path: path_str,
                            symlink_target: None,
                        });
                    }
                }
            }
            Ok(ExtensionContent {
                content: ext.description,
                path: None,
                symlink_target: None,
            })
        }
        ExtensionKind::Cli => {
            Ok(ExtensionContent {
                content: ext.description,
                path: None,
                symlink_target: None,
            })
        }
    }
}

#[tauri::command]
pub fn scan_and_sync(state: State<AppState>) -> Result<usize, String> {
    // Scan filesystem WITHOUT holding the lock — this is the slow part
    let adapters = adapter::all_adapters();
    let extensions = scanner::scan_all(&adapters);
    let count = extensions.len();

    // Lock briefly for a single transactional write (fast — one fsync total)
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.sync_extensions(&extensions).map_err(|e| e.to_string())?;
    Ok(count)
}

#[tauri::command]
pub fn get_cached_update_statuses(state: State<AppState>) -> Result<Vec<(String, UpdateStatus)>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let extensions = store.list_extensions(None, None).map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for ext in extensions {
        let Some(meta) = ext.install_meta else { continue };
        // Only include extensions that have been checked before
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
                // No local revision (pre-existing skill) — treat as update available
                UpdateStatus::UpdateAvailable { remote_hash: remote.to_string() }
            }
            _ => {
                if let Some(err) = meta.check_error {
                    UpdateStatus::Error { message: err }
                } else {
                    continue; // No remote_revision and no error — nothing to report
                }
            }
        };
        results.push((ext.id, status));
    }
    Ok(results)
}

#[tauri::command]
pub async fn check_updates(state: State<'_, AppState>) -> Result<Vec<(String, UpdateStatus)>, String> {
    let store_clone = state.store.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<(String, UpdateStatus)>, String> {
        // Read all extensions and release the lock before doing slow network calls
        let (updatable, unlinked): (Vec<(String, InstallMeta)>, Vec<(String, String)>) = {
            let store = store_clone.lock().map_err(|e| e.to_string())?;
            let extensions = store.list_extensions(None, None).map_err(|e| e.to_string())?;
            let mut has_meta = Vec::new();
            let mut no_meta = Vec::new();
            for e in extensions {
                if let Some(meta) = e.install_meta {
                    match meta.install_type.as_str() {
                        "git" | "marketplace" => has_meta.push((e.id, meta)),
                        _ => {}
                    }
                } else if e.kind == ExtensionKind::Skill {
                    no_meta.push((e.id, e.name));
                }
            }
            (has_meta, no_meta)
        };

        // Try to match unlinked skills against marketplace by name.
        // Only link when there is exactly one result with an exact name match.
        if !unlinked.is_empty() {
            let unique_names: std::collections::HashSet<&str> =
                unlinked.iter().map(|(_, name)| name.as_str()).collect();

            // For each unique name, search marketplace and resolve remote revision
            let mut matched: std::collections::HashMap<String, (String, String, Option<String>)> = std::collections::HashMap::new();
            for name in &unique_names {
                if let Ok(results) = marketplace::search_skills(name, 5) {
                    let exact: Vec<_> = results.iter()
                        .filter(|r| r.name.eq_ignore_ascii_case(name))
                        .collect();
                    if exact.len() == 1 {
                        let item = exact[0];
                        let git_url = marketplace::git_url_for_source(&item.source);
                        // Get current remote HEAD as baseline — so we don't falsely
                        // show "update available" when we don't know the local version
                        let remote_rev = manager::get_remote_head(&git_url).ok();
                        matched.insert(
                            name.to_string(),
                            (git_url, item.skill_id.clone(), remote_rev),
                        );
                    }
                }
            }

            if !matched.is_empty() {
                let store = store_clone.lock().map_err(|e| e.to_string())?;
                let now = Utc::now();
                for (id, name) in &unlinked {
                    if let Some((git_url, skill_id, remote_rev)) = matched.get(name.as_str()) {
                        // Set revision = remote_rev as baseline: "assume local is at this version"
                        // Next check_updates will detect if remote moved past this point
                        let meta = InstallMeta {
                            install_type: "marketplace".into(),
                            url: Some(format!("{}/{}", &git_url.trim_end_matches(".git"), skill_id)),
                            url_resolved: Some(git_url.clone()),
                            branch: None,
                            subpath: if skill_id.is_empty() { None } else { Some(skill_id.clone()) },
                            revision: remote_rev.clone(),
                            remote_revision: remote_rev.clone(),
                            checked_at: Some(now),
                            check_error: None,
                        };
                        let _ = store.set_install_meta(id, &meta);
                        // Don't push to updatable — we already know the status (up-to-date)
                        // and don't need to call get_remote_head again
                    }
                }
            }
        }

        // Check each extension for updates (network-heavy: git ls-remote per extension)
        let statuses: Vec<_> = updatable
            .iter()
            .map(|(id, meta)| {
                let status = manager::check_update(meta);
                (id.clone(), meta.clone(), status)
            })
            .collect();

        // Persist check state
        let store = store_clone.lock().map_err(|e| e.to_string())?;
        let now = Utc::now();
        for (id, _meta, status) in &statuses {
            let (remote_rev, check_err) = match status {
                UpdateStatus::UpToDate { remote_hash } => (Some(remote_hash.as_str()), None),
                UpdateStatus::UpdateAvailable { remote_hash } => (Some(remote_hash.as_str()), None),
                UpdateStatus::Error { message } => (None, Some(message.as_str())),
            };
            let _ = store.update_check_state(id, remote_rev, now, check_err);
        }

        Ok(statuses.into_iter().map(|(id, _, status)| (id, status)).collect())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_extension(state: State<'_, AppState>, id: String) -> Result<manager::InstallResult, String> {
    let store_clone = state.store.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<manager::InstallResult, String> {
    let (ext, install_meta) = {
        let store = store_clone.lock().map_err(|e| e.to_string())?;
        let ext = store.get_extension(&id).map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Extension '{}' not found", id))?;
        let meta = ext.install_meta.clone()
            .ok_or("Extension has no install metadata — cannot update")?;
        match meta.install_type.as_str() {
            "git" | "marketplace" => {}
            _ => return Err(format!("Extensions with install type '{}' cannot be updated", meta.install_type)),
        }
        (ext, meta)
    };
    let url = install_meta.url_resolved.as_deref()
        .or(install_meta.url.as_deref())
        .ok_or("Extension has no remote URL")?;

    // Clone the repo once
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let clone_dir = temp.path().join("repo");
    let output = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, &clone_dir.to_string_lossy()])
        .output()
        .map_err(|e| format!("Failed to run git clone: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed: {}", stderr.trim()));
    }
    let revision = manager::capture_git_revision_pub(&clone_dir);

    let skill_name = &ext.name;
    let skill_source = manager::find_skill_in_repo(&clone_dir, skill_name)
        .ok_or_else(|| format!("Skill '{}' not found in repository", skill_name))?;

    // Find all installed paths (deduplicated) and copy the latest version to each
    let all_siblings: Vec<Extension> = {
        let store = store_clone.lock().map_err(|e| e.to_string())?;
        let all = store.list_extensions(Some(ext.kind), None).map_err(|e| e.to_string())?;
        all.into_iter()
            .filter(|e| e.name == ext.name && e.source_path.is_some())
            .collect()
    };

    let mut updated_dirs = std::collections::HashSet::new();
    for sibling in &all_siblings {
        let source_path = sibling.source_path.as_deref().unwrap();
        let skill_dir = std::path::Path::new(source_path).parent()
            .ok_or("Cannot determine skill directory from source path")?;
        if !updated_dirs.insert(skill_dir.to_string_lossy().to_string()) {
            continue;
        }
        deployer::deploy_skill(&skill_source, skill_dir.parent().unwrap_or(skill_dir))
            .map_err(|e| e.to_string())?;
    }

    // Update install metadata for all siblings
    {
        let store = store_clone.lock().map_err(|e| e.to_string())?;
        let updated_meta = InstallMeta {
            revision: revision.clone().or(install_meta.revision.clone()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
            ..install_meta
        };
        for sibling in &all_siblings {
            let _ = store.set_install_meta(&sibling.id, &updated_meta);
        }
    }

    // Re-scan affected agents only and persist
    let adapters = adapter::all_adapters();
    let affected_agents: std::collections::HashSet<String> = all_siblings.iter()
        .flat_map(|s| s.agents.iter().cloned())
        .collect();
    {
        let store = store_clone.lock().map_err(|e| e.to_string())?;
        for a in &adapters {
            if affected_agents.contains(a.name()) {
                let exts = scanner::scan_adapter(a.as_ref());
                store.sync_extensions_for_agent(a.name(), &exts).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(manager::InstallResult {
        name: skill_name.clone(),
        was_update: true,
        revision,
    })
    }).await.map_err(|e| e.to_string())?
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
        let store = state.store.lock().map_err(|e| e.to_string())?;
        for a in &adapters {
            if agents.contains(&a.name().to_string()) {
                let exts = scanner::scan_adapter(a.as_ref());
                store.sync_extensions_for_agent(a.name(), &exts).map_err(|e| e.to_string())?;
                extensions.extend(exts);
            }
        }
        // Save install metadata for each agent
        let meta = InstallMeta {
            install_type: "local".into(),
            url: Some(path.clone()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: None,
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        for agent_name in &agents {
            let ext_id = scanner::stable_id_for(&skill_name, "skill", agent_name);
            let _ = store.set_install_meta(&ext_id, &meta);
        }
    }

    // Audit
    let audit_results = audit_extension_by_name(&result.name, &extensions, &adapters);
    if !audit_results.is_empty() {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        for r in &audit_results {
            let _ = store.insert_audit_result(r);
        }
    }

    Ok(result)
}

#[tauri::command]
pub fn install_from_git(state: State<AppState>, url: String, target_agent: Option<String>, skill_id: Option<String>) -> Result<manager::InstallResult, String> {
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
        let store = state.store.lock().map_err(|e| e.to_string())?;
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

// --- Multi-skill git install flow ---

#[derive(serde::Serialize)]
#[serde(tag = "type")]
pub enum ScanResult {
    Installed { result: manager::InstallResult },
    MultipleSkills { clone_id: String, skills: Vec<manager::DiscoveredSkill> },
    NoSkills,
}

#[tauri::command]
pub async fn scan_git_repo(state: State<'_, AppState>, url: String, target_agents: Vec<String>) -> Result<ScanResult, String> {
    // Clean up stale pending clones (older than 10 minutes)
    if let Ok(mut clones) = state.pending_clones.lock() {
        clones.retain(|_, v| v.created_at.elapsed().as_secs() < 600);
    }

    let store_clone = state.store.clone();
    let pending_clones = state.pending_clones.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<ScanResult, String> {
        let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let clone_dir = temp.path().join("repo");

        let output = std::process::Command::new("git")
            .args(["clone", "--depth", "1", &url, &clone_dir.to_string_lossy()])
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
                    let store = store_clone.lock().map_err(|e| e.to_string())?;
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
                        for agent_name in &installed_agents {
                            let ext_id = scanner::stable_id_for(&result.name, "skill", agent_name);
                            let _ = store.set_install_meta(&ext_id, &meta);
                        }
                    }
                }

                Ok(ScanResult::Installed { result: last_result.unwrap() })
            }
            _ => {
                // Multiple skills -- cache the clone and return the list
                let clone_id = uuid::Uuid::new_v4().to_string();
                let mut clones = pending_clones.lock().map_err(|e| e.to_string())?;
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
        let mut clones = state.pending_clones.lock().map_err(|e| e.to_string())?;
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
            let store = store_clone.lock().map_err(|e| e.to_string())?;
            for a in &adapters {
                if target_agents.contains(&a.name().to_string()) {
                    let exts = scanner::scan_adapter(a.as_ref());
                    store.sync_extensions_for_agent(a.name(), &exts).map_err(|e| e.to_string())?;
                }
            }
            // Persist install source metadata for each installed skill+agent
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
            }
        }

        // pending._temp_dir is dropped here, cleaning up the clone
        let results = results.into_iter().map(|(_, _, r)| r).collect();
        Ok(results)
    }).await.map_err(|e| e.to_string())?
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
pub async fn search_marketplace(query: String, kind: String, limit: Option<usize>) -> Result<Vec<hk_core::marketplace::MarketplaceItem>, String> {
    let lim = limit.unwrap_or(20);
    match kind.as_str() {
        "mcp" => hk_core::marketplace::search_servers_async(&query, lim).await,
        _ => hk_core::marketplace::search_skills_async(&query, lim).await,
    }.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trending_marketplace(kind: String, limit: Option<usize>) -> Result<Vec<hk_core::marketplace::MarketplaceItem>, String> {
    let lim = limit.unwrap_or(10);
    match kind.as_str() {
        "mcp" => hk_core::marketplace::trending_servers_async(lim).await,
        _ => hk_core::marketplace::trending_skills_async(lim).await,
    }.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_skill_preview(source: String, skill_id: String) -> Result<String, String> {
    hk_core::marketplace::fetch_skill_content_async(&source, &skill_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_skill_audit(source: String, skill_id: String) -> Result<Option<hk_core::marketplace::SkillAuditInfo>, String> {
    hk_core::marketplace::fetch_audit_info_async(&source, &skill_id).await.map_err(|e| e.to_string())
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
    let git_url = hk_core::marketplace::git_url_for_source(&source);
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
                        if scanner::stable_id_for(&name, "skill", adapter.name()) == extension_id {
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

            // Re-scan target agent to pick up the deployed extension
            let store = state.store.lock().map_err(|e| e.to_string())?;
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
            deployer::deploy_mcp_server(&config_path, &entry).map_err(|e| e.to_string())?;

            // Re-scan target agent
            let store = state.store.lock().map_err(|e| e.to_string())?;
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
            let entry = source_entry.ok_or_else(|| "Could not find source hook config".to_string())?;
            let config_path = target_adapter.hook_config_path();
            deployer::deploy_hook(&config_path, &entry).map_err(|e| e.to_string())?;

            // Re-scan target agent
            let store = state.store.lock().map_err(|e| e.to_string())?;
            let exts = scanner::scan_adapter(target_adapter.as_ref());
            store.sync_extensions_for_agent(target_adapter.name(), &exts).map_err(|e| e.to_string())?;
            Ok(format!("{}:{}", entry.event, entry.command))
        }
        other => Err(format!("Cross-agent deploy not supported for '{}' extensions", other)),
    }
}

// --- Project commands ---

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> Result<Vec<Project>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let mut projects = store.list_projects().map_err(|e| e.to_string())?;
    for p in &mut projects {
        p.exists = std::path::Path::new(&p.path).exists();
    }
    Ok(projects)
}

#[tauri::command]
pub fn add_project(state: State<AppState>, path: String) -> Result<Project, String> {
    // Canonicalize to prevent duplicates via symlinks/relative paths
    let project_path = std::path::Path::new(&path)
        .canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;
    let path = project_path.to_string_lossy().to_string();

    // Validate the path contains project markers for any supported agent
    let has_agent_config =
        project_path.join(".claude").is_dir()
        || project_path.join(".mcp.json").is_file()
        || project_path.join(".codex").is_dir()
        || project_path.join(".gemini").is_dir()
        || project_path.join(".cursor").join("rules").is_dir()
        || project_path.join(".cursorrules").is_file()
        || project_path.join(".github").join("copilot-instructions.md").is_file()
        || project_path.join(".github").join("instructions").is_dir()
        || project_path.join(".agent").join("rules").is_dir()
        || project_path.join(".agent").join("skills").is_dir();
    if !has_agent_config {
        return Err("Directory does not contain any recognized agent configuration".to_string());
    }

    // Check for duplicate before insert
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let existing = store.list_projects().map_err(|e| e.to_string())?;
    if existing.iter().any(|p| p.path == path) {
        return Err("Project already added".to_string());
    }

    // Generate stable ID from path hash
    let id = format!("proj-{:016x}", scanner::fnv1a(path.as_bytes()));

    let name = project_path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let project = Project {
        id: id.clone(),
        name,
        path,
        created_at: Utc::now(),
        exists: true,
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
pub fn list_agent_configs(state: State<AppState>) -> Result<Vec<AgentDetail>, String> {
    let adapters = adapter::all_adapters();
    let store = state.store.lock().map_err(|e| e.to_string())?;

    let projects: Vec<(String, String)> = store.list_projects()
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.name, p.path))
        .collect();

    let mut results = Vec::new();
    for a in &adapters {
        let detected = a.detect();
        let mut config_files = if detected {
            scanner::scan_agent_configs(a.as_ref(), &projects)
        } else {
            vec![]
        };

        // Merge user-defined custom config paths (skip if path already found by auto-scan)
        let existing_paths: std::collections::HashSet<String> = config_files.iter()
            .filter_map(|f| std::path::Path::new(&f.path).canonicalize().ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        if let Ok(custom_paths) = store.list_custom_config_paths(a.name()) {
            for (id, path, label, category_str) in custom_paths {
                let canonical = std::path::Path::new(&path).canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.clone());
                if existing_paths.contains(&canonical) { continue; }
                let category = match category_str.as_str() {
                    "rules" => ConfigCategory::Rules,
                    "memory" => ConfigCategory::Memory,
                    "ignore" => ConfigCategory::Ignore,
                    _ => ConfigCategory::Settings,
                };
                let p = std::path::Path::new(&path);
                let (size_bytes, modified_at, is_dir, exists) = if let Ok(meta) = std::fs::metadata(p) {
                    let modified = meta.modified().ok().map(|t| {
                        let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                        chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0).unwrap_or_default()
                    });
                    (meta.len(), modified, meta.is_dir(), true)
                } else {
                    (0, None, false, false)
                };
                config_files.push(AgentConfigFile {
                    path: path.clone(),
                    agent: a.name().to_string(),
                    category,
                    scope: ConfigScope::Global,
                    file_name: p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| path.clone()),
                    size_bytes,
                    modified_at,
                    is_dir,
                    exists,
                    custom_id: Some(id),
                    custom_label: Some(label),
                });
            }
        }

        let extensions = store.list_extensions(None, Some(a.name())).unwrap_or_default();
        let extension_counts = ExtensionCounts {
            skill: extensions.iter().filter(|e| e.kind == ExtensionKind::Skill).count(),
            mcp: extensions.iter().filter(|e| e.kind == ExtensionKind::Mcp).count(),
            plugin: extensions.iter().filter(|e| e.kind == ExtensionKind::Plugin).count(),
            hook: extensions.iter().filter(|e| e.kind == ExtensionKind::Hook).count(),
            cli: extensions.iter().filter(|e| e.kind == ExtensionKind::Cli).count(),
        };

        results.push(AgentDetail {
            name: a.name().to_string(),
            detected,
            config_files,
            extension_counts,
        });
    }
    Ok(results)
}

/// Validate that a path is within a known agent directory, registered project, or the app data dir.
fn is_path_within_allowed_dirs(path: &std::path::Path, state: &AppState) -> Result<bool, String> {
    let canonical = path.canonicalize().map_err(|e| format!("Invalid path: {}", e))?;
    let adapters = adapter::all_adapters();
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let projects = store.list_projects().unwrap_or_default();

    let allowed = adapters.iter().any(|a| {
            a.base_dir().canonicalize().map_or(false, |d| canonical.starts_with(&d))
        })
        || projects.iter().any(|p| {
            std::path::Path::new(&p.path).canonicalize().map_or(false, |d| canonical.starts_with(&d))
        })
        || dirs::home_dir().map(|h| h.join(".harnesskit"))
            .and_then(|d| d.canonicalize().ok())
            .map_or(false, |d| canonical.starts_with(&d));
    Ok(allowed)
}

#[tauri::command]
pub fn read_config_file_preview(state: State<AppState>, path: String, max_lines: Option<usize>) -> Result<String, String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err("File not found".into());
    }

    if !is_path_within_allowed_dirs(file_path, &state)? {
        return Err("Path is not within a known agent or project directory".into());
    }

    if file_path.is_dir() {
        return Ok(format_dir_tree(file_path, "", 0, 3));
    }

    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let limit = max_lines.unwrap_or(30);
    let preview: String = content
        .lines()
        .take(limit)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(preview)
}

fn format_dir_tree(dir: &std::path::Path, prefix: &str, depth: u8, max_depth: u8) -> String {
    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return String::new(),
    };
    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| {
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
    });
    // Skip hidden files/dirs
    entries.retain(|e| {
        !e.file_name().to_string_lossy().starts_with('.')
    });

    let mut lines = Vec::new();
    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if is_dir {
            lines.push(format!("{}{}{}/", prefix, connector, name));
            if depth < max_depth {
                let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                let subtree = format_dir_tree(&entry.path(), &child_prefix, depth + 1, max_depth);
                if !subtree.is_empty() {
                    lines.push(subtree);
                }
            }
        } else {
            lines.push(format!("{}{}{}", prefix, connector, name));
        }
    }
    lines.join("\n")
}

// --- CLI commands ---

#[tauri::command]
pub fn get_cli_with_children(
    state: State<AppState>,
    cli_id: String,
) -> Result<(Extension, Vec<Extension>), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let cli = store.get_extension(&cli_id).map_err(|e| e.to_string())?
        .ok_or_else(|| format!("CLI not found: {}", cli_id))?;
    let children = store.get_child_skills(&cli_id).map_err(|e| e.to_string())?;
    Ok((cli, children))
}

#[tauri::command]
pub fn list_cli_marketplace() -> Result<Vec<MarketplaceItem>, String> {
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

    // Step 1: Execute the install command from embedded registry
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&entry.install_command)
        .output()
        .map_err(|e| format!("Failed to run install command: {}", e))?;
    if !output.status.success() {
        return Err(format!("CLI install failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    // Step 2: Install skills
    let skills_cmd = entry.skills_install_command.unwrap_or_else(|| {
        format!("npx -y skills add {} -y -g", entry.skills_repo)
    });
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&skills_cmd)
        .output()
        .map_err(|e| format!("Failed to install skills: {}", e))?;
    if !output.status.success() {
        eprintln!("Warning: skills install had issues: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Step 3: Trigger re-scan
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let adapters = adapter::all_adapters();
    let exts = scanner::scan_all(&adapters);
    store.sync_extensions(&exts).map_err(|e| e.to_string())?;
    Ok(())
}

// --- Custom config path commands ---

#[tauri::command]
pub fn add_custom_config_path(
    state: State<AppState>,
    agent: String,
    path: String,
    label: String,
    category: String,
) -> Result<i64, String> {
    // Resolve ~ to home directory
    let resolved = if path.starts_with("~/") {
        dirs::home_dir()
            .map(|h| h.join(&path[2..]).to_string_lossy().to_string())
            .unwrap_or(path.clone())
    } else {
        path
    };
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.add_custom_config_path(&agent, &resolved, &label, &category).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_custom_config_path(
    state: State<AppState>,
    id: i64,
    path: String,
    label: String,
    category: String,
) -> Result<(), String> {
    let resolved = if path.starts_with("~/") {
        dirs::home_dir()
            .map(|h| h.join(&path[2..]).to_string_lossy().to_string())
            .unwrap_or(path.clone())
    } else {
        path
    };
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.update_custom_config_path(id, &resolved, &label, &category).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_custom_config_path(state: State<AppState>, id: i64) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.remove_custom_config_path(id).map_err(|e| e.to_string())
}
