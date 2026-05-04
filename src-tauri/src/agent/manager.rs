// Agent 会话管理器：start/stop/总结/翻译/IPC 事件聚合层。
//
// 设计：
//   - 三个 backend (Claude / OpenCode / Codex) 都通过对应 agent::xxx 模块完成 CLI 通信
//   - 本模块负责把每个 turn 套到 LLM 翻译/总结管线里：
//       中文 prompt → translate_zh_to_en → backend turn → 拿到英文输出
//       英文输出 → translate_en_to_zh → 中文 → generate_agent_summary → mode/emotion/options
//   - SessionSnapshot 状态由 IPC events 透传给前端宠物气泡
//   - 会话续接：每个 backend 自动捕获 session_id 存到 RuntimeState 里供下次 turn 复用

use crate::agent::runtime::{ClaudeStreamClient, RuntimeState, DEFAULT_RUN_ID};
use crate::agent::{claude as claude_agent, codex as codex_agent, opencode as opencode_agent};
use crate::ipc::events::{self, SessionCompletePayload};
use crate::llm::{
    generate_agent_summary, load_llm_config, translate_en_to_zh, translate_zh_to_en, LlmConfig,
};
use crate::session::snapshot::SessionSnapshot;
use crate::session::state::AgentStatus;
use crate::AppState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

// ---------------------------------------------------------------------------
// 会话与管理器
// ---------------------------------------------------------------------------

pub struct AgentSession {
    pub snapshot: Arc<Mutex<SessionSnapshot>>,
    /// 用 get_session_logs 命令读出（暂未由 backend 主动写入，预留作未来调试面板）。
    pub logs: Arc<Mutex<Vec<String>>>,
    pub created_at: Instant,
    /// 用于 cli-output 事件路由（前端按 stream_id 把流式日志分发到对应会话面板）。
    pub stream_id: String,
}

impl AgentSession {
    pub fn new(session_id: String, agent_type: String, cwd: Option<String>) -> Self {
        let stream_id = format!("stream-{}", session_id);
        Self {
            snapshot: Arc::new(Mutex::new(SessionSnapshot::new(
                session_id,
                agent_type,
                cwd,
                None,
            ))),
            logs: Arc::new(Mutex::new(Vec::new())),
            created_at: Instant::now(),
            stream_id,
        }
    }
}

pub struct AgentManager {
    pub sessions: HashMap<String, AgentSession>,
    pending_permission: HashMap<(String, String), ()>,
    /// 当前活动会话（兼容老的无参 stop_agent）。命名沿用 demo 时代，但承载所有 backend。
    pub active_session: Option<String>,
    /// 会话续接缓存：(agent_type, cwd) → 上次的 session_id / thread_id。
    /// 下次同 agent_type+cwd 提交时自动 resume，让对话有上下文延续。
    pub last_session_per_context: HashMap<(String, String), String>,
}

impl AgentManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            pending_permission: HashMap::new(),
            active_session: None,
            last_session_per_context: HashMap::new(),
        }
    }

    pub fn clear_active_session_if(&mut self, session_id: &str) {
        if self.active_session.as_deref() == Some(session_id) {
            self.active_session = None;
        }
    }

    pub fn cleanup_completed_sessions(&mut self, max_age: std::time::Duration) -> Vec<String> {
        let now = Instant::now();
        let ids: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| {
                let st = s
                    .snapshot
                    .lock()
                    .map(|g| g.status)
                    .unwrap_or(AgentStatus::Idle);
                matches!(st, AgentStatus::Completed | AgentStatus::Error)
                    && now.duration_since(s.created_at) > max_age
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &ids {
            self.clear_active_session_if(id);
            self.sessions.remove(id);
            log::info!("cleanup removed stale session {}", id);
        }
        ids
    }

    pub fn respond_permission_stub(
        &mut self,
        session_id: &str,
        tool_use_id: &str,
        _decision: &str,
    ) -> Result<(), String> {
        self.pending_permission
            .remove(&(session_id.to_string(), tool_use_id.to_string()));
        log::info!(
            "respond_permission (stub): session={} tool_use_id={}",
            session_id,
            tool_use_id
        );
        Ok(())
    }

    fn remember_session(&mut self, agent_type: &str, cwd: &str, session_id: &str) {
        if session_id.trim().is_empty() {
            return;
        }
        self.last_session_per_context.insert(
            (agent_type.to_string(), cwd.to_string()),
            session_id.to_string(),
        );
    }

    fn last_session_for(&self, agent_type: &str, cwd: &str) -> Option<String> {
        self.last_session_per_context
            .get(&(agent_type.to_string(), cwd.to_string()))
            .cloned()
    }
}

