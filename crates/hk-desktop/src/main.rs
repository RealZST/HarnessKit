mod commands;

use commands::AppState;
use hk_core::store::Store;
use std::sync::Mutex;

#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    let data_dir = dirs::home_dir().unwrap_or_default().join(".harnesskit");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
    let store = Store::open(&data_dir.join("metadata.db")).expect("Failed to open database");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            store: Mutex::new(store),
            pending_clones: Mutex::new(std::collections::HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_extensions,
            commands::list_agents,
            commands::get_dashboard_stats,
            commands::toggle_extension,
            commands::list_audit_results,
            commands::run_audit,
            commands::scan_and_sync,
            commands::delete_extension,
            commands::get_extension_content,
            commands::check_updates,
            commands::update_extension,
            commands::install_from_git,
            commands::update_tags,
            commands::get_all_tags,
            commands::update_category,
            commands::search_marketplace,
            commands::trending_marketplace,
            commands::fetch_skill_preview,
            commands::fetch_skill_audit,
            commands::install_from_marketplace,
            commands::deploy_to_agent,
            commands::list_projects,
            commands::add_project,
            commands::remove_project,
            commands::discover_projects,
            commands::get_project_extensions,
            commands::update_agent_path,
            commands::set_agent_enabled,
            commands::update_agent_order,
            commands::list_skill_files,
            commands::open_in_system,
            commands::list_agent_configs,
            commands::read_config_file_preview,
            commands::scan_git_repo,
            commands::install_scanned_skills,
            commands::get_cli_with_children,
            commands::list_cli_marketplace,
            commands::install_cli,
            commands::add_custom_config_path,
            commands::update_custom_config_path,
            commands::remove_custom_config_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
