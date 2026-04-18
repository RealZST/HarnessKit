use axum::extract::State;
use axum::Json;
use hk_core::models::DashboardStats;
use hk_core::manager;
use serde::Deserialize;

use crate::router::ApiError;
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

pub async fn get_dashboard_stats(
    State(state): State<WebState>,
) -> Result<DashboardStats> {
    let store = state.store.lock();
    let exts = store.list_extensions(None, None)?;
    let severity_map = store.count_latest_findings_by_severity()?;
    let stats = DashboardStats {
        total_extensions: exts.len(),
        skill_count: exts.iter().filter(|e| e.kind == hk_core::models::ExtensionKind::Skill).count(),
        mcp_count: exts.iter().filter(|e| e.kind == hk_core::models::ExtensionKind::Mcp).count(),
        plugin_count: exts.iter().filter(|e| e.kind == hk_core::models::ExtensionKind::Plugin).count(),
        hook_count: exts.iter().filter(|e| e.kind == hk_core::models::ExtensionKind::Hook).count(),
        cli_count: exts.iter().filter(|e| e.kind == hk_core::models::ExtensionKind::Cli).count(),
        critical_issues: severity_map.get("critical").copied().unwrap_or(0),
        high_issues: severity_map.get("high").copied().unwrap_or(0),
        medium_issues: severity_map.get("medium").copied().unwrap_or(0),
        low_issues: severity_map.get("low").copied().unwrap_or(0),
        updates_available: 0,
    };
    Ok(Json(stats))
}

#[derive(Deserialize)]
pub struct UpdateTagsParams {
    pub id: String,
    pub tags: Vec<String>,
}

pub async fn update_tags(
    State(state): State<WebState>,
    Json(params): Json<UpdateTagsParams>,
) -> Result<()> {
    let store = state.store.lock();
    store.update_tags(&params.id, &params.tags)?;
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct BatchUpdateTagsParams {
    pub ids: Vec<String>,
    pub tags: Vec<String>,
}

pub async fn batch_update_tags(
    State(state): State<WebState>,
    Json(params): Json<BatchUpdateTagsParams>,
) -> Result<()> {
    let store = state.store.lock();
    store.batch_update_tags(&params.ids, &params.tags)?;
    Ok(Json(()))
}

pub async fn get_all_tags(
    State(state): State<WebState>,
) -> Result<Vec<String>> {
    let store = state.store.lock();
    let tags = store.get_all_tags()?;
    Ok(Json(tags))
}

#[derive(Deserialize)]
pub struct UpdatePackParams {
    pub id: String,
    pub pack: Option<String>,
}

pub async fn update_pack(
    State(state): State<WebState>,
    Json(params): Json<UpdatePackParams>,
) -> Result<()> {
    let store = state.store.lock();
    store.update_pack(&params.id, params.pack.as_deref())?;
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct BatchUpdatePackParams {
    pub ids: Vec<String>,
    pub pack: Option<String>,
}

pub async fn batch_update_pack(
    State(state): State<WebState>,
    Json(params): Json<BatchUpdatePackParams>,
) -> Result<()> {
    let store = state.store.lock();
    store.batch_update_pack(&params.ids, params.pack.as_deref())?;
    Ok(Json(()))
}

pub async fn get_all_packs(
    State(state): State<WebState>,
) -> Result<Vec<String>> {
    let store = state.store.lock();
    let packs = store.get_all_packs()?;
    Ok(Json(packs))
}

#[derive(Deserialize)]
pub struct ToggleByPackParams {
    pub pack: String,
    pub enabled: bool,
}

pub async fn toggle_by_pack(
    State(state): State<WebState>,
    Json(params): Json<ToggleByPackParams>,
) -> Result<Vec<String>> {
    let store = state.store.lock();
    let ids = store.find_ids_by_pack(&params.pack)?;
    for id in &ids {
        manager::toggle_extension_with_adapters(
            &store,
            &state.adapters,
            id,
            params.enabled,
        )?;
    }
    Ok(Json(ids))
}

#[derive(Deserialize)]
pub struct ReadConfigPreviewParams {
    pub path: String,
    pub max_lines: Option<usize>,
}

pub async fn read_config_file_preview(
    State(_state): State<WebState>,
    Json(params): Json<ReadConfigPreviewParams>,
) -> Result<String> {
    let home = dirs::home_dir().unwrap_or_default();
    let canonical = std::fs::canonicalize(&params.path)
        .map_err(|_| ApiError::not_found("File not found"))?;
    if !canonical.starts_with(&home) {
        return Err(ApiError::forbidden("Path not allowed"));
    }
    let content = std::fs::read_to_string(&canonical)
        .map_err(|_| ApiError::not_found("Cannot read file"))?;
    let max = params.max_lines.unwrap_or(30);
    let truncated: String = content.lines().take(max).collect::<Vec<_>>().join("\n");
    Ok(Json(truncated))
}
