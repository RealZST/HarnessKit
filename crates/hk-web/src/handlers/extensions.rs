use std::path::PathBuf;

use axum::extract::State;
use axum::Json;
use hk_core::adapter;
use hk_core::models::{Extension, ExtensionKind};
use hk_core::{manager, scanner};
use serde::Deserialize;

use crate::router::{blocking, ApiError};
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

// --- Skill location helper (mirrors hk-desktop/commands/helpers.rs) ---

pub(crate) struct SkillLocation {
    pub(crate) entry_path: PathBuf,
    pub(crate) skill_file: PathBuf,
    pub(crate) skill_dir: PathBuf,
}

pub(crate) fn find_skill_by_id(
    adapters: &[Box<dyn adapter::AgentAdapter>],
    ext_id: &str,
    agent_filter: &[String],
) -> Option<SkillLocation> {
    for a in adapters {
        if !agent_filter.contains(&a.name().to_string()) {
            continue;
        }
        for skill_dir in a.skill_dirs() {
            let Ok(entries) = std::fs::read_dir(&skill_dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                let skill_file = if path.is_dir() {
                    let md = path.join("SKILL.md");
                    if md.exists() { md } else { path.join("SKILL.md.disabled") }
                } else if path.extension().is_some_and(|e| e == "md" || e == "disabled") {
                    path.clone()
                } else {
                    continue;
                };
                if !skill_file.exists() { continue; }
                let name = scanner::parse_skill_name(&skill_file).unwrap_or_else(|| {
                    path.file_stem().unwrap_or_default().to_string_lossy().to_string()
                });
                if scanner::stable_id_for(&name, "skill", a.name()) == ext_id {
                    return Some(SkillLocation { entry_path: path, skill_file, skill_dir: skill_dir.clone() });
                }
            }
        }
    }
    None
}

#[derive(Deserialize, Default)]
pub struct ListParams {
    pub kind: Option<String>,
    pub agent: Option<String>,
}

pub async fn list_extensions(
    State(state): State<WebState>,
    Json(params): Json<ListParams>,
) -> Result<Vec<Extension>> {
    blocking(move || {
        let store = state.store.lock();
        let kind = params.kind.as_deref().and_then(|s| s.parse::<ExtensionKind>().ok());
        store.list_extensions(kind, params.agent.as_deref())
    }).await
}

#[derive(Deserialize)]
pub struct ToggleParams {
    pub id: String,
    pub enabled: bool,
}

pub async fn toggle_extension(
    State(state): State<WebState>,
    Json(params): Json<ToggleParams>,
) -> Result<()> {
    blocking(move || {
        let store = state.store.lock();
        manager::toggle_extension_with_adapters(
            &store,
            &state.adapters,
            &params.id,
            params.enabled,
        )?;
        Ok(())
    }).await
}

#[derive(Deserialize)]
pub struct IdParams {
    pub id: String,
}

