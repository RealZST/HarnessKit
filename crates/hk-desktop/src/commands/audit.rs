use hk_core::{adapter, auditor::{self, Auditor}, models::*, scanner};
use tauri::State;
use super::AppState;
use super::helpers::find_skill_by_id;

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
                let (skill_content, skill_path) = if let Some(loc) = find_skill_by_id(&adapters, &ext.id, &ext.agents) {
                    (
                        std::fs::read_to_string(&loc.skill_file).unwrap_or_default(),
                        loc.skill_file.to_string_lossy().to_string(),
                    )
                } else {
                    (String::new(), ext.name.clone())
                };
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

        let mut input = auditor::AuditInput {
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
            cli_parent_id: ext.cli_parent_id.clone(),
            cli_meta: ext.cli_meta.clone(),
            child_permissions: vec![],
            pack: ext.pack.clone(),
        };
        if ext.kind == ExtensionKind::Cli {
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
