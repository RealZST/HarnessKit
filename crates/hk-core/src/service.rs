use anyhow::Result;
use crate::{
    adapter::AgentAdapter,
    auditor::{AuditInput, Auditor},
    models::*,
    scanner,
    store::Store,
};

/// Common post-install flow: scan affected agents, sync to store, set install meta,
/// update pack, audit the installed extension(s), and persist audit results.
///
/// This extracts the duplicated 30-50 line pattern found in install_from_local,
/// install_from_git, install_from_marketplace, scan_git_repo, and install_scanned_skills.
pub fn post_install_sync(
    store: &Store,
    adapters: &[Box<dyn AgentAdapter>],
    agent_names: &[String],
    skill_name: &str,
    install_meta: Option<InstallMeta>,
    pack: Option<&str>,
) -> Result<Vec<Extension>> {
    // 1. Scan and sync affected agents
    let mut extensions = Vec::new();
    for a in adapters {
        if agent_names.contains(&a.name().to_string()) {
            let exts = scanner::scan_adapter(a.as_ref());
            store.sync_extensions_for_agent(a.name(), &exts)?;
            extensions.extend(exts);
        }
    }

    // 2. Set install meta and pack for each agent
    if let Some(ref meta) = install_meta {
        for agent_name in agent_names {
            let ext_id = scanner::stable_id_for(skill_name, "skill", agent_name);
            let _ = store.set_install_meta(&ext_id, meta);
            if let Some(p) = pack {
                let _ = store.update_pack(&ext_id, Some(p));
            }
        }
    }

    // 3. Audit the installed extensions
    let audit_results = audit_extension_by_name(skill_name, &extensions, adapters);
    for r in &audit_results {
        let _ = store.insert_audit_result(r);
    }

    Ok(extensions)
}

/// Full audit of all extensions — scans skill content, MCP server info, hooks, plugins,
/// and CLIs, then runs the auditor's rule engine and persists results.
///
/// This is the service-layer equivalent of the desktop `run_audit` command
/// and the CLI `cmd_audit` logic.
pub fn run_full_audit(
    store: &Store,
    adapters: &[Box<dyn AgentAdapter>],
) -> Result<Vec<AuditResult>> {
    let extensions = store.list_extensions(None, None)?;
    let auditor = Auditor::new();
    let mut inputs = Vec::new();

    for ext in &extensions {
        let (content, mcp_command, mcp_args, mcp_env, file_path) = match ext.kind {
            ExtensionKind::Skill => {
                let (skill_content, skill_path) = find_skill_content(adapters, &ext.id, &ext.agents);
                (skill_content, None, vec![], Default::default(), skill_path.unwrap_or_else(|| ext.name.clone()))
            }
            ExtensionKind::Mcp => {
                let mut cmd = None;
                let mut args = vec![];
                let mut env = std::collections::HashMap::new();
                for a in adapters {
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

        let mut input = AuditInput {
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
        if ext.kind == ExtensionKind::Cli
            && let Ok(children) = store.get_child_skills(&ext.id)
        {
            input.child_permissions = children.into_iter().flat_map(|c| c.permissions).collect();
        }
        inputs.push(input);
    }

    let results = auditor.audit_batch(&inputs);

    // Persist results
    for result in &results {
        let _ = store.insert_audit_result(result);
    }

    Ok(results)
}

/// Audit a single extension by name (best-effort, skills only).
/// Returns audit results to be stored by the caller.
fn audit_extension_by_name(
    name: &str,
    extensions: &[Extension],
    adapters: &[Box<dyn AgentAdapter>],
) -> Vec<AuditResult> {
    let auditor = Auditor::new();
    let mut results = Vec::new();
    for ext in extensions {
        if ext.name != name { continue; }
        let input = match ext.kind {
            ExtensionKind::Skill => {
                let (content, file_path) = find_skill_content(adapters, &ext.id, &ext.agents);
                AuditInput {
                    extension_id: ext.id.clone(),
                    kind: ext.kind,
                    name: ext.name.clone(),
                    content,
                    source: ext.source.clone(),
                    file_path: file_path.unwrap_or_else(|| ext.name.clone()),
                    mcp_command: None,
                    mcp_args: vec![],
                    mcp_env: Default::default(),
                    installed_at: ext.installed_at,
                    updated_at: ext.updated_at,
                    permissions: ext.permissions.clone(),
                    cli_parent_id: ext.cli_parent_id.clone(),
                    cli_meta: ext.cli_meta.clone(),
                    child_permissions: vec![],
                    pack: ext.pack.clone(),
                }
            }
            _ => continue,
        };
        results.push(auditor.audit(&input));
    }
    results
}

/// Find skill content and file path by scanning adapters for the matching extension.
fn find_skill_content(
    adapters: &[Box<dyn AgentAdapter>],
    ext_id: &str,
    agent_filter: &[String],
) -> (String, Option<String>) {
    for a in adapters {
        if !agent_filter.contains(&a.name().to_string()) { continue; }
        for skill_dir in a.skill_dirs() {
            let Ok(entries) = std::fs::read_dir(&skill_dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                let skill_file = if path.is_dir() {
                    let md = path.join("SKILL.md");
                    if md.exists() { md } else { path.join("SKILL.md.disabled") }
                } else if path.extension().is_some_and(|e| e == "md" || e == "disabled") {
                    path.clone()
                } else { continue };
                if !skill_file.exists() { continue; }
                let name = scanner::parse_skill_name(&skill_file).unwrap_or_else(||
                    path.file_stem().unwrap_or_default().to_string_lossy().to_string()
                );
                if scanner::stable_id_for(&name, "skill", a.name()) == ext_id {
                    let content = std::fs::read_to_string(&skill_file).unwrap_or_default();
                    return (content, Some(skill_file.to_string_lossy().to_string()));
                }
            }
        }
    }
    (String::new(), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use tempfile::TempDir;

    fn test_store() -> (Store, TempDir) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Store::open(&db_path).unwrap();
        (store, dir)
    }

    #[test]
    fn test_post_install_sync_empty_agents() {
        let (store, _dir) = test_store();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![];
        let result = post_install_sync(
            &store,
            &adapters,
            &[],
            "test-skill",
            None,
            None,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_run_full_audit_empty_store() {
        let (store, _dir) = test_store();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![];
        let result = run_full_audit(&store, &adapters);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