pub async fn get_extension_content(
    State(state): State<WebState>,
    Json(params): Json<IdParams>,
) -> Result<ExtensionContentResponse> {
    blocking(move || {
        let id = &params.id;
        let store = state.store.lock();
        let ext = store.get_extension(id)?
            .ok_or_else(|| hk_core::HkError::NotFound("Extension not found".into()))?;
        drop(store); // release lock before I/O

        let adapters = &*state.adapters;
        match ext.kind {
            ExtensionKind::Skill => {
                // Scan adapters to find the skill on disk (same as desktop)
                if let Some(loc) = find_skill_by_id(adapters, id, &ext.agents) {
                    let dir = if loc.entry_path.is_dir() {
                        loc.entry_path.to_string_lossy().to_string()
                    } else {
                        loc.skill_file.parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default()
                    };
                    // Detect symlink: check entry itself, then parent skill_dir
                    let dir_symlink_target = if loc.skill_dir
                        .symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false)
                    {
                        std::fs::read_link(&loc.skill_dir).ok()
                    } else {
                        None
                    };
                    let symlink_target = if loc.entry_path
                        .symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false)
                    {
                        std::fs::read_link(&loc.entry_path).ok()
                            .map(|t| t.to_string_lossy().to_string())
                    } else if let Some(ref resolved_dir) = dir_symlink_target {
                        let entry_name = loc.entry_path.file_name().unwrap_or_default();
                        Some(resolved_dir.join(entry_name).to_string_lossy().to_string())
                    } else {
                        None
                    };
                    let content = std::fs::read_to_string(&loc.skill_file)?;
                    Ok(ExtensionContentResponse { content, path: Some(dir), symlink_target })
                } else {
                    Err(hk_core::HkError::NotFound("Skill file not found".into()))
                }
            }
            ExtensionKind::Mcp => {
                // Read MCP server config from adapter — shows command/args/env
                let mut fallback_config_path = None;
                for adapter in adapters {
                    if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                    let config_path = adapter.mcp_config_path();
                    if fallback_config_path.is_none() {
                        fallback_config_path = Some(config_path.to_string_lossy().to_string());
                    }
                    for server in adapter.read_mcp_servers() {
                        if scanner::stable_id_for(&server.name, "mcp", adapter.name()) == *id {
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
                            return Ok(ExtensionContentResponse {
                                content: lines.join("\n"),
                                path: Some(config_path.to_string_lossy().to_string()),
                                symlink_target: None,
                            });
                        }
                    }
                }
                Ok(ExtensionContentResponse {
                    content: ext.description,
                    path: fallback_config_path,
                    symlink_target: None,
                })
            }
            ExtensionKind::Hook => {
                // Read hook config — shows event/matcher/command
                let mut fallback_config_path = None;
                for adapter in adapters {
                    if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                    let config_path = adapter.hook_config_path();
                    if fallback_config_path.is_none() {
                        fallback_config_path = Some(config_path.to_string_lossy().to_string());
                    }
                    for hook in adapter.read_hooks() {
                        let hook_name = format!("{}:{}:{}", hook.event, hook.matcher.as_deref().unwrap_or("*"), hook.command);
                        if scanner::stable_id_for(&hook_name, "hook", adapter.name()) == *id {
                            let mut lines = vec![format!("Event: {}", hook.event)];
                            if let Some(m) = &hook.matcher { lines.push(format!("Matcher: {}", m)); }
                            lines.push(format!("Command: {}", hook.command));
                            return Ok(ExtensionContentResponse {
                                content: lines.join("\n"),
                                path: Some(config_path.to_string_lossy().to_string()),
                                symlink_target: None,
                            });
                        }
                    }
                }
                Ok(ExtensionContentResponse {
                    content: ext.description,
                    path: fallback_config_path,
                    symlink_target: None,
                })
            }
            ExtensionKind::Plugin => {
                // Read README.md from plugin directory
                for adapter in adapters {
                    if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                    for plugin in adapter.read_plugins() {
                        if scanner::stable_id_for(&format!("{}:{}", plugin.name, plugin.source), "plugin", adapter.name()) == *id {
                            let path_str = plugin.path.as_ref().map(|p| p.to_string_lossy().to_string());
                            let content = plugin.path.as_ref()
                                .and_then(|p| {
                                    for candidate in [p.join("README.md"), p.join("readme.md")] {
                                        if let Ok(text) = std::fs::read_to_string(&candidate) { return Some(text); }
                                    }
                                    let mut dir = p.clone();
                                    while dir.pop() {
                                        if dir.join(".git").exists() {
                                            for name in ["README.md", "readme.md"] {
                                                if let Ok(text) = std::fs::read_to_string(dir.join(name)) { return Some(text); }
                                            }
                                            break;
                                        }
                                    }
                                    None
                                })
                                .unwrap_or(ext.description.clone());
                            return Ok(ExtensionContentResponse { content, path: path_str, symlink_target: None });
                        }
                    }
                }
                Ok(ExtensionContentResponse { content: ext.description, path: None, symlink_target: None })
            }
            ExtensionKind::Cli => {
                Ok(ExtensionContentResponse { content: ext.description, path: None, symlink_target: None })
            }
        }
    }).await
}

