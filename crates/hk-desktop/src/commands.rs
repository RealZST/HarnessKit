use hk_core::{adapter, auditor::Auditor, models::*, scanner, store::Store};
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
    Ok(DashboardStats {
        total_extensions: all.len(),
        skill_count: all.iter().filter(|e| e.kind == ExtensionKind::Skill).count(),
        mcp_count: all.iter().filter(|e| e.kind == ExtensionKind::Mcp).count(),
        plugin_count: all.iter().filter(|e| e.kind == ExtensionKind::Plugin).count(),
        hook_count: all.iter().filter(|e| e.kind == ExtensionKind::Hook).count(),
        critical_issues: 0,
        high_issues: 0,
        medium_issues: 0,
        low_issues: 0,
        updates_available: 0,
    })
}

#[tauri::command]
pub fn toggle_extension(state: State<AppState>, id: String, enabled: bool) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.set_enabled(&id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn run_audit(state: State<AppState>) -> Result<Vec<AuditResult>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let extensions = store.list_extensions(None, None).map_err(|e| e.to_string())?;
    let auditor = Auditor::new();
    let mut results = Vec::new();
    for ext in &extensions {
        let input = hk_core::auditor::AuditInput {
            extension_id: ext.id.clone(),
            kind: ext.kind,
            name: ext.name.clone(),
            content: String::new(),
            source: ext.source.clone(),
            file_path: ext.name.clone(),
            mcp_command: None,
            mcp_args: vec![],
            mcp_env: Default::default(),
            installed_at: ext.installed_at,
            updated_at: ext.updated_at,
        };
        let result = auditor.audit(&input);
        let _ = store.insert_audit_result(&result);
        results.push(result);
    }
    Ok(results)
}

#[tauri::command]
pub fn scan_and_sync(state: State<AppState>) -> Result<usize, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let adapters = adapter::all_adapters();
    let extensions = scanner::scan_all(&adapters);
    let count = extensions.len();
    // Upsert: stable IDs + INSERT OR REPLACE ensures no duplicates
    for ext in &extensions {
        let _ = store.insert_extension(ext);
    }
    Ok(count)
}
