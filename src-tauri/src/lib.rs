mod agent;
mod hook;
mod ipc;
mod llm;
mod session;
mod window_utils;

use tauri::Manager;

use ipc::commands::{
    get_session_logs, launch_agent, respond_permission, select_project_folder, set_click_through,
    start_agent, stop_agent, translate_only,
};
use std::path::Path;
use std::sync::Arc;

fn load_dotenv_files() {
    let repo_dotenv = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|root| root.join(".env"));
    if let Some(ref path) = repo_dotenv {
        if path.is_file() {
            match dotenvy::from_filename(path) {
                Ok(_) => log::info!("已加载 .env: {}", path.display()),
                Err(e) => log::warn!(
                    "未能解析仓库根目录 .env（{}）：{}。若含 Windows 路径请使用正斜杠 D:/path/to/opencode-cli.exe",
                    path.display(),
                    e
                ),
            }
        }
    }
    match dotenvy::dotenv() {
        Ok(p) => log::info!("已加载工作目录 .env: {}", p.display()),
        Err(_) => log::debug!("当前工作目录无 .env 或为空，已跳过"),
    }
}

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
    load_dotenv_files();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(AppState::new()))
        .setup(|app| {
            hook::watcher::try_spawn_hook_log_watcher();

            let handle = app.handle().clone();
            let state = app.state::<Arc<AppState>>();
            session::cleanup::spawn_idle_cleanup_loop(handle, Arc::clone(&state));

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
