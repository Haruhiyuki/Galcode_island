mod agent;
mod hook;
mod ipc;
mod llm;
mod session;
mod window_utils;

use tauri::Manager;

use ipc::commands::{
    get_session_logs, launch_agent, respond_permission, select_project_folder, set_click_through,
    start_agent, stop_agent, translate_only, update_llm_settings,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<std::sync::Mutex<agent::manager::AgentManager>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(std::sync::Mutex::new(agent::manager::AgentManager::new())),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(AppState::new()))
        .setup(|app| {
            hook::watcher::try_spawn_hook_log_watcher();

            let handle = app.handle().clone();
            let state = app.state::<Arc<AppState>>();
            hook::server::spawn_hook_http_server(handle.clone(), Arc::clone(&state));

            let install_handle = handle.clone();
            std::thread::spawn(move || {
                hook::installer::install_detected_hooks(&install_handle);
            });

            session::cleanup::spawn_idle_cleanup_loop(handle, Arc::clone(&state));

            ipc::tray::setup_tray(app).map_err(|e| {
                log::warn!("tray setup skipped: {}", e);
                e
            }).ok();

            // Windows WebView2 + transparent/hidden edge cases: force main window visible & focused.
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
                #[cfg(debug_assertions)]
                {
                    let _ = win.open_devtools();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            select_project_folder,
            start_agent,
            launch_agent,
            stop_agent,
            respond_permission,
            get_session_logs,
            translate_only,
            set_click_through,
            update_llm_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
