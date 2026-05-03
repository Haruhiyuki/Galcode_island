use super::config::{preset_demo, preset_opencode};
use super::launcher::{read_new_lines, resolve_demo_script, spawn_demo_process, spawn_opencode_terminal};
use crate::hook::event::HookEvent;
use crate::ipc::events::{self, SessionCompletePayload};
use crate::llm::{
    generate_agent_summary, load_llm_config, translate_en_to_zh,
    translate_zh_to_en, LlmConfig,
};
use crate::session::snapshot::SessionSnapshot;
use crate::session::state::{reduce_event, AgentStatus};
use crate::AppState;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub struct AgentSession {
    pub snapshot: Arc<Mutex<SessionSnapshot>>,
    pub child: Arc<Mutex<Option<std::process::Child>>>,
    pub logs: Arc<Mutex<Vec<String>>>,
    pub created_at: Instant,
}

impl AgentSession {
    pub fn new(session_id: String, agent_type: String, cwd: Option<String>) -> Self {
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
        }
    }
}

pub struct AgentManager {
    pub sessions: HashMap<String, AgentSession>,
    pending_permission: HashMap<(String, String), ()>,
    /// Last demo session started via `start_agent` / `launch_agent` (for plan-compat `stop_agent` without id).
    pub active_demo_session: Option<String>,
}

impl AgentManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            pending_permission: HashMap::new(),
            active_demo_session: None,
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
                let st = s.snapshot.lock().map(|g| g.status).unwrap_or(AgentStatus::Idle);
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
}

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

    let cwd_path = PathBuf::from(&cwd);
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
        run_stdout_loop(
            app_handle,
            state_clone,
            sid.clone(),
            stdout,
            trimmed,
            llm,
        );
    });

    Ok(LaunchResult {
        session_id,
        status: AgentStatus::Running,
    })
}

