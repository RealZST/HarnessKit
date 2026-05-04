use super::AppState;
use hk_core::{HkError, models::*};
use tauri::State;

// Root children count as level 1, so `1` here means show at most two levels total.
const DIR_PREVIEW_MAX_DEPTH: u8 = 1;
const DIR_PREVIEW_MAX_ENTRIES_PER_DIR: usize = 5;

#[tauri::command]
pub fn get_dashboard_stats(state: State<AppState>) -> Result<DashboardStats, HkError> {
    let store = state.store.lock();
    let all = store.list_extensions(None, None)?;

    // Count issues from latest audit results in a single query instead of N+1
    let severity_counts = store.count_latest_findings_by_severity()?;

    Ok(DashboardStats {
        total_extensions: all.len(),
        skill_count: all
            .iter()
            .filter(|e| e.kind == ExtensionKind::Skill)
            .count(),
        mcp_count: all.iter().filter(|e| e.kind == ExtensionKind::Mcp).count(),
        plugin_count: all
            .iter()
            .filter(|e| e.kind == ExtensionKind::Plugin)
            .count(),
        hook_count: all.iter().filter(|e| e.kind == ExtensionKind::Hook).count(),
        cli_count: all.iter().filter(|e| e.kind == ExtensionKind::Cli).count(),
        critical_issues: severity_counts.get("critical").copied().unwrap_or(0),
        high_issues: severity_counts.get("high").copied().unwrap_or(0),
        medium_issues: severity_counts.get("medium").copied().unwrap_or(0),
        low_issues: severity_counts.get("low").copied().unwrap_or(0),
        updates_available: 0, // Populated by explicit check_updates call
    })
}

// --- Tags & Category commands ---

#[tauri::command]
pub fn update_tags(state: State<AppState>, id: String, tags: Vec<String>) -> Result<(), HkError> {
    let store = state.store.lock();
    store.update_tags(&id, &tags)
}

#[tauri::command]
pub fn batch_update_tags(
    state: State<AppState>,
    ids: Vec<String>,
    tags: Vec<String>,
) -> Result<(), HkError> {
    let store = state.store.lock();
    store.batch_update_tags(&ids, &tags)
}

#[tauri::command]
pub fn get_all_tags(state: State<AppState>) -> Result<Vec<String>, HkError> {
    let store = state.store.lock();
    store.get_all_tags()
}

#[tauri::command]
pub fn update_pack(
    state: State<AppState>,
    id: String,
    pack: Option<String>,
) -> Result<(), HkError> {
    let store = state.store.lock();
    store.update_pack(&id, pack.as_deref())
}

#[tauri::command]
pub fn batch_update_pack(
    state: State<AppState>,
    ids: Vec<String>,
    pack: Option<String>,
) -> Result<(), HkError> {
    let store = state.store.lock();
    store.batch_update_pack(&ids, pack.as_deref())
}

#[tauri::command]
pub fn get_all_packs(state: State<AppState>) -> Result<Vec<String>, HkError> {
    let store = state.store.lock();
    store.get_all_packs()
}

#[tauri::command]
pub fn toggle_by_pack(
    state: State<AppState>,
    pack: String,
    enabled: bool,
) -> Result<Vec<String>, HkError> {
    let store = state.store.lock();
    let ids = store.find_ids_by_pack(&pack)?;
    for id in &ids {
        hk_core::manager::toggle_extension(&store, id, enabled)?;
    }
    Ok(ids)
}

// --- Config file preview ---

#[tauri::command]
pub fn read_config_file_preview(
    _state: State<AppState>,
    path: String,
    max_lines: Option<usize>,
) -> Result<String, HkError> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err(HkError::NotFound("File not found".into()));
    }

    // OS gates whether the user can read this file; HK does not impose its
    // own home/project boundary on read-only previews of user-typed paths.
    if file_path.is_dir() {
        return Ok(render_dir_tree(file_path));
    }

    let content = std::fs::read_to_string(file_path)?;

    let limit = max_lines.unwrap_or(30);
    let total_lines = content.lines().count();
    let mut preview: String = content.lines().take(limit).collect::<Vec<_>>().join("\n");

    if total_lines > limit {
        preview.push_str(&format!("\n\n... ({} more lines)", total_lines - limit));
    }

    Ok(preview)
}

fn render_dir_tree(dir: &std::path::Path) -> String {
    let tree = format_dir_tree(
        dir,
        "",
        0,
        DIR_PREVIEW_MAX_DEPTH,
        DIR_PREVIEW_MAX_ENTRIES_PER_DIR,
    );
    if tree.is_empty() {
        "(empty directory)".to_string()
    } else {
        tree
    }
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
    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| {
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_dir
            .cmp(&a_dir)
            .then_with(|| a.file_name().cmp(&b.file_name()))
    });
    // Skip hidden files/dirs
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
                let subtree = format_dir_tree(
                    &entry.path(),
                    &child_prefix,
                    depth + 1,
                    max_depth,
                    max_entries_per_dir,
                );
                if !subtree.is_empty() {
                    lines.push(subtree);
                }
            }
        } else {
            lines.push(format!("{}{}{}", prefix, connector, name));
        }
    }

    if omitted_count > 0 {
        let suffix = if omitted_count == 1 { "" } else { "s" };
        lines.push(format!(
            "{}└── ... {} more item{}",
            prefix, omitted_count, suffix
        ));
    }

    lines.join("\n")
}

// --- Custom config path commands ---

#[tauri::command]
pub fn add_custom_config_path(
    state: State<AppState>,
    agent: String,
    path: String,
    label: String,
    category: String,
    target_scope: ConfigScope,
) -> Result<i64, HkError> {
    let resolved = hk_core::sanitize::resolve_and_validate_config_path(&path)?;
    let scope_json = serde_json::to_string(&target_scope).ok();
    let store = state.store.lock();
    store.add_custom_config_path(&agent, &resolved, &label, &category, scope_json.as_deref())
}

#[tauri::command]
pub fn update_custom_config_path(
    state: State<AppState>,
    id: i64,
    path: String,
    label: String,
    category: String,
) -> Result<(), HkError> {
    let resolved = hk_core::sanitize::resolve_and_validate_config_path(&path)?;
    let store = state.store.lock();
    store.update_custom_config_path(id, &resolved, &label, &category)
}

#[tauri::command]
pub fn remove_custom_config_path(state: State<AppState>, id: i64) -> Result<(), HkError> {
    let store = state.store.lock();
    store.remove_custom_config_path(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_dir_tree_truncates_large_directories() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..30 {
            std::fs::write(dir.path().join(format!("file-{i}.txt")), "x").unwrap();
        }

        let preview = render_dir_tree(dir.path());
        assert!(preview.contains("... 25 more items"));
    }

    #[test]
    fn test_render_dir_tree_limits_depth_to_two_levels() {
        let dir = tempfile::tempdir().unwrap();
        let level1 = dir.path().join("level-1");
        let level2 = level1.join("level-2");
        let level3 = level2.join("level-3");

        std::fs::create_dir_all(&level3).unwrap();
        std::fs::write(level1.join("visible.txt"), "x").unwrap();
        std::fs::write(level3.join("hidden.txt"), "x").unwrap();

        let preview = render_dir_tree(dir.path());
        assert!(preview.contains("level-1/"));
        assert!(preview.contains("level-2/"));
        assert!(preview.contains("visible.txt"));
        assert!(!preview.contains("level-3/"));
        assert!(!preview.contains("hidden.txt"));
    }
}
