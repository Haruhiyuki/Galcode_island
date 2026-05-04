use crate::agent::claude::{self as claude_agent, ClaudeModelsResult, CliRuntimeStatus};
use crate::agent::codex::{self as codex_agent, CodexStatus, CodexVerifyResult};
use crate::agent::manager::{self, LaunchResult};
use crate::agent::opencode::{self as opencode_agent, OpencodeStatus};
use crate::agent::runtime::{RuntimeState, DEFAULT_RUN_ID};
use crate::AppState;
use serde::Serialize;
use serde_json::{json, Value};
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

    eprintln!(
        "[galcode] start_agent: agent={agent_type} cwd={cwd} input_len={}",
        user_input_zh.len()
    );

    let prev = {
        let mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.active_session.clone()
    };
    if let Some(sid) = prev {
        let _ = manager::stop_session(
            app.clone(),
            Arc::clone(state.inner()),
            Arc::clone(runtime_state.inner()),
            sid,
        );
    }

    let result = match agent_type.as_str() {
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
        _ => Err(format!("暂不支持的 agent 类型: {}", agent_type)),
    };
    match &result {
        Ok(r) => eprintln!("[galcode] start_agent ok, sid={}", r.session_id),
        Err(e) => eprintln!("[galcode] start_agent FAILED: {}", e),
    }
    result
}

/// `session_id` 省略时停止当前 `active_session`。
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
                .and_then(|m| m.active_session.clone())
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
    provider: Option<String>,
    model: Option<String>,
    thinking: Option<bool>,
) -> Result<(), String> {
    crate::llm::client::update_global_settings(
        base_url,
        api_key,
        nickname,
        system_prompt,
        provider.unwrap_or_default(),
        model.unwrap_or_default(),
        thinking.unwrap_or(false),
    );
    Ok(())
}

/// 拉取 OpenAI 兼容服务商的模型列表（DeepSeek / OpenAI / Moonshot / 通义 / 智谱 等）。
/// `base_url` 和 `apiKey` 由前端传入，因为用户可能在保存前就想试探。
#[tauri::command]
pub async fn list_llm_models(
    base_url: String,
    api_key: String,
) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || crate::llm::client::list_models(&base_url, &api_key))
        .await
        .map_err(|error| format!("list_llm_models task failed: {error}"))?
}

/// 写入某个 backend 的运行时偏好（model / effort / proxy / binary，全部 Option）。
/// 后端的 launch_*_agent 在每次 turn 启动时都会读这份偏好。
/// `backend` 取值：`"claude-code" | "codex" | "opencode"`。
#[tauri::command]
pub fn update_backend_preferences(
    backend: String,
    model: Option<String>,
    effort: Option<String>,
    proxy: Option<String>,
    binary: Option<String>,
) -> Result<(), String> {
    crate::agent::preferences::update_backend_preferences(&backend, model, effort, proxy, binary)
}

#[tauri::command]
pub fn set_click_through(app: AppHandle, enabled: bool) -> Result<(), String> {
    let w = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到 main 窗口".to_string())?;
    crate::window_utils::set_click_through(&w, enabled)
}

// ===========================================================================
// 细粒度 backend 命令（与 designcode 风格对齐）
//
// 这些命令是"原始接口"——不走 LLM 翻译/总结管线，直接 spawn / send / 等响应。
// 当前 ChatBubble UI 仍然用 `start_agent`（套了 LLM + 凉宫春日总结），新命令
// 给 Settings / 调试面板 / 未来多 tab 直连场景使用。
// ===========================================================================

// ---------------------------------------------------------------------------
// Claude Code
// ---------------------------------------------------------------------------

// status / models 内部会 spawn `claude --version` / `claude auth status` /
// `claude --help` 三个子进程顺序 wait（首次还要 stage bundled binary 复制 ~220MB）
// 整体 4-8s 是常态。改成 async + spawn_blocking 把阻塞挪到 tokio blocking pool，
// 主线程不卡，前端 SettingsModal 能立即弹出再异步刷新状态条。

#[tauri::command]
pub async fn claude_status(
    app: AppHandle,
    binary: Option<String>,
) -> Result<CliRuntimeStatus, String> {
    let handle = app.clone();
    tokio::task::spawn_blocking(move || claude_agent::claude_status_snapshot(&handle, binary.as_deref()))
        .await
        .map_err(|error| format!("claude_status task failed: {error}"))?
}

