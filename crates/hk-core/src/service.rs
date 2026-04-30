use crate::{
    HkError,
    adapter::AgentAdapter,
    auditor::{AuditInput, Auditor},
    deployer,
    models::*,
    scanner,
    store::Store,
};
use parking_lot::Mutex;

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
    target_scope: &ConfigScope,
) -> Result<Vec<Extension>, HkError> {
    // 1. Scan and sync affected agents — scope-aware.
    let mut extensions = Vec::new();
    for a in adapters {
        if !agent_names.contains(&a.name().to_string()) {
            continue;
        }
        match target_scope {
            ConfigScope::Global => {
                // Existing path: scan_adapter covers global skill_dirs / mcp /
                // hooks / plugins, and sync_extensions_for_agent's stale-removal
                // is correct here (we DO want stale global rows for this agent
                // to be cleaned up).
                let exts = scanner::scan_adapter(a.as_ref());
                store.sync_extensions_for_agent(a.name(), &exts)?;
                extensions.extend(exts);
            }
            ConfigScope::Project { name, path } => {
                // Project path: scan_project_extensions returns Project-scoped
                // rows with scope-aware stable_ids. We deliberately use
                // insert_extension (upsert-only, no stale removal) instead of
                // sync_extensions_for_agent — the latter would treat every
                // global row for this agent as stale (since they're absent
                // from the project scan) and delete the unprotected ones.
                let exts = scanner::scan_project_extensions(
                    a.as_ref(),
                    name,
                    std::path::Path::new(path),
                );
                for ext in &exts {
                    store.insert_extension(ext)?;
                }
                extensions.extend(exts);
            }
        }
    }

    // 2. Set install meta and pack for each agent — scope-aware id so the
    // right row gets updated.
    if let Some(ref meta) = install_meta {
        for agent_name in agent_names {
            let ext_id = scanner::stable_id_with_scope_for(
                skill_name,
                "skill",
                agent_name,
                target_scope,
            );
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
) -> Result<Vec<AuditResult>, HkError> {
    let extensions = store.list_extensions(None, None)?;
    let results = audit_extensions(&extensions, adapters);

    for result in &results {
        let _ = store.insert_audit_result(result);
    }

    Ok(results)
}

/// Run audit on a pre-fetched list of extensions without needing a store reference.
/// Useful when callers need to control lock scope separately for reads and writes.
pub fn audit_extensions(
    extensions: &[Extension],
    adapters: &[Box<dyn AgentAdapter>],
) -> Vec<AuditResult> {
    let auditor = Auditor::new();
    let mut inputs = Vec::new();

    for ext in extensions {
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
                let plugin_dir = ext.source_path.as_deref().unwrap_or(&ext.name);
                let content = read_plugin_content(plugin_dir);
                let file_path = ext.source_path.clone().unwrap_or_else(|| ext.name.clone());
                (content, None, vec![], Default::default(), file_path)
            }
            ExtensionKind::Cli => {
                (String::new(), None, vec![], Default::default(), ext.name.clone())
            }
        };

        let input = AuditInput {
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
        inputs.push(input);
    }

    auditor.audit_batch(&inputs)
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

/// Read source files from a plugin directory for audit analysis.
/// Returns concatenated content with file markers.
/// Reads .js, .ts, .py, .sh files up to a total of 512 KB.
/// NOTE: .json files are excluded — package.json is handled separately by
/// `infer_plugin_permissions` and `plugin-lifecycle-scripts` rule, and
/// package-lock.json would consume the entire read budget with URLs.
fn read_plugin_content(plugin_path: &str) -> String {
    use std::path::Path;

    let dir = Path::new(plugin_path);
    if !dir.is_dir() {
        return String::new();
    }

    let allowed_extensions = ["js", "ts", "py", "sh", "mjs", "cjs"];
    let max_total_bytes: usize = 512 * 1024;
    let mut total_bytes = 0usize;
    let mut parts = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else {
        return String::new();
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() { continue; }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !allowed_extensions.contains(&ext) { continue; }
        if let Ok(content) = std::fs::read_to_string(&path) {
            let bytes_to_add = content.len();
            if total_bytes + bytes_to_add > max_total_bytes { break; }
            parts.push(format!("// === {} ===\n{}", path.file_name().unwrap_or_default().to_string_lossy(), content));
            total_bytes += bytes_to_add;
        }
    }

    parts.join("\n")
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

// --- Extension command flows shared by hk-desktop and hk-web -------------

/// Rich detail returned by `get_extension_content`. Surfaces the on-disk
/// representation (file/dir path + readable text) so the UI's detail panel
/// can show it. `symlink_target` is only set for skills whose entry or
/// containing dir is a symlink — useful for development setups where the
/// user keeps the canonical copy elsewhere.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtensionContent {
    pub content: String,
    pub path: Option<String>,
    pub symlink_target: Option<String>,
}

/// Remove an extension from disk/config (per-kind) and then from the DB.
///
/// Disk and DB are mutated in two separately-locked phases so I/O does not
/// hold the store mutex. The DB delete happens last; if disk removal fails
/// the row stays so the next scan can recover.
pub fn delete_extension(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    id: &str,
) -> Result<(), HkError> {
    // Phase 1: read metadata under the lock, then drop it before any I/O.
    let (ext, projects) = {
        let store = store.lock();
        let ext = store
            .get_extension(id)?
            .ok_or_else(|| HkError::NotFound("Extension not found".into()))?;
        let projects = store.list_project_tuples();
        (ext, projects)
    };

    // Phase 2: filesystem / config-file mutation. No DB access here.
    match ext.kind {
        ExtensionKind::Skill => {
            if let Some(loc) = scanner::find_skill_by_id(adapters, id, &ext.agents, &projects) {
                if loc.entry_path.is_dir() {
                    std::fs::remove_dir_all(&loc.entry_path)?;
                } else {
                    std::fs::remove_file(&loc.entry_path)?;
                }
            }
        }
        ExtensionKind::Mcp => {
            for adapter in adapters.iter() {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(config_path) = adapter.mcp_config_path_for(&ext.scope) else {
                    continue;
                };
                for server in adapter.read_mcp_servers_from(&config_path) {
                    let candidate = scanner::stable_id_with_scope_for(
                        &server.name,
                        "mcp",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == id {
                        deployer::remove_mcp_server(
                            &config_path,
                            &server.name,
                            adapter.mcp_format(),
                        )?;
                    }
                }
            }
        }
        ExtensionKind::Hook => {
            for adapter in adapters.iter() {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(config_path) = adapter.hook_config_path_for(&ext.scope) else {
                    continue;
                };
                for hook in adapter.read_hooks_from(&config_path) {
                    let hook_name = format!(
                        "{}:{}:{}",
                        hook.event,
                        hook.matcher.as_deref().unwrap_or("*"),
                        hook.command
                    );
                    let candidate = scanner::stable_id_with_scope_for(
                        &hook_name,
                        "hook",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == id {
                        deployer::remove_hook(
                            &config_path,
                            &hook.event,
                            hook.matcher.as_deref(),
                            &hook.command,
                            adapter.hook_format(),
                        )?;
                    }
                }
            }
        }
        ExtensionKind::Cli => {
            // Child skills/MCPs are deleted separately by their own IDs.
            // This branch only runs for full CLI uninstall (parent record cleanup).
        }
        ExtensionKind::Plugin => {
            for adapter in adapters.iter() {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                for plugin in adapter.read_plugins() {
                    if scanner::stable_id_for(
                        &format!("{}:{}", plugin.name, plugin.source),
                        "plugin",
                        adapter.name(),
                    ) != id
                    {
                        continue;
                    }
                    let plugin_key = if plugin.source.is_empty() {
                        plugin.name.clone()
                    } else {
                        format!("{}@{}", plugin.name, plugin.source)
                    };
                    if adapter.name() == "claude" {
                        let config_path = adapter.plugin_config_path();
                        deployer::remove_plugin_entry(&config_path, &plugin_key)?;
                    } else if adapter.name() == "codex" {
                        // Remove folder + config.toml entry
                        if let Some(ref path) = plugin.path {
                            let target = if let Some(parent) = path.parent() {
                                if parent
                                    .file_name()
                                    .map(|n| n != "cache" && n != "plugins")
                                    .unwrap_or(false)
                                {
                                    parent
                                } else {
                                    path.as_path()
                                }
                            } else {
                                path.as_path()
                            };
                            if target.is_dir() {
                                std::fs::remove_dir_all(target)?;
                            } else if target.is_file() {
                                std::fs::remove_file(target)?;
                            }
                        }
                        deployer::remove_codex_plugin_entry(
                            &adapter.mcp_config_path(),
                            &plugin_key,
                        )?;
                    } else if adapter.name() == "gemini" {
                        if let Some(ref path) = plugin.path
                            && path.is_dir()
                        {
                            std::fs::remove_dir_all(path)?;
                        }
                        deployer::remove_gemini_extension_entry(
                            &adapter.base_dir().join("extensions"),
                            &plugin.name,
                        )?;
                    } else if adapter.name() == "copilot" {
                        if let Some(ref path) = plugin.path
                            && path.is_dir()
                        {
                            std::fs::remove_dir_all(path)?;
                        }
                        if let (Some(uri), Some(vscode_dir)) =
                            (&plugin.uri, adapter.vscode_user_dir())
                        {
                            // Best-effort: VS Code may hold a lock on state.vscdb
                            if let Err(e) =
                                deployer::remove_vscode_plugin_entry(&vscode_dir, uri)
                            {
                                eprintln!(
                                    "Warning: failed to clean up VS Code plugin entry: {e}"
                                );
                            }
                        }
                    } else if let Some(ref path) = plugin.path
                        && path.is_dir()
                    {
                        // Cursor, etc. — just remove folder
                        std::fs::remove_dir_all(path)?;
                    }
                }
            }
        }
    }

    // Phase 3: DB delete, only after disk side succeeded.
    store.lock().delete_extension(id)
}

/// Read the rich on-disk content for an extension (skill text, MCP server
/// config summary, hook detail, plugin README, …). Pure read-only — locks
/// the store only to fetch metadata, then releases before any I/O.
pub fn get_extension_content(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    id: &str,
) -> Result<ExtensionContent, HkError> {
    let (ext, projects) = {
        let store = store.lock();
        let ext = store
            .get_extension(id)?
            .ok_or_else(|| HkError::NotFound("Extension not found".into()))?;
        let projects = store.list_project_tuples();
        (ext, projects)
    };

    match ext.kind {
        ExtensionKind::Skill => {
            if let Some(loc) = scanner::find_skill_by_id(adapters, id, &ext.agents, &projects) {
                let dir = if loc.entry_path.is_dir() {
                    loc.entry_path.to_string_lossy().to_string()
                } else {
                    loc.skill_file
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                };
                // Detect symlink: check entry itself, then parent skill_dir
                let dir_symlink_target = if loc
                    .skill_dir
                    .symlink_metadata()
                    .map(|m| m.is_symlink())
                    .unwrap_or(false)
                {
                    std::fs::read_link(&loc.skill_dir).ok()
                } else {
                    None
                };
                let symlink_target = if loc
                    .entry_path
                    .symlink_metadata()
                    .map(|m| m.is_symlink())
                    .unwrap_or(false)
                {
                    std::fs::read_link(&loc.entry_path)
                        .ok()
                        .map(|t| t.to_string_lossy().to_string())
                } else if let Some(ref resolved_dir) = dir_symlink_target {
                    let entry_name = loc.entry_path.file_name().unwrap_or_default();
                    Some(resolved_dir.join(entry_name).to_string_lossy().to_string())
                } else {
                    None
                };
                let content = std::fs::read_to_string(&loc.skill_file)?;
                Ok(ExtensionContent {
                    content,
                    path: Some(dir),
                    symlink_target,
                })
            } else {
                Err(HkError::NotFound("Skill file not found".into()))
            }
        }
        ExtensionKind::Mcp => {
            // The trait helper resolves the right file per scope; the
            // scanner's `source_path` is the canonical config path for project
            // entries — we prefer it when set.
            let mut fallback_config_path = ext.source_path.clone();
            for adapter in adapters {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(config_path) = adapter.mcp_config_path_for(&ext.scope) else {
                    continue;
                };
                if fallback_config_path.is_none() {
                    fallback_config_path = Some(config_path.to_string_lossy().to_string());
                }
                for server in adapter.read_mcp_servers_from(&config_path) {
                    let candidate = scanner::stable_id_with_scope_for(
                        &server.name,
                        "mcp",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == id {
                        let mut lines = vec![format!("Command: {}", server.command)];
                        if !server.args.is_empty() {
                            lines.push(format!("Args: {}", server.args.join(" ")));
                        }
                        if !server.env.is_empty() {
                            lines.push("Environment:".into());
                            for k in server.env.keys() {
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
            // Disabled MCP: still surface the config path where it lived.
            Ok(ExtensionContent {
                content: ext.description,
                path: fallback_config_path,
                symlink_target: None,
            })
        }
        ExtensionKind::Hook => {
            let mut fallback_config_path = ext.source_path.clone();
            for adapter in adapters {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(config_path) = adapter.hook_config_path_for(&ext.scope) else {
                    continue;
                };
                if fallback_config_path.is_none() {
                    fallback_config_path = Some(config_path.to_string_lossy().to_string());
                }
                for hook in adapter.read_hooks_from(&config_path) {
                    let hook_name = format!(
                        "{}:{}:{}",
                        hook.event,
                        hook.matcher.as_deref().unwrap_or("*"),
                        hook.command
                    );
                    let candidate = scanner::stable_id_with_scope_for(
                        &hook_name,
                        "hook",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == id {
                        let mut lines = vec![format!("Event: {}", hook.event)];
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
                path: fallback_config_path,
                symlink_target: None,
            })
        }
        ExtensionKind::Plugin => {
            for adapter in adapters {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                for plugin in adapter.read_plugins() {
                    if scanner::stable_id_for(
                        &format!("{}:{}", plugin.name, plugin.source),
                        "plugin",
                        adapter.name(),
                    ) == id
                    {
                        let path_str = plugin
                            .path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string());
                        // Try README.md from plugin dir first, then walk up to
                        // find the repo root (for git-cloned plugins where
                        // README sits one or more levels above the manifest).
                        let content = plugin
                            .path
                            .as_ref()
                            .and_then(|p| {
                                for candidate in [p.join("README.md"), p.join("readme.md")] {
                                    if let Ok(text) = std::fs::read_to_string(&candidate) {
                                        return Some(text);
                                    }
                                }
                                let mut dir = p.clone();
                                while dir.pop() {
                                    if dir.join(".git").exists() {
                                        for name in ["README.md", "readme.md"] {
                                            if let Ok(text) =
                                                std::fs::read_to_string(dir.join(name))
                                            {
                                                return Some(text);
                                            }
                                        }
                                        break;
                                    }
                                }
                                None
                            })
                            .unwrap_or(ext.description);
                        return Ok(ExtensionContent {
                            content,
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
        ExtensionKind::Cli => Ok(ExtensionContent {
            content: ext.description,
            path: None,
            symlink_target: None,
        }),
    }
}

/// Cross-agent deploy: copy a Skill / MCP / Hook / CLI from its source agent
/// into `target_agent`. Returns a human-readable identifier of what was
/// deployed (skill name, MCP server name, or `event:command` for hooks) so
/// the UI can show the result. The wrapper is responsible for any post-deploy
/// rescan/sync (web does this; desktop does not, matching prior behavior).
pub fn install_to_agent(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    extension_id: &str,
    target_agent: &str,
) -> Result<String, HkError> {
    let (ext, projects) = {
        let store = store.lock();
        let ext = store
            .get_extension(extension_id)?
            .ok_or_else(|| HkError::NotFound("Extension not found".into()))?;
        let projects = store.list_project_tuples();
        (ext, projects)
    };

    let target_adapter = adapters
        .iter()
        .find(|a| a.name() == target_agent)
        .ok_or_else(|| HkError::NotFound(format!("Agent '{}' not found", target_agent)))?;

    match ext.kind {
        ExtensionKind::Skill => {
            let source_path =
                scanner::find_skill_by_id(adapters, extension_id, &ext.agents, &projects)
                    .map(|loc| loc.entry_path)
                    .ok_or_else(|| {
                        HkError::Internal("Could not find source skill files".into())
                    })?;
            let target_dir = target_adapter
                .skill_dirs()
                .into_iter()
                .next()
                .ok_or_else(|| {
                    HkError::Internal(format!(
                        "No skill directory for agent '{}'",
                        target_agent
                    ))
                })?;
            let deployed_name = deployer::deploy_skill(&source_path, &target_dir)?;

            // Propagate install_meta from source to the new target row so
            // cross-agent deploys produce consistent provenance. Without
            // this, only the agent that originally received the marketplace
            // install carries install_meta, and dedup downstream sees the
            // group as split. Hand-managed (no install_meta) sources just
            // skip the write — target stays unlinked, which is correct.
            //
            // We must scan-and-sync the target adapter first so the new row
            // exists in the DB before set_install_meta can update it.
            // Cross-agent deploy targets are global-only today, so the
            // target_id derives from the unscoped stable_id.
            if let Some(meta) = ext.install_meta.clone() {
                let scanned = scanner::scan_adapter(target_adapter.as_ref());
                let target_id =
                    scanner::stable_id_for(&deployed_name, "skill", target_agent);
                let store_guard = store.lock();
                store_guard.sync_extensions_for_agent(target_agent, &scanned)?;
                let _ = store_guard.set_install_meta(&target_id, &meta);
            }
            Ok(deployed_name)
        }
        ExtensionKind::Mcp => {
            let mut source_entry = None;
            for adapter in adapters.iter() {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(source_path) = adapter.mcp_config_path_for(&ext.scope) else {
                    continue;
                };
                for server in adapter.read_mcp_servers_from(&source_path) {
                    let candidate = scanner::stable_id_with_scope_for(
                        &server.name,
                        "mcp",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == extension_id {
                        source_entry = Some(server);
                        break;
                    }
                }
                if source_entry.is_some() {
                    break;
                }
            }
            let mut entry = source_entry.ok_or_else(|| {
                HkError::Internal("Could not find source MCP server config".into())
            })?;
            if target_adapter.needs_path_injection() {
                deployer::ensure_path_injection(&mut entry);
            }
            let config_path = target_adapter.mcp_config_path();
            deployer::deploy_mcp_server(&config_path, &entry, target_adapter.mcp_format())?;
            Ok(entry.name)
        }
        ExtensionKind::Hook => {
            let mut source_entry = None;
            for adapter in adapters.iter() {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(source_path) = adapter.hook_config_path_for(&ext.scope) else {
                    continue;
                };
                for hook in adapter.read_hooks_from(&source_path) {
                    let hook_name = format!(
                        "{}:{}:{}",
                        hook.event,
                        hook.matcher.as_deref().unwrap_or("*"),
                        hook.command
                    );
                    let candidate = scanner::stable_id_with_scope_for(
                        &hook_name,
                        "hook",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == extension_id {
                        source_entry = Some(hook);
                        break;
                    }
                }
                if source_entry.is_some() {
                    break;
                }
            }
            let mut entry = source_entry
                .ok_or_else(|| HkError::Internal("Could not find source hook config".into()))?;

            // Translate event name to the target agent's convention. Agents
            // disagree on hook event names (Claude `PreToolUse` vs Codex
            // `pre_tool_use`, etc.) so a missing translation is a hard error.
            let translated_event = target_adapter
                .translate_hook_event(&entry.event)
                .ok_or_else(|| {
                    HkError::Internal(format!(
                        "Hook event '{}' is not supported by {}",
                        entry.event, target_agent
                    ))
                })?;
            entry.event = translated_event;

            let config_path = target_adapter.hook_config_path();
            deployer::deploy_hook(&config_path, &entry, target_adapter.hook_format())?;

            // Codex requires hooks feature enabled in config.toml
            if target_adapter.name() == "codex"
                && let Err(e) = deployer::ensure_codex_hooks_enabled(&target_adapter.base_dir())
            {
                eprintln!("[hk] warning: {e}");
            }

            Ok(format!("{}:{}", entry.event, entry.command))
        }
        ExtensionKind::Cli => {
            // Deploy the CLI's associated skill to the target agent.
            let binary_name = ext
                .cli_meta
                .as_ref()
                .map(|m| m.binary_name.clone())
                .unwrap_or_else(|| ext.name.to_lowercase());
            // CLI source skills are global-only today, but search every scope
            // so a future project-scoped CLI skill can still seed install_to_agent.
            let locations = scanner::skill_locations(&binary_name, adapters, &projects, None);
            let source_path = locations
                .into_iter()
                .next()
                .map(|(_, path)| path)
                .ok_or_else(|| {
                    HkError::Internal("Could not find source skill files for CLI".into())
                })?;
            let target_dir = target_adapter
                .skill_dirs()
                .into_iter()
                .next()
                .ok_or_else(|| {
                    HkError::Internal(format!(
                        "No skill directory for agent '{}'",
                        target_agent
                    ))
                })?;
            let deployed_name = deployer::deploy_skill(&source_path, &target_dir)?;
            Ok(deployed_name)
        }
        other => Err(HkError::Internal(format!(
            "Cross-agent deploy not supported for '{}' extensions",
            other.as_str()
        ))),
    }
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
            &ConfigScope::Global,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    /// Project-scope post_install_sync must scan the project directory, upsert
    /// the project row, and write install_meta to the project-scoped row id —
    /// not the unscoped (global) one.
    #[test]
    fn test_post_install_sync_writes_install_meta_to_project_scoped_row() {
        use crate::adapter;

        let dir = TempDir::new().unwrap();
        let proj_dir = TempDir::new().unwrap();
        let home = dir.path();
        let store = Store::open(&home.join("test.db")).unwrap();

        // Project-scope skill on disk (matches Claude's project_skill_dirs())
        let proj_path = proj_dir.path().to_string_lossy().to_string();
        let skills_dir = proj_dir.path().join(".claude").join("skills").join("foo");
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("SKILL.md"), "---\nname: foo\n---\n").unwrap();

        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![
            Box::new(adapter::claude::ClaudeAdapter::with_home(home.to_path_buf())),
        ];

        let target_scope = ConfigScope::Project {
            name: "demo".into(),
            path: proj_path.clone(),
        };
        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some("https://github.com/foo/bar".into()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: None,
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };

        post_install_sync(
            &store,
            &adapters,
            &["claude".into()],
            "foo",
            Some(meta.clone()),
            None,
            &target_scope,
        )
        .unwrap();

        // Assert: install_meta lands on the project-scoped row
        let project_id =
            scanner::stable_id_with_scope_for("foo", "skill", "claude", &target_scope);
        let ext = store
            .get_extension(&project_id)
            .unwrap()
            .expect("project-scoped row should exist after sync");
        assert_eq!(
            ext.install_meta
                .as_ref()
                .expect("install_meta should be set")
                .url,
            meta.url,
        );

        // And: no global row got bogus meta
        let global_id = scanner::stable_id_for("foo", "skill", "claude");
        let global = store.get_extension(&global_id).unwrap();
        assert!(
            global.is_none() || global.unwrap().install_meta.is_none(),
            "global row should not exist or should not have install_meta",
        );
    }

    #[test]
    fn test_run_full_audit_empty_store() {
        let (store, _dir) = test_store();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![];
        let result = run_full_audit(&store, &adapters);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_read_plugin_content_reads_js_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.js"), "eval(user_input)").unwrap();
        std::fs::write(tmp.path().join("readme.md"), "# Hello").unwrap(); // should be skipped
        let content = read_plugin_content(&tmp.path().to_string_lossy());
        assert!(content.contains("eval(user_input)"));
        assert!(!content.contains("# Hello"));
    }

    #[test]
    fn test_read_plugin_content_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let content = read_plugin_content(&tmp.path().to_string_lossy());
        assert!(content.is_empty());
    }

    /// Cross-agent skill deploy must propagate the source's install_meta to
    /// the new target row. Otherwise dedup later splits a logically-single
    /// marketplace skill across agents that have inconsistent install_meta.
    #[test]
    fn test_install_to_agent_propagates_install_meta() {
        use crate::adapter;

        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let store_raw = Store::open(&home.join("test.db")).unwrap();
        let store = Mutex::new(store_raw);

        // Source: a Claude global skill installed from a marketplace.
        std::fs::create_dir_all(home.join(".claude").join("skills").join("foo")).unwrap();
        std::fs::write(
            home.join(".claude").join("skills").join("foo").join("SKILL.md"),
            "---\nname: foo\n---\n",
        )
        .unwrap();

        // Codex must detect (`<home>/.codex/` exists) so scan_adapter picks
        // up the deployed copy.
        std::fs::create_dir_all(home.join(".codex")).unwrap();

        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![
            Box::new(adapter::claude::ClaudeAdapter::with_home(home.to_path_buf())),
            Box::new(adapter::codex::CodexAdapter::with_home(home.to_path_buf())),
        ];

        let source_id = scanner::stable_id_for("foo", "skill", "claude");
        let install_meta = InstallMeta {
            install_type: "marketplace".into(),
            url: Some("https://github.com/foo/bar/foo".into()),
            url_resolved: Some("https://github.com/foo/bar.git".into()),
            branch: None,
            subpath: Some("foo".into()),
            revision: Some("abc123".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        let source_ext = Extension {
            id: source_id.clone(),
            kind: ExtensionKind::Skill,
            name: "foo".into(),
            description: String::new(),
            source: Source {
                origin: SourceOrigin::Agent,
                url: None,
                version: None,
                commit_hash: None,
            },
            agents: vec!["claude".into()],
            tags: vec![],
            pack: None,
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            source_path: Some(
                home.join(".claude")
                    .join("skills")
                    .join("foo")
                    .join("SKILL.md")
                    .to_string_lossy()
                    .to_string(),
            ),
            cli_parent_id: None,
            cli_meta: None,
            install_meta: Some(install_meta.clone()),
            scope: ConfigScope::Global,
        };
        store.lock().insert_extension(&source_ext).unwrap();

        // Cross-agent deploy: claude/foo → codex.
        install_to_agent(&store, &adapters, &source_id, "codex").unwrap();

        // File deployed to codex skill dir.
        let target_skill_md = home
            .join(".codex")
            .join("skills")
            .join("foo")
            .join("SKILL.md");
        assert!(target_skill_md.exists(), "deploy_skill should write target SKILL.md");

        // Target row carries the same install_meta as source — the whole
        // point of this test.
        let target_id = scanner::stable_id_for("foo", "skill", "codex");
        let target = store.lock().get_extension(&target_id).unwrap().unwrap();
        let target_meta = target
            .install_meta
            .expect("target row should have install_meta propagated from source");
        assert_eq!(target_meta.install_type, install_meta.install_type);
        assert_eq!(target_meta.url, install_meta.url);
        assert_eq!(target_meta.url_resolved, install_meta.url_resolved);
        assert_eq!(target_meta.subpath, install_meta.subpath);
        assert_eq!(target_meta.revision, install_meta.revision);
    }

    /// When the source skill has no install_meta (hand-managed), deploying
    /// to another agent must NOT fabricate one — target stays unlinked,
    /// matching the source's provenance.
    #[test]
    fn test_install_to_agent_skips_when_source_has_no_install_meta() {
        use crate::adapter;

        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let store_raw = Store::open(&home.join("test.db")).unwrap();
        let store = Mutex::new(store_raw);

        std::fs::create_dir_all(home.join(".claude").join("skills").join("bar")).unwrap();
        std::fs::write(
            home.join(".claude").join("skills").join("bar").join("SKILL.md"),
            "---\nname: bar\n---\n",
        )
        .unwrap();
        std::fs::create_dir_all(home.join(".codex")).unwrap();

        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![
            Box::new(adapter::claude::ClaudeAdapter::with_home(home.to_path_buf())),
            Box::new(adapter::codex::CodexAdapter::with_home(home.to_path_buf())),
        ];

        let source_id = scanner::stable_id_for("bar", "skill", "claude");
        let source_ext = Extension {
            id: source_id.clone(),
            kind: ExtensionKind::Skill,
            name: "bar".into(),
            description: String::new(),
            source: Source {
                origin: SourceOrigin::Agent,
                url: None,
                version: None,
                commit_hash: None,
            },
            agents: vec!["claude".into()],
            tags: vec![],
            pack: None,
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            source_path: Some(
                home.join(".claude")
                    .join("skills")
                    .join("bar")
                    .join("SKILL.md")
                    .to_string_lossy()
                    .to_string(),
            ),
            cli_parent_id: None,
            cli_meta: None,
            install_meta: None,
            scope: ConfigScope::Global,
        };
        store.lock().insert_extension(&source_ext).unwrap();

        install_to_agent(&store, &adapters, &source_id, "codex").unwrap();

        // No install_meta to propagate — target row may not even exist in
        // the DB yet (we only sync target when there's meta to write). The
        // file is on disk; that's enough.
        let target_skill_md = home
            .join(".codex")
            .join("skills")
            .join("bar")
            .join("SKILL.md");
        assert!(target_skill_md.exists());

        // If a row happens to be there from a previous flow, it must NOT
        // have install_meta fabricated.
        let target_id = scanner::stable_id_for("bar", "skill", "codex");
        if let Some(row) = store.lock().get_extension(&target_id).unwrap() {
            assert!(
                row.install_meta.is_none(),
                "must not synthesize install_meta when source had none"
            );
        }
    }
}
