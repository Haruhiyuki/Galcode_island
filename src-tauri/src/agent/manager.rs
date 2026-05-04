// Agent 会话管理器：start/stop/总结/翻译/IPC 事件聚合层。
//
// 设计：
//   - 三个 backend (Claude / OpenCode / Codex) 都通过对应 agent::xxx 模块完成 CLI 通信
//   - 本模块负责把每个 turn 套到 LLM 翻译/总结管线里：
//       中文 prompt → translate_zh_to_en → backend turn → 拿到英文输出
//       英文输出 → translate_en_to_zh → 中文 → generate_agent_summary → mode/emotion/options
//   - SessionSnapshot 状态由 IPC events 透传给前端宠物气泡
//   - 会话续接：每个 backend 自动捕获 session_id 存到 RuntimeState 里供下次 turn 复用
//
// 老 demo 路径保留（python scripts/demo_agent.py），用于不依赖外部 CLI 的烟测。

use super::config::preset_demo;
use super::launcher::{resolve_demo_script, spawn_demo_process};
use crate::agent::runtime::{ClaudeStreamClient, RuntimeState, DEFAULT_RUN_ID};
use crate::agent::{claude as claude_agent, codex as codex_agent, opencode as opencode_agent};
use crate::hook::event::HookEvent;
use crate::ipc::events::{self, SessionCompletePayload};
use crate::llm::{
    generate_agent_summary, load_llm_config, translate_en_to_zh, translate_zh_to_en, LlmConfig,
};
use crate::session::snapshot::SessionSnapshot;
use crate::session::state::{reduce_event, AgentStatus};
use crate::AppState;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

// ---------------------------------------------------------------------------
// 会话与管理器
// ---------------------------------------------------------------------------

pub struct AgentSession {
    pub snapshot: Arc<Mutex<SessionSnapshot>>,
    /// 仅 demo 路径用 — 其它 backend 的 client 在 RuntimeState 里管理。
    pub child: Arc<Mutex<Option<std::process::Child>>>,
    pub logs: Arc<Mutex<Vec<String>>>,
    pub created_at: Instant,
    /// 用于 cli-output 事件路由（参考项目里前端按 stream_id 把流式日志分发到正确 tab）。
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
            child: Arc::new(Mutex::new(None)),
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
    pub active_demo_session: Option<String>,
    /// 会话续接缓存：(agent_type, cwd) → 上次的 session_id / thread_id。
    /// 下次同 agent_type+cwd 提交时自动 resume，让对话有上下文延续。
    pub last_session_per_context: HashMap<(String, String), String>,
}

