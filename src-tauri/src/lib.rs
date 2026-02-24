mod hive;
mod pty;

use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pty_manager = Arc::new(Mutex::new(pty::PtyManager::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(pty_manager)
        .invoke_handler(tauri::generate_handler![
            // PTY
            pty::create_pty,
            pty::write_to_pty,
            pty::resize_pty,
            pty::close_pty,
            // App config
            hive::load_app_config,
            hive::save_app_config,
            hive::reset_app,
            hive::get_app_config_path,
            // Hive management
            hive::preflight_check,
            hive::get_home_dir,
            hive::list_dirs,
            hive::init_beehive,
            hive::load_beehive,
            hive::verify_repo,
            hive::create_hive,
            hive::list_hives,
            hive::delete_hive,
            hive::list_branches,
            hive::create_comb,
            hive::list_combs,
            hive::delete_comb,
            hive::copy_comb,
            hive::save_comb_panes,
            hive::get_comb_panes,
            hive::save_custom_buttons,
            hive::install_cli,
            hive::uninstall_cli,
            hive::cli_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
