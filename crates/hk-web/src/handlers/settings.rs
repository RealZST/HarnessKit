use axum::extract::State;
use axum::Json;
use hk_core::models::DashboardStats;
use hk_core::manager;
use serde::Deserialize;

use crate::router::{blocking, ApiError};
use crate::state::WebState;

type Result<T> = std::result::Result<Json<T>, ApiError>;

pub async fn get_dashboard_stats(
    State(state): State<WebState>,
) -> Result<DashboardStats> {
    blocking(move || {
        let store = state.store.lock();
        let exts = store.list_extensions(None, None)?;
        let severity_map = store.count_latest_findings_by_severity()?;
        Ok(DashboardStats {
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
        })
    }).await
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
    blocking(move || {
        let store = state.store.lock();
        store.update_tags(&params.id, &params.tags)?;
        Ok(())
    }).await
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
    blocking(move || {
        let store = state.store.lock();
        store.batch_update_tags(&params.ids, &params.tags)?;
        Ok(())
    }).await
}

pub async fn get_all_tags(
    State(state): State<WebState>,
) -> Result<Vec<String>> {
    blocking(move || {
        let store = state.store.lock();
        Ok(store.get_all_tags()?)
    }).await
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
    blocking(move || {
        let store = state.store.lock();
        store.update_pack(&params.id, params.pack.as_deref())?;
        Ok(())
    }).await
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
    blocking(move || {
        let store = state.store.lock();
        store.batch_update_pack(&params.ids, params.pack.as_deref())?;
        Ok(())
    }).await
}

pub async fn get_all_packs(
    State(state): State<WebState>,
) -> Result<Vec<String>> {
    blocking(move || {
        let store = state.store.lock();
        Ok(store.get_all_packs()?)
    }).await
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
    blocking(move || {
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
        Ok(ids)
    }).await
}

#[derive(Deserialize)]
pub struct ReadConfigPreviewParams {
    pub path: String,
    pub max_lines: Option<usize>,
}

pub async fn read_config_file_preview(
    State(state): State<WebState>,
    Json(params): Json<ReadConfigPreviewParams>,
) -> Result<String> {
    blocking(move || {
        let file_path = std::path::Path::new(&params.path);
        if !file_path.exists() {
            return Err(hk_core::HkError::NotFound("File not found".into()));
        }
        let canonical = std::fs::canonicalize(file_path)
            .map_err(|_| hk_core::HkError::NotFound("File not found".into()))?;
        let store = state.store.lock();
        if !super::is_path_allowed(&canonical, &store) {
            return Err(hk_core::HkError::PermissionDenied("Path not allowed".into()));
        }

        if file_path.is_dir() {
            return Ok(render_dir_tree(file_path));
        }

        let content = std::fs::read_to_string(&canonical)
            .map_err(|_| hk_core::HkError::NotFound("Cannot read file".into()))?;
        let max = params.max_lines.unwrap_or(30);
        let total_lines = content.lines().count();
        let mut preview: String = content.lines().take(max).collect::<Vec<_>>().join("\n");
        if total_lines > max {
            preview.push_str(&format!("\n\n... ({} more lines)", total_lines - max));
        }
        Ok(preview)
    }).await
}

const DIR_PREVIEW_MAX_DEPTH: u8 = 1;
const DIR_PREVIEW_MAX_ENTRIES_PER_DIR: usize = 5;

fn render_dir_tree(dir: &std::path::Path) -> String {
    let tree = format_dir_tree(dir, "", 0, DIR_PREVIEW_MAX_DEPTH, DIR_PREVIEW_MAX_ENTRIES_PER_DIR);
    if tree.is_empty() { "(empty directory)".to_string() } else { tree }
}

fn format_dir_tree(
    dir: &std::path::Path,
    prefix: &str,
    depth: u8,
    max_depth: u8,
    max_entries_per_dir: usize,
) -> String {
    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return String::new(),
    };
    entries.sort_by(|a, b| {
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
    });
    entries.retain(|e| !e.file_name().to_string_lossy().starts_with('.'));

    let omitted_count = entries.len().saturating_sub(max_entries_per_dir);
    entries.truncate(max_entries_per_dir);

    let mut lines = Vec::new();
    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == count - 1 && omitted_count == 0;
        let connector = if is_last { "└── " } else { "├── " };
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if is_dir {
            lines.push(format!("{}{}{}/", prefix, connector, name));
            if depth < max_depth {
                let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                let subtree = format_dir_tree(&entry.path(), &child_prefix, depth + 1, max_depth, max_entries_per_dir);
                if !subtree.is_empty() { lines.push(subtree); }
            }
        } else {
            lines.push(format!("{}{}{}", prefix, connector, name));
        }
    }

    if omitted_count > 0 {
        let suffix = if omitted_count == 1 { "" } else { "s" };
        lines.push(format!("{}└── ... {} more item{}", prefix, omitted_count, suffix));
    }

    lines.join("\n")
}