// ---------------------------------------------------------------------------
// 公共结果类型
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub session_id: String,
    pub status: AgentStatus,
}

// ---------------------------------------------------------------------------
// Claude Code Agent
// ---------------------------------------------------------------------------

pub fn launch_claude_agent(
    app: AppHandle,
    state: Arc<AppState>,
    runtime_state: Arc<RuntimeState>,
    cwd: String,
    task_zh: String,
) -> Result<LaunchResult, String> {
    let trimmed = task_zh.trim().to_string();
    if trimmed.is_empty() {
        return Err("任务内容不能为空".into());
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let sess = AgentSession::new(session_id.clone(), "claude-code".into(), Some(cwd.clone()));
    let stream_id = sess.stream_id.clone();
    {
        let mut sn = sess.snapshot.lock().map_err(|e| e.to_string())?;
        sn.last_user_prompt = Some(trimmed.clone());
        sn.status = AgentStatus::Running;
    }

    let resume_session_id = {
        let mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.last_session_for("claude-code", &cwd)
    };

    {
        let mut mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.active_session = Some(session_id.clone());
        mgr.sessions.insert(session_id.clone(), sess);
    }

    emit_status_running(&app, &session_id, "Claude Code starting");

    let app_handle = app.clone();
    let state_clone = Arc::clone(&state);
    let runtime_clone = Arc::clone(&runtime_state);
    let sid = session_id.clone();
    let user_zh = trimmed.clone();
    let cwd_owned = cwd.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let llm = load_llm_config();
        let prompt_for_agent = translate_input(&llm, &user_zh);
        let prefs = crate::agent::preferences::load_backend_preferences("claude-code");

        let turn_result = claude_agent::run_claude_stream_turn(
            &app_handle,
            runtime_clone.as_ref(),
            DEFAULT_RUN_ID,
            &prompt_for_agent,
            &cwd_owned,
            resume_session_id.as_deref(),
            prefs.model.as_deref(),
            prefs.effort.as_deref(),
            prefs.binary.as_deref(),
            prefs.proxy.as_deref(),
            Some(&stream_id),
        );

        match turn_result {
            Ok((next_session_id, output_en)) => {
                if let Some(next_sid) = next_session_id {
                    if let Ok(mut mgr) = state_clone.manager.lock() {
                        mgr.remember_session("claude-code", &cwd_owned, &next_sid);
                    }
                }
                finalize_session(
                    &app_handle,
                    &state_clone,
                    &sid,
                    &user_zh,
                    output_en,
                    llm.as_ref(),
                );
            }
            Err(error) => {
                fail_session(&app_handle, &state_clone, &sid, &error, "CLAUDE_TURN_FAILED");
            }
        }
    });

    Ok(LaunchResult {
        session_id,
        status: AgentStatus::Running,
    })
}

// ---------------------------------------------------------------------------
// Codex Agent
// ---------------------------------------------------------------------------

