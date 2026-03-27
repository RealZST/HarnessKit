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
        .manage(AppState {
            store: Mutex::new(store),
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_extensions,
            commands::list_agents,
            commands::get_dashboard_stats,
            commands::toggle_extension,
            commands::run_audit,
            commands::scan_and_sync,
            commands::delete_extension,
            commands::get_extension_content,
            commands::check_updates,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
