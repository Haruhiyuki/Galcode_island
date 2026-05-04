// 这些 lint 在 backend 模块里"看着像问题但其实合理"，统一在 crate 级别抑制：
//   - needless_return: cfg(target_os) 平台分支里 return 是必须的（编译期只激活一个分支，
//     单分支视角下 return 后面"无代码"看似冗余，但少了 return 类型不匹配）
//   - too_many_arguments: backend launch/turn 函数固有参数多 (cwd/model/effort/proxy/binary/...)
//   - type_complexity: Arc<Mutex<HashMap<...>>> 等并发数据结构的复杂签名是设计本身
#![allow(
    clippy::needless_return,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

mod agent;
mod ipc;
mod llm;
mod session;
mod window_utils;

use tauri::Manager;

use ipc::commands::{
    get_session_logs, respond_permission, select_project_folder, set_click_through, start_agent,
    stop_agent, translate_only, update_llm_settings,
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

impl Default for AppState {
    fn default() -> Self {
        Self::new()
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
