mod commands;
mod icon;

use commands::AppState;
use hk_core::store::Store;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    let data_dir = dirs::home_dir()
        .expect("Cannot determine home directory — set HOME environment variable")
        .join(".harnesskit");
    std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
    let store = Store::open(&data_dir.join("metadata.db")).expect("Failed to open database");


    // NOTE: tauri.conf.json sets `macOSPrivateApi: true`. This is required for:
    // 1. Window transparency (`"transparent": true` in window config)
    // 2. Sidebar vibrancy effect (`"windowEffects": {"effects": ["sidebar"]}`)
    // Without this flag, the NSVisualEffectView APIs needed for these effects
    // are not accessible, resulting in an opaque white window background.
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            store: Arc::new(Mutex::new(store)),
            pending_clones: Arc::new(Mutex::new(std::collections::HashMap::new())),
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
            commands::get_skill_locations,
            commands::get_cached_update_statuses,
            commands::check_updates,
            commands::update_extension,
            commands::install_from_local,
            commands::install_from_git,
            commands::update_tags,
            commands::get_all_tags,
            commands::update_pack,
            commands::get_all_packs,
            commands::toggle_by_pack,
            commands::search_marketplace,
            commands::trending_marketplace,
            commands::fetch_skill_preview,
            commands::fetch_cli_readme,
            commands::fetch_skill_audit,
            commands::install_from_marketplace,
            commands::deploy_to_agent,
            commands::list_projects,
            commands::add_project,
            commands::remove_project,
            commands::discover_projects,
            commands::update_agent_path,
            commands::set_agent_enabled,
            commands::update_agent_order,
            commands::list_skill_files,
            commands::open_in_system,
            commands::reveal_in_file_manager,
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
            icon::set_app_icon,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide instead of quit on macOS red X
                window.hide().unwrap_or_default();
                api.prevent_close();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                // Re-show window when dock icon is clicked
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}