#[tauri::command]
pub async fn claude_models(
    app: AppHandle,
    binary: Option<String>,
) -> Result<ClaudeModelsResult, String> {
    let handle = app.clone();
    tokio::task::spawn_blocking(move || claude_agent::build_claude_model_catalog(&handle, binary.as_deref()))
        .await
        .map_err(|error| format!("claude_models task failed: {error}"))?
}

#[tauri::command]
pub async fn claude_verify(
    app: AppHandle,
    model: Option<String>,
    effort: Option<String>,
    binary: Option<String>,
    proxy: Option<String>,
) -> Result<CodexVerifyResult, String> {
    let handle = app.clone();
    let join = tokio::task::spawn_blocking(move || {
        claude_agent::run_claude_probe(
            &handle,
            model.as_deref(),
            effort.as_deref(),
            binary.as_deref(),
            proxy.as_deref(),
        )
    });
    let message = join
        .await
        .map_err(|error| format!("Claude verification task failed to join: {error}"))??;
    Ok(CodexVerifyResult { ok: true, message })
}

#[tauri::command]
pub async fn claude_login_open(
    app: AppHandle,
    binary: Option<String>,
    proxy: Option<String>,
) -> Result<String, String> {
    let handle = app.clone();
    tokio::task::spawn_blocking(move || {
        claude_agent::open_claude_login_terminal(&handle, binary.as_deref(), proxy.as_deref())
    })
    .await
    .map_err(|error| format!("claude_login_open task failed: {error}"))?
}

/// Claude 原始 turn —— 不翻译、不套总结，直接走 stream-json。
/// 返回 { sessionId, output }（output 是 CLI 英文原文）。
#[tauri::command]
pub async fn claude_send_prompt(
    app: AppHandle,
    runtime_state: State<'_, Arc<RuntimeState>>,
    run_id: Option<String>,
    text: String,
    cwd: String,
    session_id: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    binary: Option<String>,
    proxy: Option<String>,
    stream_id: Option<String>,
) -> Result<Value, String> {
    let runtime = Arc::clone(runtime_state.inner());
    let run = run_id.unwrap_or_else(|| DEFAULT_RUN_ID.to_string());
    let handle = app.clone();
    let join = tokio::task::spawn_blocking(move || {
        claude_agent::run_claude_stream_turn(
            &handle,
            runtime.as_ref(),
            &run,
            &text,
            &cwd,
            session_id.as_deref(),
            model.as_deref(),
            effort.as_deref(),
            binary.as_deref(),
            proxy.as_deref(),
            stream_id.as_deref(),
        )
    });
    let (next_session_id, output) = join
        .await
        .map_err(|error| format!("Claude prompt task failed to join: {error}"))??;
    Ok(json!({
        "sessionId": next_session_id,
        "output": output,
    }))
}

// ---------------------------------------------------------------------------
// Codex
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn codex_status(app: AppHandle, binary: Option<String>) -> Result<CodexStatus, String> {
    let handle = app.clone();
    tokio::task::spawn_blocking(move || codex_agent::codex_status_snapshot(&handle, binary.as_deref()))
        .await
        .map_err(|error| format!("codex_status task failed: {error}"))?
}

#[tauri::command]
pub async fn codex_verify(
    app: AppHandle,
    model: Option<String>,
    reasoning_effort: Option<String>,
    binary: Option<String>,
    proxy: Option<String>,
) -> Result<CodexVerifyResult, String> {
    let handle = app.clone();
    let join = tokio::task::spawn_blocking(move || {
        codex_agent::run_codex_probe(
            &handle,
            model.as_deref(),
            reasoning_effort.as_deref(),
            binary.as_deref(),
            proxy.as_deref(),
        )
    });
    let message = join
        .await
        .map_err(|error| format!("Codex verification task failed to join: {error}"))??;
    Ok(CodexVerifyResult { ok: true, message })
}

#[tauri::command]
pub async fn codex_login_open(
    app: AppHandle,
    binary: Option<String>,
    device_auth: Option<bool>,
    proxy: Option<String>,
) -> Result<String, String> {
    let handle = app.clone();
    tokio::task::spawn_blocking(move || {
        codex_agent::open_codex_login_terminal(
            &handle,
            binary.as_deref(),
            device_auth.unwrap_or(false),
            proxy.as_deref(),
        )
    })
    .await
    .map_err(|error| format!("codex_login_open task failed: {error}"))?
}

