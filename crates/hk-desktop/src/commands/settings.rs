use hk_core::models::*;
use tauri::State;
use super::AppState;
use super::helpers::is_path_within_allowed_dirs;

// Root children count as level 1, so `1` here means show at most two levels total.
const DIR_PREVIEW_MAX_DEPTH: u8 = 1;
const DIR_PREVIEW_MAX_ENTRIES_PER_DIR: usize = 5;

#[tauri::command]
pub fn get_dashboard_stats(state: State<AppState>) -> Result<DashboardStats, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let all = store.list_extensions(None, None).map_err(|e| e.to_string())?;

    // Count issues from latest audit results
    let mut critical_issues = 0usize;
    let mut high_issues = 0usize;
    let mut medium_issues = 0usize;
    let mut low_issues = 0usize;
    for ext in &all {
        if let Ok(audits) = store.get_audit_results(&ext.id)
            && let Some(latest) = audits.first() {
                for finding in &latest.findings {
                    match finding.severity {
                        Severity::Critical => critical_issues += 1,
                        Severity::High => high_issues += 1,
                        Severity::Medium => medium_issues += 1,
                        Severity::Low => low_issues += 1,
                    }
                }
            }
    }

    Ok(DashboardStats {
        total_extensions: all.len(),
        skill_count: all.iter().filter(|e| e.kind == ExtensionKind::Skill).count(),
        mcp_count: all.iter().filter(|e| e.kind == ExtensionKind::Mcp).count(),
        plugin_count: all.iter().filter(|e| e.kind == ExtensionKind::Plugin).count(),
        hook_count: all.iter().filter(|e| e.kind == ExtensionKind::Hook).count(),
        cli_count: all.iter().filter(|e| e.kind == ExtensionKind::Cli).count(),
        critical_issues,
        high_issues,
        medium_issues,
        low_issues,
        updates_available: 0, // Populated by explicit check_updates call
    })
}

// --- Tags & Category commands ---

#[tauri::command]
pub fn update_tags(state: State<AppState>, id: String, tags: Vec<String>) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.update_tags(&id, &tags).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_tags(state: State<AppState>) -> Result<Vec<String>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.get_all_tags().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_category(state: State<AppState>, id: String, category: Option<String>) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.update_category(&id, category.as_deref()).map_err(|e| e.to_string())
}

// --- Config file preview ---

#[tauri::command]
pub fn read_config_file_preview(state: State<AppState>, path: String, max_lines: Option<usize>) -> Result<String, String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err("File not found".into());
    }

    if !is_path_within_allowed_dirs(file_path, &state)? {
        return Err("Path is not within a known agent or project directory".into());
    }

    if file_path.is_dir() {
        return Ok(render_dir_tree(file_path));
    }

    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let limit = max_lines.unwrap_or(30);
    let total_lines = content.lines().count();
    let mut preview: String = content
        .lines()
        .take(limit)
        .collect::<Vec<_>>()
        .join("\n");

    if total_lines > limit {
        preview.push_str(&format!("\n\n... ({} more lines)", total_lines - limit));
    }

    Ok(preview)
}

fn render_dir_tree(dir: &std::path::Path) -> String {
    let tree = format_dir_tree(dir, "", 0, DIR_PREVIEW_MAX_DEPTH, DIR_PREVIEW_MAX_ENTRIES_PER_DIR);
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
        b_dir.cmp(&a_dir).then_with(|| a.file_name().cmp(&b.file_name()))
    });
    // Skip hidden files/dirs
    entries.retain(|e| {
        !e.file_name().to_string_lossy().starts_with('.')
    });

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
        lines.push(format!("{}└── ... {} more item{}", prefix, omitted_count, suffix));
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
) -> Result<i64, String> {
    // Resolve ~ to home directory
    let resolved = if path.starts_with("~/") {
        dirs::home_dir()
            .map(|h| h.join(&path[2..]).to_string_lossy().to_string())
            .unwrap_or(path.clone())
    } else {
        path
    };
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.add_custom_config_path(&agent, &resolved, &label, &category).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_custom_config_path(
    state: State<AppState>,
    id: i64,
    path: String,
    label: String,
    category: String,
) -> Result<(), String> {
    let resolved = if path.starts_with("~/") {
        dirs::home_dir()
            .map(|h| h.join(&path[2..]).to_string_lossy().to_string())
            .unwrap_or(path.clone())
    } else {
        path
    };
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.update_custom_config_path(id, &resolved, &label, &category).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_custom_config_path(state: State<AppState>, id: i64) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.remove_custom_config_path(id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::AppState;
    use hk_core::store::Store;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    fn test_state() -> (AppState, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("test.db")).unwrap();
        (
            AppState {
                store: Arc::new(Mutex::new(store)),
                pending_clones: Arc::new(Mutex::new(HashMap::new())),
            },
            dir,
        )
    }

    #[test]
    fn test_custom_paths_are_allowed_for_preview_and_open() {
        let (state, dir) = test_state();
        let custom_dir = dir.path().join("custom");
        std::fs::create_dir_all(&custom_dir).unwrap();

        state.store.lock().unwrap()
            .add_custom_config_path("claude", &custom_dir.to_string_lossy(), "", "settings")
            .unwrap();

        assert!(is_path_within_allowed_dirs(&custom_dir, &state).unwrap());
    }

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