pub fn launch_codex_agent(
    app: AppHandle,
    state: Arc<AppState>,
    runtime_state: Arc<RuntimeState>,
    cwd: String,
    task_zh: String,
) -> Result<LaunchResult, String> {
    let trimmed = task_zh.trim().to_string();
    if trimmed.is_empty() {
        return Err("任务内容不能为空".into());
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let sess = AgentSession::new(session_id.clone(), "codex".into(), Some(cwd.clone()));
    let stream_id = sess.stream_id.clone();
    {
        let mut sn = sess.snapshot.lock().map_err(|e| e.to_string())?;
        sn.last_user_prompt = Some(trimmed.clone());
        sn.status = AgentStatus::Running;
    }

    let resume_thread_id = {
        let mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.last_session_for("codex", &cwd)
    };

    {
        let mut mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.active_session = Some(session_id.clone());
        mgr.sessions.insert(session_id.clone(), sess);
    }

    emit_status_running(&app, &session_id, "Codex App Server starting");

    let app_handle = app.clone();
    let state_clone = Arc::clone(&state);
    let runtime_clone = Arc::clone(&runtime_state);
    let sid = session_id.clone();
    let user_zh = trimmed.clone();
    let cwd_owned = cwd.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let llm = load_llm_config();
        let prompt_for_agent = translate_input(&llm, &user_zh);
        let prefs = crate::agent::preferences::load_backend_preferences("codex");

        let turn_result = codex_agent::run_codex_app_server_turn(
            &app_handle,
            runtime_clone.as_ref(),
            DEFAULT_RUN_ID,
            &cwd_owned,
            resume_thread_id.as_deref(),
            None, // system_prompt
            &prompt_for_agent,
            prefs.model.as_deref(),
            prefs.effort.as_deref(),
            prefs.binary.as_deref(),
            prefs.proxy.as_deref(),
            Some(&stream_id),
        );

        match turn_result {
            Ok((thread_id, output_en)) => {
                if let Ok(mut mgr) = state_clone.manager.lock() {
                    mgr.remember_session("codex", &cwd_owned, &thread_id);
                }
                finalize_session(
                    &app_handle,
                    &state_clone,
                    &sid,
                    &user_zh,
                    output_en,
                    llm.as_ref(),
                );
            }
            Err(error) => {
                fail_session(&app_handle, &state_clone, &sid, &error, "CODEX_TURN_FAILED");
            }
        }
    });

    Ok(LaunchResult {
        session_id,
        status: AgentStatus::Running,
    })
}

// ---------------------------------------------------------------------------
// OpenCode Agent
// ---------------------------------------------------------------------------

