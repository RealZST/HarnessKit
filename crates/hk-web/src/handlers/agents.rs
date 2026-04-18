use axum::extract::State;
use axum::Json;
use hk_core::models::{AgentDetail, AgentInfo, ExtensionCounts, ExtensionKind, AgentConfigFile, ConfigCategory, ConfigScope};
use hk_core::scanner;
use serde::Deserialize;

use crate::router::ApiError;
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

pub async fn list_agents(
    State(state): State<WebState>,
) -> Result<Vec<AgentInfo>> {
    let store = state.store.lock();
    let db_order = store.get_agent_order().unwrap_or_default();
    let order_map: std::collections::HashMap<String, i32> = db_order.into_iter().collect();

    let mut result = Vec::new();
    for a in state.adapters.iter() {
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
    result.sort_by_key(|a| *order_map.get(&a.name).unwrap_or(&999));
    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct SetAgentEnabledParams {
    pub name: String,
    pub enabled: bool,
}

pub async fn set_agent_enabled(
    State(state): State<WebState>,
    Json(params): Json<SetAgentEnabledParams>,
) -> Result<()> {
    let store = state.store.lock();
    store.set_agent_enabled(&params.name, params.enabled)?;
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct UpdateAgentOrderParams {
    pub names: Vec<String>,
}

pub async fn update_agent_order(
    State(state): State<WebState>,
    Json(params): Json<UpdateAgentOrderParams>,
) -> Result<()> {
    let store = state.store.lock();
    store.set_agent_order(&params.names)?;
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct UpdateAgentPathParams {
    pub name: String,
    pub path: Option<String>,
}

pub async fn update_agent_path(
    State(state): State<WebState>,
    Json(params): Json<UpdateAgentPathParams>,
) -> Result<()> {
    let store = state.store.lock();
    store.set_agent_path(&params.name, params.path.as_deref())?;
    Ok(Json(()))
}

pub async fn list_agent_configs(
    State(state): State<WebState>,
) -> Result<Vec<AgentDetail>> {
    let store = state.store.lock();
    let projects: Vec<(String, String)> = store
        .list_projects()
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.name, p.path))
        .collect();

    let mut results = Vec::new();
    for a in state.adapters.iter() {
        let detected = a.detect();
        let mut config_files = if detected {
            scanner::scan_agent_configs(a.as_ref(), &projects)
        } else {
            vec![]
        };

        let existing_paths: std::collections::HashSet<String> = config_files
            .iter()
            .filter_map(|f| std::path::Path::new(&f.path).canonicalize().ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        if let Ok(custom_paths) = store.list_custom_config_paths(a.name()) {
            for (id, path, label, category_str) in custom_paths {
                let canonical = std::path::Path::new(&path)
                    .canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.clone());
                if existing_paths.contains(&canonical) {
                    continue;
                }
                let category = match category_str.as_str() {
                    "rules" => ConfigCategory::Rules,
                    "memory" => ConfigCategory::Memory,
                    "ignore" => ConfigCategory::Ignore,
                    _ => ConfigCategory::Settings,
                };
                let p = std::path::Path::new(&path);
                let (size_bytes, modified_at, is_dir, exists) =
                    if let Ok(meta) = std::fs::metadata(p) {
                        let modified = meta.modified().ok().map(|t| {
                            let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                            chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                                .unwrap_or_default()
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
                    file_name: p.file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.clone()),
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
    Ok(Json(results))
}

#[derive(Deserialize)]
pub struct AddCustomConfigPathParams {
    pub agent: String,
    pub path: String,
    pub label: String,
    pub category: String,
}

pub async fn add_custom_config_path(
    State(state): State<WebState>,
    Json(params): Json<AddCustomConfigPathParams>,
) -> Result<i64> {
    let store = state.store.lock();
    let id = store.add_custom_config_path(&params.agent, &params.path, &params.label, &params.category)?;
    Ok(Json(id))
}

#[derive(Deserialize)]
pub struct UpdateCustomConfigPathParams {
    pub id: i64,
    pub path: String,
    pub label: String,
    pub category: String,
}

pub async fn update_custom_config_path(
    State(state): State<WebState>,
    Json(params): Json<UpdateCustomConfigPathParams>,
) -> Result<()> {
    let store = state.store.lock();
    store.update_custom_config_path(params.id, &params.path, &params.label, &params.category)?;
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct RemoveCustomConfigPathParams {
    pub id: i64,
}

pub async fn remove_custom_config_path(
    State(state): State<WebState>,
    Json(params): Json<RemoveCustomConfigPathParams>,
) -> Result<()> {
    let store = state.store.lock();
    store.remove_custom_config_path(params.id)?;
    Ok(Json(()))
}
