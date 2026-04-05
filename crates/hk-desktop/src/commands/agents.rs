use hk_core::{adapter, models::*, scanner};
use tauri::State;
use super::AppState;

#[tauri::command]
pub fn list_agents(state: State<AppState>) -> Result<Vec<AgentInfo>, String> {
    let adapters = adapter::all_adapters();
    let store = state.store.lock();

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
    let store = state.store.lock();
    store.set_agent_path(&name, path.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_agent_enabled(state: State<AppState>, name: String, enabled: bool) -> Result<(), String> {
    let store = state.store.lock();
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
    let store = state.store.lock();
    store.set_agent_order(&names).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_agent_configs(state: State<AppState>) -> Result<Vec<AgentDetail>, String> {
    let adapters = adapter::all_adapters();
    let store = state.store.lock();

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
        // Exclude child skills (they're shown under their parent CLI)
        let top_level = extensions.iter().filter(|e| e.cli_parent_id.is_none());
        let extension_counts = ExtensionCounts {
            skill: top_level.clone().filter(|e| e.kind == ExtensionKind::Skill).count(),
            mcp: top_level.clone().filter(|e| e.kind == ExtensionKind::Mcp).count(),
            plugin: top_level.clone().filter(|e| e.kind == ExtensionKind::Plugin).count(),
            hook: top_level.clone().filter(|e| e.kind == ExtensionKind::Hook).count(),
            cli: top_level.filter(|e| e.kind == ExtensionKind::Cli).count(),
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
