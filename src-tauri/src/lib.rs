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
        .manage(Arc::new(agent::runtime::RuntimeState::default()))
        .setup(|app| {
            hook::watcher::try_spawn_hook_log_watcher();

            let handle = app.handle().clone();
            let state = app.state::<Arc<AppState>>();
            session::cleanup::spawn_idle_cleanup_loop(handle.clone(), Arc::clone(&state));

            // 启动时清理上一轮崩溃 / 强退留下的 opencode/codex/claude 孤儿子进程，
            // 避免端口被占用或重复消耗 token。
            agent::sysutils::cleanup_stale_runtime_orphans(&handle);

            ipc::tray::setup_tray(app).map_err(|e| {
                log::warn!("tray setup skipped: {}", e);
                e
            }).ok();

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
