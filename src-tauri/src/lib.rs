mod commands;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::new().expect("failed to init app state");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            create_profile,
            update_profile,
            delete_profile,
            set_engine_path,
            engine_configured,
            launch_profile,
            stop_profile,
            check_proxy,
            add_extensions,
            get_default_extensions,
            set_default_extensions,
            clone_profile_cmd,
            check_for_update,
            download_update,
            download_engine,
            profile_size,
            clear_profile_cache,
            clear_all_caches,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
