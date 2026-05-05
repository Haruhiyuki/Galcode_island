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

/// 多 tab UI 启动 / reattach 时枚举所有活跃会话。
/// 前端拿这个 list 跟自己持久化的 tab 列表对比，决定哪些 tab 还能继续显示进度。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub run_id: String,
    pub agent_type: String,
    pub status: crate::session::state::AgentStatus,
    pub cwd: Option<String>,
    pub stream_id: String,
    pub last_user_prompt: Option<String>,
    pub created_at_ms: u128,
}

/// 返回当前 manager 里所有 session 的摘要快照。
/// 不带状态过滤——已完成 / 出错的也会列出来，让前端决定是否清理。
#[tauri::command]
pub fn list_sessions(state: State<Arc<AppState>>) -> Result<Vec<SessionSummary>, String> {
    let mgr = state.manager.lock().map_err(|e| e.to_string())?;
    let mut summaries: Vec<SessionSummary> = mgr
        .sessions
        .iter()
        .map(|(sid, sess)| {
            let snap = sess.snapshot.lock().ok();
            SessionSummary {
                session_id: sid.clone(),
                run_id: sess.run_id.clone(),
                agent_type: snap
                    .as_ref()
                    .map(|s| s.agent_type.clone())
                    .unwrap_or_default(),
                status: snap
                    .as_ref()
                    .map(|s| s.status)
                    .unwrap_or(crate::session::state::AgentStatus::Idle),
                cwd: snap.as_ref().and_then(|s| s.cwd.clone()),
                stream_id: sess.stream_id.clone(),
                last_user_prompt: snap.as_ref().and_then(|s| s.last_user_prompt.clone()),
                created_at_ms: sess.created_at.elapsed().as_millis(),
            }
        })
        .collect();
    // 按最近创建（elapsed 越小越新）排在前面
    summaries.sort_by_key(|s| s.created_at_ms);
    Ok(summaries)
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
///
/// `run_id` 是 tab 标识：多 tab UI 下每个 tab 独占一个 run_id，所有
/// IPC 事件按 run_id 分发到对应 tab slice。前端不传时兜底 DEFAULT_RUN_ID
/// （单 tab 模式下兼容老调用路径）。
///
/// 多 tab 模式下，**不再**强行 stop 上一个 active_session：
/// 每个 tab 独立运行，互不干扰；只有传入相同 run_id 时才会替换原有任务。
#[tauri::command]
pub fn start_agent(
    app: AppHandle,
    state: State<Arc<AppState>>,
    runtime_state: State<Arc<RuntimeState>>,
    user_input_zh: String,
    cwd: Option<String>,
    agent: Option<String>,
    run_id: Option<String>,
) -> Result<LaunchResult, String> {
    let cwd = cwd.unwrap_or_else(|| ".".to_string());
    let agent_type = agent
        .clone()
        .unwrap_or_else(|| "claude-code".to_string());
    let run_id = run_id.unwrap_or_else(|| DEFAULT_RUN_ID.to_string());

    eprintln!(
        "[galcode] start_agent: run_id={run_id} agent={agent_type} cwd={cwd} input_len={}",
        user_input_zh.len()
    );

    // 如果同 run_id 还有未完成的会话，先停掉再重启（同 tab 内只能跑一个 turn）。
    // 不同 run_id 的并发会话互不影响。
    let prev = {
        let mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.sessions
            .iter()
            .find(|(_, sess)| sess.run_id == run_id)
            .map(|(sid, _)| sid.clone())
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
            run_id,
            cwd,
            user_input_zh,
        ),
        "opencode" => manager::launch_opencode_agent(
            app,
            Arc::clone(state.inner()),
            Arc::clone(runtime_state.inner()),
            run_id,
            cwd,
            user_input_zh,
        ),
        "codex" => manager::launch_codex_agent(
            app,
            Arc::clone(state.inner()),
            Arc::clone(runtime_state.inner()),
            run_id,
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

/// 停止指定会话。
///
/// 优先级：`session_id` > `run_id` 反查会话 > active_session 兜底。
/// 多 tab 模式下推荐传 `run_id`：从该 tab 的 sessions 里找当前 active 的会话停掉。
#[tauri::command]
pub fn stop_agent(
    app: AppHandle,
    state: State<Arc<AppState>>,
    runtime_state: State<Arc<RuntimeState>>,
    session_id: Option<String>,
    run_id: Option<String>,
) -> Result<(), String> {
    let sid = session_id
        .or_else(|| {
            run_id.as_ref().and_then(|rid| {
                state.manager.lock().ok().and_then(|mgr| {
                    mgr.sessions
                        .iter()
                        .find(|(_, sess)| sess.run_id == *rid)
                        .map(|(sid, _)| sid.clone())
                })
            })
        })
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