pub fn launch_opencode_agent(
    app: AppHandle,
    state: Arc<AppState>,
    cwd: String,
    task_zh: String,
) -> Result<LaunchResult, String> {
    let trimmed = task_zh.trim().to_string();
    if trimmed.is_empty() {
        return Err("任务内容不能为空".into());
    }

    eprintln!("[galcode] launch_opencode_agent: cwd={}, task={}", cwd, trimmed);

    let llm = load_llm_config();
    let task_for_agent = match &llm {
        Some(cfg) => translate_zh_to_en(cfg, &trimmed).unwrap_or_else(|_| trimmed.clone()),
        None => trimmed.clone(),
    };

    eprintln!("[galcode] task_for_agent={}", task_for_agent);

    let cwd_path = PathBuf::from(&cwd);
    let cfg = preset_opencode();

    // Spawn the agent in a visible terminal; output is tee'd to a temp file
    eprintln!("[galcode] calling spawn_opencode_terminal...");
    let launch = spawn_opencode_terminal(&cfg, &cwd_path, &task_for_agent)?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let agent_type = "opencode".to_string();
    let sess = AgentSession::new(session_id.clone(), agent_type, Some(cwd.clone()));
    {
        let mut sn = sess.snapshot.lock().map_err(|e| e.to_string())?;
        sn.last_user_prompt = Some(trimmed.clone());
        sn.status = AgentStatus::Running;
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
            tool_description: Some("OpenCode agent started in terminal".into()),
            percent: Some(0.0),
        },
    );

    let app_handle = app.clone();
    let state_clone = Arc::clone(&state);
    let sid = session_id.clone();
    let output_file = launch.output_file.clone();
    let script_file = launch.script_file.clone();

    std::thread::spawn(move || {
        run_file_monitor(
            app_handle,
            state_clone,
            sid.clone(),
            output_file,
            script_file,
            trimmed,
            llm,
        );
    });

    Ok(LaunchResult {
        session_id,
        status: AgentStatus::Running,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub session_id: String,
    pub status: AgentStatus,
}

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

    // When agent itself errored, skip LLM pipeline and show error directly
    if agent_errored {
        if let Ok(mut s) = snapshot.lock() {
            s.status = AgentStatus::Error;
            s.last_assistant_message = Some(result_en.clone());
        }
        emit_err(&app, &session_id, &result_en, "AGENT_ERROR");
        // Also emit session-complete with error mode so frontend shows ResultCard
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

    let result_zh = match &llm {
        Some(cfg) => translate_en_to_zh(cfg, &result_en).unwrap_or_else(|_| result_en.clone()),
        None => result_en.clone(),
    };

    let (mode, emotion, summary, suggestion_options) = match &llm {
        Some(cfg) => match generate_agent_summary(cfg, &user_zh, &result_zh) {
            Ok(s) => (
                Some(s.mode.clone()),
                Some(s.emotion_speech.clone()),
                Some(s.summary_translation.clone()),
                Some(s.next_options.clone()),
            ),
            Err(e) => (
                Some("error".into()),
                Some(format!("LLM 总结生成失败: {}", e)),
                Some(format!("Agent 原始输出:\n{}", result_zh.chars().take(500).collect::<String>())),
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

    if let Ok(mut s) = snapshot.lock() {
        s.status = match mode.as_deref() {
            Some("error") => AgentStatus::Error,
            _ => AgentStatus::Completed,
        };
        s.last_assistant_message = Some(result_zh.clone());
    }

    let _ = app.emit(
        "agent://session-complete",
        SessionCompletePayload {
            session_id: session_id.clone(),
            mode: mode.clone(),
            emotion: emotion.clone(),
            summary_translation: summary.clone(),
            result_raw: Some(result_en.clone()),
            result_zh: Some(result_zh.clone()),
            suggestion_options: suggestion_options.clone(),
        },
    );

    // Legacy events
    let _ = app.emit(
        "agent-done",
        serde_json::json!({
            "resultRaw": result_en,
            "resultZh": result_zh,
            "sessionId": session_id,
        }),
    );
    if let Some(opts) = suggestion_options {
        // You could emit suggestion-ready with the first option, or change the frontend to expect an array.
        // For backwards compatibility we can just join them or pass the array.
        let _ = app.emit(
            "suggestion-ready",
            serde_json::json!({ "options": opts, "sessionId": session_id }),
        );
    }

    clear_active_demo_session(&state, &session_id);
}

/// Poll the JSONL output file for new lines, parse events, and emit IPC updates.
/// This replaces the stdout pipe approach — the agent runs in a visible terminal
/// and writes its output to both the terminal and this file (via Tee).
fn run_file_monitor(
    app: AppHandle,
    state: Arc<AppState>,
    session_id: String,
    output_file: PathBuf,
    script_file: PathBuf,
    user_zh: String,
    llm: Option<LlmConfig>,
) {
    let (snapshot, logs) = {
        let mgr = match state.manager.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(s) = mgr.sessions.get(&session_id) else {
            return;
        };
        (Arc::clone(&s.snapshot), Arc::clone(&s.logs))
    };

    let mut line_count: usize = 0;
    let mut last_result_en: Option<String> = None;
    let mut agent_errored = false;
    let max_idle = Duration::from_secs(600); // 10 min timeout with no output
    let poll_interval = Duration::from_millis(500);
    let mut idle_since = Instant::now();
    let start = Instant::now();

    // Wait a moment for the terminal to open and the file to be created
    std::thread::sleep(Duration::from_secs(2));

    loop {
        match read_new_lines(&output_file, &mut line_count) {
            Ok(lines) => {
                if lines.is_empty() {
                    // No new lines — check timeout
                    if idle_since.elapsed() > max_idle && start.elapsed() > Duration::from_secs(15) {
                        if last_result_en.is_none() {
                            emit_err(
                                &app,
                                &session_id,
                                "Agent 超时：终端无新输出（10分钟）",
                                "TIMEOUT",
                            );
                        }
                        break;
                    }
                    std::thread::sleep(poll_interval);
                    continue;
                }
                idle_since = Instant::now();

                for line in lines {
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
            }
            Err(e) => {
                log::warn!("read_new_lines error: {}", e);
                std::thread::sleep(poll_interval);
                continue;
            }
        }

        // Check if agent has finished (saw a result/error)
        if last_result_en.is_some() {
            // Give it a bit more time for the terminal to flush
            std::thread::sleep(Duration::from_secs(1));
            break;
        }

        std::thread::sleep(poll_interval);
    }

    // --- Process result (same logic as run_stdout_loop) ---

    let Some(result_en) = last_result_en else {
        if let Ok(mut s) = snapshot.lock() {
            s.status = AgentStatus::Error;
        }
        emit_err(
            &app,
            &session_id,
            "Agent 未返回结构化结果（终端可能已关闭）",
            "MISSING_RESULT",
        );
        cleanup_temp_files(&output_file, &script_file);
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
        cleanup_temp_files(&output_file, &script_file);
        clear_active_demo_session(&state, &session_id);
        return;
    }

    let result_zh = match &llm {
        Some(cfg) => translate_en_to_zh(cfg, &result_en).unwrap_or_else(|_| result_en.clone()),
        None => result_en.clone(),
    };

    let (mode, emotion, summary, suggestion_options) = match &llm {
        Some(cfg) => match generate_agent_summary(cfg, &user_zh, &result_zh) {
            Ok(s) => (
                Some(s.mode.clone()),
                Some(s.emotion_speech.clone()),
                Some(s.summary_translation.clone()),
                Some(s.next_options.clone()),
            ),
            Err(e) => (
                Some("error".into()),
                Some(format!("LLM 总结生成失败: {}", e)),
                Some(format!("Agent 原始输出:\n{}", result_zh.chars().take(500).collect::<String>())),
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

    if let Ok(mut s) = snapshot.lock() {
        s.status = match mode.as_deref() {
            Some("error") => AgentStatus::Error,
            _ => AgentStatus::Completed,
        };
        s.last_assistant_message = Some(result_zh.clone());
    }

    let _ = app.emit(
        "agent://session-complete",
        SessionCompletePayload {
            session_id: session_id.clone(),
            mode: mode.clone(),
            emotion: emotion.clone(),
            summary_translation: summary.clone(),
            result_raw: Some(result_en.clone()),
            result_zh: Some(result_zh.clone()),
            suggestion_options: suggestion_options.clone(),
        },
    );

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

    cleanup_temp_files(&output_file, &script_file);
    clear_active_demo_session(&state, &session_id);
}

fn cleanup_temp_files(output_file: &PathBuf, script_file: &PathBuf) {
    if let Err(e) = std::fs::remove_file(output_file) {
        log::debug!("Failed to remove output file {}: {}", output_file.display(), e);
    }
    if let Err(e) = std::fs::remove_file(script_file) {
        log::debug!("Failed to remove script file {}: {}", script_file.display(), e);
    }
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

pub fn stop_session(app: AppHandle, state: Arc<AppState>, session_id: String) -> Result<(), String> {
    let child_slot = {
        let mut mgr = state.manager.lock().map_err(|e| e.to_string())?;
        mgr.clear_active_demo_session_if(&session_id);
        let Some(sess) = mgr.sessions.get_mut(&session_id) else {
            return Err("会话不存在".into());
        };
        if let Ok(mut s) = sess.snapshot.lock() {
            s.interrupted = true;
            s.status = AgentStatus::Idle;
        }
        Arc::clone(&sess.child)
    };

    {
        let mut g = child_slot.lock().map_err(|e| e.to_string())?;
        if let Some(mut c) = g.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

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