pub fn launch_opencode_agent(
    app: AppHandle,
    state: Arc<AppState>,
    runtime_state: Arc<RuntimeState>,
    cwd: String,
    task_zh: String,
) -> Result<LaunchResult, String> {
    let trimmed = task_zh.trim().to_string();
    if trimmed.is_empty() {
        return Err("任务内容不能为空".into());
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let sess = AgentSession::new(session_id.clone(), "opencode".into(), Some(cwd.clone()));
    let stream_id = sess.stream_id.clone();
    {
        let mut sn = sess.snapshot.lock().map_err(|e| e.to_string())?;
        sn.last_user_prompt = Some(trimmed.clone());
        sn.status = AgentStatus::Running;
    }

    let resume_session_id = {
        let mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.last_session_for("opencode", &cwd)
    };

    {
        let mut mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.active_session = Some(session_id.clone());
        mgr.sessions.insert(session_id.clone(), sess);
    }

    emit_status_running(&app, &session_id, "OpenCode server starting");

    let app_handle = app.clone();
    let state_clone = Arc::clone(&state);
    let runtime_clone = Arc::clone(&runtime_state);
    let sid = session_id.clone();
    let user_zh = trimmed.clone();
    let cwd_owned = cwd.clone();

    tauri::async_runtime::spawn(async move {
        let llm = load_llm_config();
        let llm_for_blocking = llm.clone();
        let user_zh_for_blocking = user_zh.clone();
        let prompt_for_agent = tauri::async_runtime::spawn_blocking(move || {
            translate_input(&llm_for_blocking, &user_zh_for_blocking)
        })
        .await
        .unwrap_or_else(|_| user_zh.clone());

        let prefs = crate::agent::preferences::load_backend_preferences("opencode");

        // 启动（或复用）OpenCode serve 子进程
        if let Err(error) = opencode_agent::opencode_start(
            &app_handle,
            runtime_clone.as_ref(),
            DEFAULT_RUN_ID,
            prefs.binary.as_deref(),
            prefs.proxy.as_deref(),
            None,
            Some(&cwd_owned),
        )
        .await
        {
            fail_session(
                &app_handle,
                &state_clone,
                &sid,
                &error,
                "OPENCODE_START_FAILED",
            );
            return;
        }

        // 复用 session_id 还是新建一个
        let session_for_turn = match resume_session_id {
            Some(existing) => existing,
            None => match opencode_agent::opencode_create_session(
                &app_handle,
                runtime_clone.as_ref(),
                DEFAULT_RUN_ID,
                None,
                Some(&cwd_owned),
            )
            .await
            {
                Ok(id) => id,
                Err(error) => {
                    fail_session(
                        &app_handle,
                        &state_clone,
                        &sid,
                        &error,
                        "OPENCODE_SESSION_FAILED",
                    );
                    return;
                }
            },
        };

        let turn_result = opencode_agent::run_opencode_turn(
            &app_handle,
            runtime_clone.as_ref(),
            DEFAULT_RUN_ID,
            &session_for_turn,
            &prompt_for_agent,
            None,
            Some(&cwd_owned),
            Some(&stream_id),
        )
        .await;

        match turn_result {
            Ok((output_en, _raw)) => {
                if let Ok(mut mgr) = state_clone.manager.lock() {
                    mgr.remember_session("opencode", &cwd_owned, &session_for_turn);
                }
                let app_for_finalize = app_handle.clone();
                let state_for_finalize = Arc::clone(&state_clone);
                let sid_for_finalize = sid.clone();
                let user_zh_for_finalize = user_zh.clone();
                let _ = tauri::async_runtime::spawn_blocking(move || {
                    finalize_session(
                        &app_for_finalize,
                        &state_for_finalize,
                        &sid_for_finalize,
                        &user_zh_for_finalize,
                        output_en,
                        llm.as_ref(),
                    );
                })
                .await;
            }
            Err(error) => {
                fail_session(
                    &app_handle,
                    &state_clone,
                    &sid,
                    &error,
                    "OPENCODE_TURN_FAILED",
                );
            }
        }
    });

    Ok(LaunchResult {
        session_id,
        status: AgentStatus::Running,
    })
}

// ---------------------------------------------------------------------------
// LLM 翻译/总结管线（输入翻译 + 输出翻译 + summary 生成）
// ---------------------------------------------------------------------------

fn translate_input(llm: &Option<LlmConfig>, zh: &str) -> String {
    match llm {
        Some(cfg) => translate_zh_to_en(cfg, zh).unwrap_or_else(|_| zh.to_string()),
        None => zh.to_string(),
    }
}

/// 处理 backend turn 的成功结果：英→中翻译 + LLM summary + emit complete + 状态归位。
fn finalize_session(
    app: &AppHandle,
    state: &Arc<AppState>,
    session_id: &str,
    user_zh: &str,
    result_en: String,
    llm: Option<&LlmConfig>,
) {
    let snapshot = match state.manager.lock() {
        Ok(mgr) => mgr
            .sessions
            .get(session_id)
            .map(|s| Arc::clone(&s.snapshot)),
        Err(_) => None,
    };

    let result_zh = match llm {
        Some(cfg) => translate_en_to_zh(cfg, &result_en).unwrap_or_else(|_| result_en.clone()),
        None => result_en.clone(),
    };

    let (mode, emotion, summary, suggestion_options) = match llm {
        Some(cfg) => match generate_agent_summary(cfg, user_zh, &result_zh) {
            Ok(s) => (
                Some(s.mode),
                Some(s.emotion_speech),
                Some(s.summary_translation),
                Some(s.next_options),
            ),
            Err(e) => (
                Some("error".into()),
                Some(format!("LLM 总结生成失败: {}", e)),
                Some(format!(
                    "Agent 原始输出:\n{}",
                    result_zh.chars().take(500).collect::<String>()
                )),
                Some(vec!["重试".into()]),
            ),
        },
        None => {
            let no_llm_hint = "未配置 LLM API Key（在设置中配置后，将自动总结 Agent 输出）";
            (
                Some("complete".into()),
                Some(no_llm_hint.into()),
                Some(result_zh.chars().take(500).collect::<String>()),
                Some(vec!["配置 API Key".into(), "重试".into()]),
            )
        }
    };

    if let Some(snap) = snapshot {
        if let Ok(mut s) = snap.lock() {
            s.status = match mode.as_deref() {
                Some("error") => AgentStatus::Error,
                _ => AgentStatus::Completed,
            };
            s.last_assistant_message = Some(result_zh.clone());
        }
    }

    let _ = app.emit(
        "agent://session-complete",
        SessionCompletePayload {
            session_id: session_id.to_string(),
            mode: mode.clone(),
            emotion: emotion.clone(),
            summary_translation: summary.clone(),
            result_raw: Some(result_en.clone()),
            result_zh: Some(result_zh.clone()),
            suggestion_options: suggestion_options.clone(),
        },
    );

    let _ = (result_en, result_zh, suggestion_options);
    clear_active_session(state, session_id);
}

fn fail_session(
    app: &AppHandle,
    state: &Arc<AppState>,
    session_id: &str,
    message: &str,
    code: &str,
) {
    if let Ok(mgr) = state.manager.lock() {
        if let Some(s) = mgr.sessions.get(session_id) {
            if let Ok(mut snap) = s.snapshot.lock() {
                snap.status = AgentStatus::Error;
                snap.last_assistant_message = Some(message.to_string());
            }
        }
    }
    emit_err(app, session_id, message, code);
    let _ = app.emit(
        "agent://session-complete",
        SessionCompletePayload {
            session_id: session_id.to_string(),
            mode: Some("error".into()),
            emotion: Some(format!("Agent 出错了: {}", message)),
            summary_translation: Some(message.to_string()),
            result_raw: None,
            result_zh: None,
            suggestion_options: Some(vec![]),
        },
    );
    clear_active_session(state, session_id);
}

fn emit_status_running(app: &AppHandle, session_id: &str, description: &str) {
    let _ = app.emit(
        "agent://status-changed",
        events::StatusChangedPayload {
            session_id: session_id.to_string(),
            status: AgentStatus::Running,
            tool_name: None,
            tool_description: Some(description.to_string()),
            percent: Some(0.0),
        },
    );
}

fn clear_active_session(state: &Arc<AppState>, session_id: &str) {
    if let Ok(mut mgr) = state.manager.lock() {
        mgr.clear_active_session_if(session_id);
    }
}

fn emit_err(app: &AppHandle, session_id: &str, message: &str, code: &str) {
    let _ = app.emit(
        "agent://error",
        events::ErrorPayload {
            session_id: session_id.to_string(),
            message: message.to_string(),
            code: code.to_string(),
        },
    );
}

// ---------------------------------------------------------------------------
// 停止会话
// ---------------------------------------------------------------------------

pub fn stop_session(
    app: AppHandle,
    state: Arc<AppState>,
    runtime_state: Arc<RuntimeState>,
    session_id: String,
) -> Result<(), String> {
    let snapshot = {
        let mut mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.clear_active_session_if(&session_id);
        let Some(sess) = mgr.sessions.get_mut(&session_id) else {
            return Err("会话不存在".into());
        };
        Arc::clone(&sess.snapshot)
    };
    if let Ok(mut s) = snapshot.lock() {
        s.interrupted = true;
        s.status = AgentStatus::Idle;
    }

    // claude / codex / opencode 的 client 在 RuntimeState 里。
    // 当前实现：不杀整个 client（避免影响其他可能正在跑的 turn 复用）。
    // app 退出时统一 drain_*_clients 清理。
    // 要单独中断当前 turn 需要给每个 backend 加 abort_turn 接口。
    let _ = runtime_state;

    let _ = app.emit(
        "agent://status-changed",
        events::StatusChangedPayload {
            session_id: session_id.clone(),
            status: AgentStatus::Idle,
            tool_name: None,
            tool_description: Some("stopped".into()),
            percent: None,
        },
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 杂项
// ---------------------------------------------------------------------------

pub fn get_logs(state: Arc<AppState>, session_id: String) -> Result<Vec<String>, String> {
    let mgr = state
        .manager
        .lock()
        .map_err(|_| "lock poisoned".to_string())?;
    let Some(s) = mgr.sessions.get(&session_id) else {
        return Err("会话不存在".into());
    };
    let g = s.logs.lock().map_err(|e| e.to_string())?;
    Ok(g.clone())
}

/// App 退出时清理所有 backend 子进程。
///
/// 退出阶段必须**阻塞拿锁**完成清理，try_lock 拿不到就跳过是漏杀子进程的主因。
/// 顺序：
///   1. drain 三个 backend 各自的 client/state，逐个 kill 子进程树
///   2. kill_opencode_listeners 兜底（防止 drain 漏掉的端口仍被占）
///   3. kill_all_direct_children 杀掉所有未注册到 state 的直系子进程
///   4. cleanup_stale_runtime_orphans 再扫一次 ppid==1 孤儿
pub fn shutdown_runtime_clients(app: &AppHandle) {
    use crate::agent::proc::{
        cleanup_stale_runtime_orphans, kill_all_direct_children, kill_child_descendants,
        kill_opencode_listeners,
    };
    use crate::agent::runtime::{drain_claude_clients, drain_codex_clients, drain_opencode_states};

    let runtime: tauri::State<Arc<RuntimeState>> = app.state();
    let runtime_state: &RuntimeState = runtime.inner().as_ref();

    // 多 tab 模式下遍历每个 run_id 各自的 OpencodeState，逐一杀掉子进程并收集端口。
    let mut opencode_ports: Vec<u16> = Vec::new();
    drain_opencode_states(runtime_state, |_run_id, opencode| {
        if let Some(child) = opencode.child.as_mut() {
            // 先递归杀 opencode 派生的子孙（node MCP servers、shell 工具等），
            // 再杀主进程。缺了这一步 grandchildren 会被 launchd 收养变残留。
            kill_child_descendants(child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
        opencode_ports.push(opencode.port);
        opencode.child = None;
        opencode.session_id = None;
        opencode.managed = false;
    });
    // OpenCode OAuth 回调用 1455 也加进来一起清
    opencode_ports.push(1455);
    let _ = kill_opencode_listeners(&opencode_ports);

    for client in drain_codex_clients(runtime_state) {
        client.stop();
    }

    for client in drain_claude_clients(runtime_state) {
        kill_claude_client(&client);
    }

    // 兜底：杀掉本进程的所有直系子进程（包括未进 state 的 warmup / probe 残留），
    // 退出前杀掉，否则它们会被 launchd 收养成 ppid==1 孤儿，那时已没人能扫它们。
    kill_all_direct_children();

    // 再扫一次 ppid==1 孤儿（上轮崩溃 / 强退留下的可能还在）
    cleanup_stale_runtime_orphans(app);
}

fn kill_claude_client(client: &ClaudeStreamClient) {
    crate::agent::claude::kill_claude_stream_client(client);
}