/// Codex 原始 turn —— 通过 app-server JSON-RPC，不翻译、不套总结。
/// 返回 { threadId, output }。
#[tauri::command]
pub async fn codex_send_prompt(
    app: AppHandle,
    runtime_state: State<'_, Arc<RuntimeState>>,
    run_id: Option<String>,
    thread_id: Option<String>,
    text: String,
    system: Option<String>,
    cwd: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    binary: Option<String>,
    proxy: Option<String>,
    stream_id: Option<String>,
) -> Result<Value, String> {
    let runtime = Arc::clone(runtime_state.inner());
    let run = run_id.unwrap_or_else(|| DEFAULT_RUN_ID.to_string());
    let handle = app.clone();
    let join = tokio::task::spawn_blocking(move || {
        codex_agent::run_codex_app_server_turn(
            &handle,
            runtime.as_ref(),
            &run,
            &cwd,
            thread_id.as_deref(),
            system.as_deref(),
            &text,
            model.as_deref(),
            reasoning_effort.as_deref(),
            binary.as_deref(),
            proxy.as_deref(),
            stream_id.as_deref(),
        )
    });
    let (next_thread_id, output) = join
        .await
        .map_err(|error| format!("Codex prompt task failed to join: {error}"))??;
    Ok(json!({
        "threadId": next_thread_id,
        "output": output,
    }))
}

// ---------------------------------------------------------------------------
// OpenCode（生命周期 + send_prompt）
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn opencode_status(
    app: AppHandle,
    runtime_state: State<'_, Arc<RuntimeState>>,
    run_id: Option<String>,
) -> Result<OpencodeStatus, String> {
    let run = run_id.unwrap_or_else(|| DEFAULT_RUN_ID.to_string());
    opencode_agent::snapshot_opencode(&app, runtime_state.inner().as_ref(), &run).await
}

#[tauri::command]
pub async fn opencode_start(
    app: AppHandle,
    runtime_state: State<'_, Arc<RuntimeState>>,
    run_id: Option<String>,
    binary: Option<String>,
    proxy: Option<String>,
    port: Option<u16>,
    cwd: Option<String>,
) -> Result<OpencodeStatus, String> {
    let run = run_id.unwrap_or_else(|| DEFAULT_RUN_ID.to_string());
    opencode_agent::opencode_start(
        &app,
        runtime_state.inner().as_ref(),
        &run,
        binary.as_deref(),
        proxy.as_deref(),
        port,
        cwd.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn opencode_stop(
    runtime_state: State<'_, Arc<RuntimeState>>,
    run_id: Option<String>,
) -> Result<OpencodeStatus, String> {
    let run = run_id.unwrap_or_else(|| DEFAULT_RUN_ID.to_string());
    opencode_agent::opencode_stop(runtime_state.inner().as_ref(), &run).await
}

#[tauri::command]
pub async fn opencode_create_session(
    app: AppHandle,
    runtime_state: State<'_, Arc<RuntimeState>>,
    run_id: Option<String>,
    title: Option<String>,
    directory: Option<String>,
) -> Result<String, String> {
    let run = run_id.unwrap_or_else(|| DEFAULT_RUN_ID.to_string());
    opencode_agent::opencode_create_session(
        &app,
        runtime_state.inner().as_ref(),
        &run,
        title.as_deref(),
        directory.as_deref(),
    )
    .await
}

/// OpenCode 原始 turn —— HTTP POST + SSE，session_id 必须先 create。
/// 返回 { text, raw }（text 是从 message list 提取的纯文本）。
#[tauri::command]
pub async fn opencode_send_prompt(
    app: AppHandle,
    runtime_state: State<'_, Arc<RuntimeState>>,
    run_id: Option<String>,
    session_id: String,
    text: String,
    system: Option<String>,
    directory: Option<String>,
    stream_id: Option<String>,
) -> Result<Value, String> {
    let run = run_id.unwrap_or_else(|| DEFAULT_RUN_ID.to_string());
    let (final_text, raw) = opencode_agent::run_opencode_turn(
        &app,
        runtime_state.inner().as_ref(),
        &run,
        &session_id,
        &text,
        system.as_deref(),
        directory.as_deref(),
        stream_id.as_deref(),
    )
    .await?;
    Ok(json!({ "text": final_text, "raw": raw }))
}