#[derive(serde::Serialize)]
pub struct ExtensionContentResponse {
    pub content: String,
    pub path: Option<String>,
    pub symlink_target: Option<String>,
}

pub async fn delete_extension(
    State(state): State<WebState>,
    Json(params): Json<IdParams>,
) -> Result<()> {
    blocking(move || {
        use hk_core::deployer;
        let id = &params.id;
        let ext = {
            let store = state.store.lock();
            store.get_extension(id)?
                .ok_or_else(|| hk_core::HkError::NotFound("Extension not found".into()))?
        };
        let adapters = &*state.adapters;

        match ext.kind {
            ExtensionKind::Skill => {
                if let Some(loc) = find_skill_by_id(adapters, id, &ext.agents) {
                    if loc.entry_path.is_dir() {
                        std::fs::remove_dir_all(&loc.entry_path)?;
                    } else {
                        std::fs::remove_file(&loc.entry_path)?;
                    }
                }
            }
            ExtensionKind::Mcp => {
                for adapter in adapters.iter() {
                    if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                    for server in adapter.read_mcp_servers() {
                        if scanner::stable_id_for(&server.name, "mcp", adapter.name()) == *id {
                            let config_path = adapter.mcp_config_path();
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
                    if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                    for hook in adapter.read_hooks() {
                        let hook_name = format!(
                            "{}:{}:{}",
                            hook.event,
                            hook.matcher.as_deref().unwrap_or("*"),
                            hook.command
                        );
                        if scanner::stable_id_for(&hook_name, "hook", adapter.name()) == *id {
                            let config_path = adapter.hook_config_path();
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
            ExtensionKind::Plugin => {
                for adapter in adapters.iter() {
                    if !ext.agents.contains(&adapter.name().to_string()) { continue; }
                    for plugin in adapter.read_plugins() {
                        if scanner::stable_id_for(
                            &format!("{}:{}", plugin.name, plugin.source),
                            "plugin",
                            adapter.name(),
                        ) != *id { continue; }
                        let plugin_key = if plugin.source.is_empty() {
                            plugin.name.clone()
                        } else {
                            format!("{}@{}", plugin.name, plugin.source)
                        };
                        if adapter.name() == "claude" {
                            let config_path = adapter.plugin_config_path();
                            deployer::remove_plugin_entry(&config_path, &plugin_key)?;
                        } else if adapter.name() == "codex" {
                            if let Some(ref path) = plugin.path {
                                let target = if let Some(parent) = path.parent() {
                                    if parent.file_name().map(|n| n != "cache" && n != "plugins").unwrap_or(false) {
                                        parent
                                    } else { path.as_path() }
                                } else { path.as_path() };
                                if target.is_dir() { std::fs::remove_dir_all(target)?; }
                                else if target.is_file() { std::fs::remove_file(target)?; }
                            }
                            deployer::remove_codex_plugin_entry(&adapter.mcp_config_path(), &plugin_key)?;
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
                            if let (Some(uri), Some(vscode_dir)) = (&plugin.uri, adapter.vscode_user_dir())
                                && let Err(e) = deployer::remove_vscode_plugin_entry(&vscode_dir, uri)
                            {
                                eprintln!("Warning: failed to clean up VS Code plugin entry: {e}");
                            }
                        } else if let Some(ref path) = plugin.path
                            && path.is_dir()
                        {
                            std::fs::remove_dir_all(path)?;
                        }
                    }
                }
            }
            ExtensionKind::Cli => {}
        }

        let store = state.store.lock();
        store.delete_extension(id)?;
        Ok(())
    }).await
}

#[derive(Deserialize)]
pub struct UninstallCliBinaryParams {
    pub binary_path: String,
}

pub async fn uninstall_cli_binary(
    State(_state): State<WebState>,
    Json(params): Json<UninstallCliBinaryParams>,
) -> Result<()> {
    blocking(move || {
        let path = std::path::Path::new(&params.binary_path);
        if path.exists() && path.is_file() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }).await
}

pub async fn scan_and_sync(
    State(state): State<WebState>,
) -> std::result::Result<Json<usize>, ApiError> {
    let state_bg = state.clone();

    // Phase 1+2: Scan filesystem and sync to DB
    let (count, unlinked) = tokio::task::spawn_blocking(move || {
        let store = state.store.lock();
        let projects: Vec<(String, String)> = store
            .list_projects()
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.name, p.path))
            .collect();
        let extensions = scanner::scan_all(&state.adapters, &projects);
        let count = extensions.len();

        let pre_ids: std::collections::HashSet<String> = store
            .list_extensions(Some(ExtensionKind::Skill), None)
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.id)
            .collect();

        store.sync_extensions(&extensions)?;

        let unlinked: Vec<(String, String)> = store
            .list_extensions(Some(ExtensionKind::Skill), None)?
            .into_iter()
            .filter(|e| e.install_meta.is_none() && !pre_ids.contains(&e.id))
            .map(|e| (e.id, e.name))
            .collect();

        store.run_backfill_packs()?;
        Ok::<_, hk_core::HkError>((count, unlinked))
    })
    .await
    .map_err(|e| ApiError::from(hk_core::HkError::Internal(e.to_string())))??;

    // Phase 3+4: Marketplace matching in background (no Tauri event, but data updates)
    if !unlinked.is_empty() {
        let store = state_bg.store;
        tokio::task::spawn_blocking(move || {
            let unique_names: std::collections::HashSet<String> =
                unlinked.iter().map(|(_, n)| n.clone()).collect();
            let mut matched = std::collections::HashMap::<String, (String, String, Option<String>)>::new();
            for name in &unique_names {
                if let Ok(results) = hk_core::marketplace::search_skills(name, 5) {
                    let exact: Vec<_> = results.iter().filter(|r| r.name.eq_ignore_ascii_case(name)).collect();
                    if exact.len() == 1 {
                        let item = exact[0];
                        let git_url = hk_core::marketplace::git_url_for_source(&item.source);
                        let remote_rev = hk_core::manager::get_remote_head(&git_url).ok();
                        matched.insert(name.to_string(), (git_url, item.skill_id.clone(), remote_rev));
                    }
                }
            }
            if !matched.is_empty() {
                let store = store.lock();
                let now = chrono::Utc::now();
                for (id, name) in &unlinked {
                    if let Some((git_url, skill_id, remote_rev)) = matched.get(name.as_str()) {
                        let meta = hk_core::models::InstallMeta {
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
                        let _ = store.set_install_meta(id, &meta);
                    }
                }
                let _ = store.run_backfill_packs();
            }
        });
    }

    Ok(Json(count))
}

pub async fn list_skill_files(
    State(state): State<WebState>,
    Json(params): Json<ListSkillFilesParams>,
) -> Result<Vec<FileEntry>> {
    blocking(move || {
        let path = std::path::Path::new(&params.path);
        if !path.exists() || !path.is_dir() {
            return Err(hk_core::HkError::NotFound("Directory not found".into()));
        }
        // Validate path is within an allowed agent directory
        let canonical = path.canonicalize()
            .map_err(|_| hk_core::HkError::NotFound("Cannot resolve path".into()))?;
        let normalized = super::normalize(&canonical);
        let allowed = state.adapters.iter().any(|a| {
            a.skill_dirs().iter().any(|d| normalized.starts_with(d))
                || normalized.starts_with(a.base_dir())
        });
        if !allowed {
            let store = state.store.lock();
            if !super::is_path_allowed(&canonical, &store) {
                return Err(hk_core::HkError::PathNotAllowed(
                    "Path is not within a known agent directory".into(),
                ));
            }
        }
        Ok(list_dir_entries(path, 0))
    }).await
}

#[derive(Deserialize)]
pub struct ListSkillFilesParams {
    pub path: String,
}

#[derive(serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileEntry>>,
}

fn list_dir_entries(dir: &std::path::Path, depth: u8) -> Vec<FileEntry> {
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(dir) else { return entries };
    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let children = if is_dir && depth < 1 {
            Some(list_dir_entries(&path, depth + 1))
        } else {
            if is_dir { Some(Vec::new()) } else { None }
        };
        entries.push(FileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
            children,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
    });
    entries
}
