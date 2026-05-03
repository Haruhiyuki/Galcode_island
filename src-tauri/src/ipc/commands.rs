use crate::agent::manager::{self, LaunchResult};
use crate::AppState;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResponse {
    pub ok: bool,
}

#[tauri::command]
pub fn select_project_folder(app: AppHandle) -> Result<Option<String>, String> {
    Ok(app.dialog().file().blocking_pick_folder().and_then(|fp| {
        fp.into_path()
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    }))
}

/// 黑客松计划兼容：中文任务 → 可选翻译 → 启动 Agent（工作目录默认 `.`，可传 `cwd`）。
#[tauri::command]
pub fn start_agent(
    app: AppHandle,
    state: State<Arc<AppState>>,
    user_input_zh: String,
    cwd: Option<String>,
    agent: Option<String>,
) -> Result<LaunchResult, String> {
    let cwd = cwd.unwrap_or_else(|| ".".to_string());
    let agent_type = agent.clone().unwrap_or_else(|| "opencode".to_string());

    eprintln!(
        "[galcode] start_agent called: agent={}, cwd={}, task={}",
        agent_type, cwd, user_input_zh
    );

    let prev = {
        let mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.active_demo_session.clone()
    };
    if let Some(sid) = prev {
        let _ = manager::stop_session(app.clone(), Arc::clone(state.inner()), sid);
    }

    eprintln!("[galcode] routing to agent type: {}", agent_type);
    match agent_type.as_str() {
        "opencode" => {
            eprintln!("[galcode] -> launch_opencode_agent");
            manager::launch_opencode_agent(app, Arc::clone(state.inner()), cwd, user_input_zh)
        }
        "demo" => {
            eprintln!("[galcode] -> launch_demo_agent");
            manager::launch_demo_agent(app, Arc::clone(state.inner()), cwd, user_input_zh)
        }
        "claude-code" => Err("Claude Code agent 尚未接入，敬请期待".into()),
        _ => Err(format!("暂不支持的 agent 类型: {}", agent_type)),
    }
}

#[tauri::command]
pub fn launch_agent(
    app: AppHandle,
    state: State<Arc<AppState>>,
    agent: String,
    cwd: String,
    task_zh: Option<String>,
) -> Result<LaunchResult, String> {
    match agent.as_str() {
        "demo" => {
            let task = task_zh.ok_or_else(|| "demo agent 需要参数 task_zh".to_string())?;
            manager::launch_demo_agent(app, Arc::clone(state.inner()), cwd, task)
        }
        "opencode" => {
            let task = task_zh.ok_or_else(|| "opencode agent 需要参数 task_zh".to_string())?;
            manager::launch_opencode_agent(app, Arc::clone(state.inner()), cwd, task)
        }
        _ => Err(format!("暂不支持的 agent 类型: {}", agent)),
    }
}

/// `session_id` 省略时停止当前 `active_demo_session`（与计划中的无参 `stop_agent` 一致）。
#[tauri::command]
pub fn stop_agent(
    app: AppHandle,
    state: State<Arc<AppState>>,
    session_id: Option<String>,
) -> Result<(), String> {
    let sid = session_id
        .or_else(|| {
            state
                .manager
                .lock()
                .ok()
                .and_then(|m| m.active_demo_session.clone())
        })
        .ok_or_else(|| "没有可停止的会话".to_string())?;
    manager::stop_session(app, Arc::clone(state.inner()), sid)
}

#[tauri::command]
pub fn respond_permission(
    state: State<Arc<AppState>>,
    session_id: String,
    tool_use_id: String,
    decision: String,
) -> Result<PermissionResponse, String> {
    let mut mgr = state.manager.lock().map_err(|e| e.to_string())?;
    mgr.respond_permission_stub(&session_id, &tool_use_id, &decision)?;
    Ok(PermissionResponse { ok: true })
}

#[tauri::command]
pub fn get_session_logs(state: State<Arc<AppState>>, session_id: String) -> Result<Vec<String>, String> {
    manager::get_logs(Arc::clone(state.inner()), session_id)
}

#[tauri::command]
pub fn translate_only(text_zh: String) -> Result<String, String> {
    let cfg = crate::llm::load_llm_config().ok_or_else(|| "未配置 LLM_API_KEY".to_string())?;
    crate::llm::translate_zh_to_en(&cfg, &text_zh)
}

#[tauri::command]
pub fn update_llm_settings(base_url: String, api_key: String, nickname: String, system_prompt: String) -> Result<(), String> {
    crate::llm::client::update_global_settings(base_url, api_key, nickname, system_prompt);
    Ok(())
}

#[tauri::command]
pub fn set_click_through(app: AppHandle, enabled: bool) -> Result<(), String> {
    let w = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到 main 窗口".to_string())?;
    crate::window_utils::set_click_through(&w, enabled)
}
