use crate::agent::manager::{self, LaunchResult};
use crate::agent::runtime::RuntimeState;
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

/// 中文任务 → 翻译 → 启动 Agent（claude-code / opencode / codex / demo）。
/// 工作目录默认 `.`，可通过 `cwd` 指定。
#[tauri::command]
pub fn start_agent(
    app: AppHandle,
    state: State<Arc<AppState>>,
    runtime_state: State<Arc<RuntimeState>>,
    user_input_zh: String,
    cwd: Option<String>,
    agent: Option<String>,
) -> Result<LaunchResult, String> {
    let cwd = cwd.unwrap_or_else(|| ".".to_string());
    let agent_type = agent
        .clone()
        .unwrap_or_else(|| "claude-code".to_string());

    let prev = {
        let mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.active_demo_session.clone()
    };
    if let Some(sid) = prev {
        let _ = manager::stop_session(
            app.clone(),
            Arc::clone(state.inner()),
            Arc::clone(runtime_state.inner()),
            sid,
        );
    }

    match agent_type.as_str() {
        "claude-code" => manager::launch_claude_agent(
            app,
            Arc::clone(state.inner()),
            Arc::clone(runtime_state.inner()),
            cwd,
            user_input_zh,
        ),
        "opencode" => manager::launch_opencode_agent(
            app,
            Arc::clone(state.inner()),
            Arc::clone(runtime_state.inner()),
            cwd,
            user_input_zh,
        ),
        "codex" => manager::launch_codex_agent(
            app,
            Arc::clone(state.inner()),
            Arc::clone(runtime_state.inner()),
            cwd,
            user_input_zh,
        ),
        "demo" => manager::launch_demo_agent(app, Arc::clone(state.inner()), cwd, user_input_zh),
        _ => Err(format!("暂不支持的 agent 类型: {}", agent_type)),
    }
}

#[tauri::command]
pub fn launch_agent(
    app: AppHandle,
    state: State<Arc<AppState>>,
    runtime_state: State<Arc<RuntimeState>>,
    agent: String,
    cwd: String,
    task_zh: Option<String>,
) -> Result<LaunchResult, String> {
    let task = task_zh
        .clone()
        .ok_or_else(|| format!("{} agent 需要参数 task_zh", agent))?;
    match agent.as_str() {
        "claude-code" => manager::launch_claude_agent(
            app,
            Arc::clone(state.inner()),
            Arc::clone(runtime_state.inner()),
            cwd,
            task,
        ),
        "opencode" => manager::launch_opencode_agent(
            app,
            Arc::clone(state.inner()),
            Arc::clone(runtime_state.inner()),
            cwd,
            task,
        ),
        "codex" => manager::launch_codex_agent(
            app,
            Arc::clone(state.inner()),
            Arc::clone(runtime_state.inner()),
            cwd,
            task,
        ),
        "demo" => manager::launch_demo_agent(app, Arc::clone(state.inner()), cwd, task),
        _ => Err(format!("暂不支持的 agent 类型: {}", agent)),
    }
}

/// `session_id` 省略时停止当前 `active_demo_session`。
#[tauri::command]
pub fn stop_agent(
    app: AppHandle,
    state: State<Arc<AppState>>,
    runtime_state: State<Arc<RuntimeState>>,
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
    manager::stop_session(
        app,
        Arc::clone(state.inner()),
        Arc::clone(runtime_state.inner()),
        sid,
    )
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
pub fn get_session_logs(
    state: State<Arc<AppState>>,
    session_id: String,
) -> Result<Vec<String>, String> {
    manager::get_logs(Arc::clone(state.inner()), session_id)
}

#[tauri::command]
pub fn translate_only(text_zh: String) -> Result<String, String> {
    let cfg = crate::llm::load_llm_config().ok_or_else(|| "未配置 LLM_API_KEY".to_string())?;
    crate::llm::translate_zh_to_en(&cfg, &text_zh)
}

#[tauri::command]
pub fn update_llm_settings(
    base_url: String,
    api_key: String,
    nickname: String,
    system_prompt: String,
) -> Result<(), String> {
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