impl AgentManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            pending_permission: HashMap::new(),
            active_demo_session: None,
            last_session_per_context: HashMap::new(),
        }
    }

    pub fn clear_active_demo_session_if(&mut self, session_id: &str) {
        if self.active_demo_session.as_deref() == Some(session_id) {
            self.active_demo_session = None;
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
            self.clear_active_demo_session_if(id);
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
// Demo Agent (老 python 脚本，保留作为不依赖外部 CLI 的烟测)
// ---------------------------------------------------------------------------

pub fn launch_demo_agent(
    app: AppHandle,
    state: Arc<AppState>,
    cwd: String,
    task_zh: String,
) -> Result<LaunchResult, String> {
    let trimmed = task_zh.trim().to_string();
    if trimmed.is_empty() {
        return Err("任务内容不能为空".into());
    }

    let llm = load_llm_config();
    let task_for_agent = match &llm {
        Some(cfg) => translate_zh_to_en(cfg, &trimmed).unwrap_or_else(|_| trimmed.clone()),
        None => trimmed.clone(),
    };

    let cwd_path = std::path::PathBuf::from(&cwd);
    let script = resolve_demo_script();
    let cfg = preset_demo();

    let mut child = spawn_demo_process(&cfg, &cwd_path, &script, &task_for_agent)?;
    let pid = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 Agent stdout".to_string())?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let sess = AgentSession::new(session_id.clone(), "demo".into(), Some(cwd.clone()));
    {
        let mut sn = sess.snapshot.lock().map_err(|e| e.to_string())?;
        sn.pid = Some(pid);
        sn.last_user_prompt = Some(trimmed.clone());
        sn.status = AgentStatus::Running;
    }
    {
        let mut slot = sess.child.lock().map_err(|e| e.to_string())?;
        *slot = Some(child);
    }

    {
        let mut mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.active_demo_session = Some(session_id.clone());
        mgr.sessions.insert(session_id.clone(), sess);
    }

    let _ = app.emit(
        "agent://status-changed",
        events::StatusChangedPayload {
            session_id: session_id.clone(),
            status: AgentStatus::Running,
            tool_name: None,
            tool_description: Some("Demo Agent started".into()),
            percent: Some(0.0),
        },
    );

    let app_handle = app.clone();
    let state_clone = Arc::clone(&state);
    let sid = session_id.clone();
    std::thread::spawn(move || {
        run_stdout_loop(app_handle, state_clone, sid.clone(), stdout, trimmed, llm);
    });

    Ok(LaunchResult {
        session_id,
        status: AgentStatus::Running,
    })
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
        mgr.active_demo_session = Some(session_id.clone());
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

        let turn_result = claude_agent::run_claude_stream_turn(
            &app_handle,
            runtime_clone.as_ref(),
            DEFAULT_RUN_ID,
            &prompt_for_agent,
            &cwd_owned,
            resume_session_id.as_deref(),
            None, // model: 走默认（settings.json 或 ANTHROPIC_MODEL）
            None, // effort
            None, // binary
            None, // proxy
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
        mgr.active_demo_session = Some(session_id.clone());
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

        let turn_result = codex_agent::run_codex_app_server_turn(
            &app_handle,
            runtime_clone.as_ref(),
            DEFAULT_RUN_ID,
            &cwd_owned,
            resume_thread_id.as_deref(),
            None, // system_prompt
            &prompt_for_agent,
            None, // model
            None, // reasoning_effort
            None, // binary
            None, // proxy
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
        mgr.active_demo_session = Some(session_id.clone());
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

        // 启动（或复用）OpenCode serve 子进程
        if let Err(error) = opencode_agent::opencode_start(
            &app_handle,
            runtime_clone.as_ref(),
            DEFAULT_RUN_ID,
            None,
            None,
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

    // Legacy 兼容事件
    let _ = app.emit(
        "agent-done",
        serde_json::json!({
            "resultRaw": result_en,
            "resultZh": result_zh,
            "sessionId": session_id,
        }),
    );
    if let Some(opts) = suggestion_options {
        let _ = app.emit(
            "suggestion-ready",
            serde_json::json!({ "options": opts, "sessionId": session_id }),
        );
    }

    clear_active_demo_session(state, session_id);
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
    clear_active_demo_session(state, session_id);
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

// ---------------------------------------------------------------------------
// Demo 路径专用：stdout 行 → HookEvent → SideEffect 管线
// ---------------------------------------------------------------------------

fn push_log(logs: &Arc<Mutex<Vec<String>>>, line: String) {
    if let Ok(mut g) = logs.lock() {
        g.push(line);
        if g.len() > 500 {
            let drain = g.len() - 400;
            g.drain(0..drain);
        }
    }
}

fn run_stdout_loop(
    app: AppHandle,
    state: Arc<AppState>,
    session_id: String,
    stdout: std::process::ChildStdout,
    user_zh: String,
    llm: Option<LlmConfig>,
) {
    let (snapshot, logs, child_slot) = {
        let mgr = match state.manager.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(s) = mgr.sessions.get(&session_id) else {
            return;
        };
        (
            Arc::clone(&s.snapshot),
            Arc::clone(&s.logs),
            Arc::clone(&s.child),
        )
    };

    let reader = BufReader::new(stdout);
    let mut last_result_en: Option<String> = None;
    let mut agent_errored = false;

    for line in reader.lines().flatten() {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        push_log(&logs, line.clone());

        if let Some(ev) = HookEvent::from_json_line(&line) {
            if ev.event_name == "Stop" {
                let is_error = ev
                    .raw_json
                    .get("type")
                    .and_then(|x| x.as_str())
                    .map(|t| t == "error")
                    .unwrap_or(false);
                if is_error {
                    agent_errored = true;
                }
                last_result_en = ev
                    .raw_json
                    .get("output_en")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        ev.raw_json
                            .get("output")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string())
                    });
            }

            let effects = {
                let mut snap = match snapshot.lock() {
                    Ok(g) => g,
                    Err(_) => continue,
                };
                reduce_event(&mut snap, &ev)
            };
            crate::ipc::events::apply_side_effects(&app, &session_id, effects);

            // Legacy compatibility for existing UI listeners during migration
            legacy_emit_progress(&app, &session_id, &ev);
        } else {
            let _ = app.emit(
                "agent-progress",
                serde_json::json!({
                    "stage": "log",
                    "rawLine": line.clone(),
                    "sessionId": session_id,
                }),
            );
            let _ = app.emit(
                "agent://log",
                events::LogPayload {
                    session_id: session_id.clone(),
                    level: "debug".into(),
                    message: line.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                },
            );
        }
    }

    let mut guard = match child_slot.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if let Some(mut c) = guard.take() {
        let _ = c.wait();
    }

    let Some(result_en) = last_result_en else {
        if let Ok(mut s) = snapshot.lock() {
            s.status = AgentStatus::Error;
        }
        emit_err(
            &app,
            &session_id,
            "Agent 未返回结构化结果（缺少 type=result）",
            "MISSING_RESULT",
        );
        clear_active_demo_session(&state, &session_id);
        return;
    };

    if agent_errored {
        if let Ok(mut s) = snapshot.lock() {
            s.status = AgentStatus::Error;
            s.last_assistant_message = Some(result_en.clone());
        }
        emit_err(&app, &session_id, &result_en, "AGENT_ERROR");
        let _ = app.emit(
            "agent://session-complete",
            SessionCompletePayload {
                session_id: session_id.clone(),
                mode: Some("error".into()),
                emotion: Some(format!("Agent 出错了: {}", result_en)),
                summary_translation: Some(result_en),
                result_raw: None,
                result_zh: None,
                suggestion_options: Some(vec![]),
            },
        );
        clear_active_demo_session(&state, &session_id);
        return;
    }

    finalize_session(&app, &state, &session_id, &user_zh, result_en, llm.as_ref());
}

fn clear_active_demo_session(state: &Arc<AppState>, session_id: &str) {
    if let Ok(mut mgr) = state.manager.lock() {
        mgr.clear_active_demo_session_if(session_id);
    }
}

fn legacy_emit_progress(app: &AppHandle, session_id: &str, ev: &HookEvent) {
    if ev.event_name != "DemoProgress" {
        return;
    }
    let stage = ev
        .raw_json
        .get("stage")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let message = ev
        .raw_json
        .get("message")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let percent = ev.raw_json.get("percent").and_then(|v| v.as_f64());
    let _ = app.emit(
        "agent-progress",
        serde_json::json!({
            "stage": stage,
            "message": message,
            "percent": percent,
            "sessionId": session_id,
        }),
    );
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
    let _ = app.emit(
        "agent-error",
        serde_json::json!({ "message": message, "sessionId": session_id }),
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
    let (child_slot, agent_type) = {
        let mut mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.clear_active_demo_session_if(&session_id);
        let Some(sess) = mgr.sessions.get_mut(&session_id) else {
            return Err("会话不存在".into());
        };
        let agent_type = sess
            .snapshot
            .lock()
            .map(|s| s.agent_type.clone())
            .unwrap_or_default();
        if let Ok(mut s) = sess.snapshot.lock() {
            s.interrupted = true;
            s.status = AgentStatus::Idle;
        }
        (Arc::clone(&sess.child), agent_type)
    };

    // demo 路径直接 kill child
    {
        let mut g = child_slot.lock().map_err(|e| e.to_string())?;
        if let Some(mut c) = g.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    // claude / codex / opencode 的 client 在 RuntimeState 里。
    // 当前实现：不杀整个 client（避免影响其他可能正在跑的 turn 复用）。
    // app 退出时统一 drain_*_clients 清理。
    // 如果未来要单独中断当前 turn，可以在每个 backend 加 abort_turn 接口。
    let _ = (runtime_state, agent_type);

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

/// 用于 app 退出时清理所有 backend client。
pub fn shutdown_runtime_clients(runtime_state: &RuntimeState) {
    use crate::agent::runtime::{drain_claude_clients, drain_codex_clients, drain_opencode_states};

    for client in drain_claude_clients(runtime_state) {
        kill_claude_client(&client);
    }
    for client in drain_codex_clients(runtime_state) {
        client.stop();
    }
    drain_opencode_states(runtime_state, |_run_id, opencode| {
        if let Some(child) = opencode.child.as_mut() {
            crate::agent::sysutils::kill_child_descendants(child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
    });
}

fn kill_claude_client(client: &ClaudeStreamClient) {
    crate::agent::claude::kill_claude_stream_client(client);
}
