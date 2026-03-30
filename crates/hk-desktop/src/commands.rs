use hk_core::{adapter, auditor::{self, Auditor}, deployer, manager, models::*, scanner, store::Store};
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
pub fn list_agents(state: State<AppState>) -> Result<Vec<AgentInfo>, String> {
    let adapters = adapter::all_adapters();
    let store = state.store.lock().map_err(|e| e.to_string())?;
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

    let ext = store.get_extension(&id).map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Extension not found: {}", id))?;

    match ext.kind {
        ExtensionKind::Skill => {
            toggle_skill_file(&ext, enabled).map_err(|e| e.to_string())?;
            let sibling_ids = store.find_siblings_by_source_path(&id).map_err(|e| e.to_string())?;
            for sib_id in &sibling_ids {
                store.set_enabled(sib_id, enabled).map_err(|e| e.to_string())?;
            }
        }
        ExtensionKind::Mcp => {
            toggle_mcp_config(&ext, enabled, &store).map_err(|e| e.to_string())?;
            store.set_enabled(&id, enabled).map_err(|e| e.to_string())?;
        }
        ExtensionKind::Hook => {
            toggle_hook_config(&ext, enabled, &store).map_err(|e| e.to_string())?;
            store.set_enabled(&id, enabled).map_err(|e| e.to_string())?;
        }
        ExtensionKind::Plugin => {
            toggle_plugin_config(&ext, enabled, &store).map_err(|e| e.to_string())?;
            store.set_enabled(&id, enabled).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn toggle_skill_file(ext: &Extension, enabled: bool) -> anyhow::Result<()> {
    let source_path = ext.source_path.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Skill has no source_path"))?;
    let skill_file = std::path::PathBuf::from(source_path);
    let disabled_file = skill_file.with_file_name("SKILL.md.disabled");
    if enabled {
        if disabled_file.exists() { std::fs::rename(&disabled_file, &skill_file)?; }
    } else if skill_file.exists() {
        std::fs::rename(&skill_file, &disabled_file)?;
    }
    Ok(())
}

fn toggle_mcp_config(ext: &Extension, enabled: bool, store: &Store) -> anyhow::Result<()> {
    let adapters = adapter::all_adapters();
    for a in &adapters {
        if !ext.agents.contains(&a.name().to_string()) { continue; }
        let config_path = a.mcp_config_path();
        if enabled {
            let saved = store.get_disabled_config(&ext.id)?
                .ok_or_else(|| anyhow::anyhow!("No saved config for MCP server '{}'", ext.name))?;
            let entry: serde_json::Value = serde_json::from_str(&saved)?;
            deployer::restore_mcp_server(&config_path, &ext.name, &entry)?;
            store.set_disabled_config(&ext.id, None)?;
        } else {
            let entry = deployer::read_mcp_server_config(&config_path, &ext.name)?
                .ok_or_else(|| anyhow::anyhow!("MCP server '{}' not found in config", ext.name))?;
            store.set_disabled_config(&ext.id, Some(&entry.to_string()))?;
            deployer::remove_mcp_server(&config_path, &ext.name)?;
        }
    }
    Ok(())
}

fn toggle_hook_config(ext: &Extension, enabled: bool, store: &Store) -> anyhow::Result<()> {
    let adapters = adapter::all_adapters();
    let parts: Vec<&str> = ext.name.splitn(3, ':').collect();
    if parts.len() < 3 { anyhow::bail!("Invalid hook name: {}", ext.name); }
    let (event, matcher_str, command) = (parts[0], parts[1], parts[2]);
    let matcher = if matcher_str == "*" { None } else { Some(matcher_str) };
    for a in &adapters {
        if !ext.agents.contains(&a.name().to_string()) { continue; }
        let config_path = a.hook_config_path();
        if enabled {
            let saved = store.get_disabled_config(&ext.id)?
                .ok_or_else(|| anyhow::anyhow!("No saved config for hook '{}'", ext.name))?;
            let entry: serde_json::Value = serde_json::from_str(&saved)?;
            deployer::restore_hook(&config_path, event, &entry)?;
            store.set_disabled_config(&ext.id, None)?;
        } else {
            let entry = deployer::read_hook_config(&config_path, event, matcher, command)?
                .ok_or_else(|| anyhow::anyhow!("Hook '{}' not found in config", ext.name))?;
            store.set_disabled_config(&ext.id, Some(&entry.to_string()))?;
            deployer::remove_hook(&config_path, event, matcher, command)?;
        }
    }
    Ok(())
}

fn toggle_plugin_config(ext: &Extension, enabled: bool, store: &Store) -> anyhow::Result<()> {
    let adapters = adapter::all_adapters();
    for a in &adapters {
        if !ext.agents.contains(&a.name().to_string()) { continue; }
        if a.name() == "claude" {
            let config_path = a.mcp_config_path();
            if enabled {
                // Re-enable: read plugin_key and value from saved disabled_config
                let saved = store.get_disabled_config(&ext.id)?
                    .ok_or_else(|| anyhow::anyhow!("No saved config for plugin '{}'", ext.name))?;
                let saved_obj: serde_json::Value = serde_json::from_str(&saved)?;
                let plugin_key = saved_obj.get("plugin_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Saved config missing plugin_key"))?;
                let value = saved_obj.get("value")
                    .ok_or_else(|| anyhow::anyhow!("Saved config missing value"))?;
                deployer::restore_plugin_entry(&config_path, plugin_key, value)?;
                store.set_disabled_config(&ext.id, None)?;
            } else {
                // Disable: find plugin_key from live config, save both key and value
                let plugin_key = {
                    let mut found_key = None;
                    for plugin in a.read_plugins() {
                        let id_name = format!("{}:{}", plugin.name, plugin.source);
                        if scanner::stable_id_for(&id_name, "plugin", a.name()) == ext.id {
                            found_key = Some(if plugin.source.is_empty() {
                                plugin.name.clone()
                            } else {
                                format!("{}@{}", plugin.name, plugin.source)
                            });
                            break;
                        }
                    }
                    found_key.ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found in agent config", ext.name))?
                };
                let value = deployer::read_plugin_config(&config_path, &plugin_key)?
                    .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found in config", ext.name))?;
                // Store both plugin_key and value so re-enable doesn't need the live config
                let saved = serde_json::json!({ "plugin_key": plugin_key, "value": value });
                store.set_disabled_config(&ext.id, Some(&saved.to_string()))?;
                deployer::remove_plugin_entry(&config_path, &plugin_key)?;
            }
        } else {
            if enabled {
                // Re-enable: read manifest path from saved disabled_config
                let saved = store.get_disabled_config(&ext.id)?
                    .ok_or_else(|| anyhow::anyhow!("No saved config for plugin '{}'", ext.name))?;
                let saved_obj: serde_json::Value = serde_json::from_str(&saved)?;
                let manifest_path = saved_obj.get("manifest_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Saved config missing manifest_path"))?;
                let disabled_manifest = std::path::PathBuf::from(manifest_path);
                // plugin.json.disabled -> strip ".disabled" suffix to get plugin.json
                let manifest = if disabled_manifest.to_string_lossy().ends_with(".disabled") {
                    let s = disabled_manifest.to_string_lossy();
                    std::path::PathBuf::from(&s[..s.len() - ".disabled".len()])
                } else {
                    disabled_manifest.clone()
                };
                if disabled_manifest.exists() {
                    std::fs::rename(&disabled_manifest, &manifest)?;
                }
                store.set_disabled_config(&ext.id, None)?;
            } else {
                // Disable: find plugin via live scan, rename manifest, save path
                for plugin in a.read_plugins() {
                    let plugin_id_name = format!("{}:{}", plugin.name, plugin.source);
                    if scanner::stable_id_for(&plugin_id_name, "plugin", a.name()) != ext.id { continue; }
                    if let Some(ref path) = plugin.path {
                        let manifest = path.join("plugin.json");
                        let disabled_manifest = path.join("plugin.json.disabled");
                        if manifest.exists() {
                            // Save the disabled manifest path so re-enable can find it
                            let saved = serde_json::json!({ "manifest_path": disabled_manifest.to_string_lossy() });
                            store.set_disabled_config(&ext.id, Some(&saved.to_string()))?;
                            std::fs::rename(&manifest, &disabled_manifest)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
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
pub fn list_skill_files(path: String) -> Result<Vec<FileEntry>, String> {
    let root = std::path::Path::new(&path);
    if !root.is_dir() {
        return Err("Path is not a directory".into());
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
pub fn open_in_system(path: String) -> Result<(), String> {
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
pub fn install_from_git(state: State<AppState>, url: String, target_agent: Option<String>, skill_id: Option<String>) -> Result<manager::InstallResult, String> {
    let adapters = adapter::all_adapters();
    let (target_dir, _agent_name) = if let Some(ref agent) = target_agent {
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

    // Re-scan and persist — hold lock only for fast DB writes
    let extensions = scanner::scan_all(&adapters);
    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        for ext in &extensions {
            let _ = store.insert_extension(ext);
        }
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
pub fn install_from_marketplace(state: State<AppState>, source: String, skill_id: String, target_agent: Option<String>) -> Result<manager::InstallResult, String> {
    let adapters = adapter::all_adapters();
    let (target_dir, _agent_name) = if let Some(ref agent) = target_agent {
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
    // Re-scan and persist — hold lock only for fast DB writes
    let extensions = scanner::scan_all(&adapters);
    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        for ext in &extensions {
            let _ = store.insert_extension(ext);
        }
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
        let config_files = if detected {
            scanner::scan_agent_configs(a.as_ref(), &projects)
        } else {
            vec![]
        };

        let extensions = store.list_extensions(None, Some(a.name())).unwrap_or_default();
        let extension_counts = ExtensionCounts {
            skill: extensions.iter().filter(|e| e.kind == ExtensionKind::Skill).count(),
            mcp: extensions.iter().filter(|e| e.kind == ExtensionKind::Mcp).count(),
            plugin: extensions.iter().filter(|e| e.kind == ExtensionKind::Plugin).count(),
            hook: extensions.iter().filter(|e| e.kind == ExtensionKind::Hook).count(),
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

#[tauri::command]
pub fn read_config_file_preview(state: State<AppState>, path: String, max_lines: Option<usize>) -> Result<String, String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err("File not found".into());
    }
    if !file_path.is_file() {
        return Err("Path is not a file".into());
    }

    // Validate path is under a known agent dir or registered project
    let canonical = file_path.canonicalize().map_err(|e| format!("Invalid path: {}", e))?;
    let adapters = adapter::all_adapters();
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let projects = store.list_projects().unwrap_or_default();

    let allowed = adapters.iter().any(|a| {
            a.base_dir().canonicalize().map_or(false, |d| canonical.starts_with(d))
        })
        || projects.iter().any(|p| {
            std::path::Path::new(&p.path).canonicalize().map_or(false, |d| canonical.starts_with(d))
        });

    if !allowed {
        return Err("Path is not within a known agent or project directory".into());
    }
    drop(store); // Release lock before file I/O

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
