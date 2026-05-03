use super::config::preset_demo;
use super::launcher::{
    resolve_demo_script, resolve_opencode_cli_executable, spawn_demo_process, spawn_opencode_process,
};
use crate::hook::event::{stop_output_from_raw, HookEvent};
use crate::ipc::events::{self, SessionCompletePayload, SummaryReadyPayload};
use crate::llm::{
    generate_agent_summary, load_llm_config, LlmConfig,
};
use crate::session::snapshot::SessionSnapshot;
use crate::session::state::{reduce_event, AgentStatus};
use crate::AppState;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
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
    let task_for_agent = trimmed.clone();

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

    let llm = load_llm_config();
    let task_for_agent = trimmed.clone();

    let cwd_path = PathBuf::from(&cwd);
    let opencode_exe = resolve_opencode_cli_executable()?;

    let mut child = spawn_opencode_process(&opencode_exe, &cwd_path, &task_for_agent)?;
    let pid = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 OpenCode stdout".to_string())?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let sess = AgentSession::new(session_id.clone(), "opencode".into(), Some(cwd.clone()));
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
            tool_description: Some("OpenCode started".into()),
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub session_id: String,
    pub status: AgentStatus,
}

/// Last JSONL line that parses as Stop with extractable body (same rules as main loop).
fn best_stop_from_captured_lines(lines: &[String]) -> Option<String> {
    for line in lines.iter().rev() {
        if let Some(ev) = HookEvent::from_json_line(line) {
            if matches!(ev.event_name.as_str(), "Stop" | "OpenCodeStreamText") {
                if let Some(s) = stop_output_from_raw(&ev.raw_json) {
                    return Some(s);
                }
            }
        }
    }
    None
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

    let mut bufreader = BufReader::new(stdout);
    let mut line_bytes: Vec<u8> = Vec::new();
    let mut last_result_en: Option<String> = None;

    loop {
        line_bytes.clear();
        match bufreader.read_until(b'\n', &mut line_bytes) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                log::warn!("read agent stdout: {}", e);
                break;
            }
        }
        let line = String::from_utf8_lossy(&line_bytes);
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        push_log(&logs, line.clone());

        if let Some(ev) = HookEvent::from_json_line(&line) {
            if matches!(ev.event_name.as_str(), "Stop" | "OpenCodeStreamText") {
                match stop_output_from_raw(&ev.raw_json) {
                    Some(body) => last_result_en = Some(body),
                    None => {}
                }
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

    if last_result_en.is_none() {
        if let Ok(g) = logs.lock() {
            last_result_en = best_stop_from_captured_lines(&g);
        }
    }

    let mut guard = match child_slot.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let child_exit: Option<i32> = if let Some(mut c) = guard.take() {
        c.wait().ok().and_then(|st| st.code())
    } else {
        None
    };

    let result_en = last_result_en.or_else(|| {
        snapshot
            .lock()
            .ok()
            .and_then(|g| g.last_assistant_message.clone())
            .filter(|s| !s.trim().is_empty())
    });

    let Some(result_en) = result_en else {
        if let Ok(mut s) = snapshot.lock() {
            s.status = AgentStatus::Error;
        }
        let mut detail = "Agent 未返回可解析的输出。Demo 需至少一行 JSON：`{\"type\":\"result\",\"output_en\":\"...\"}`；或 Hook 的 `Stop` 需含 output_en / output / last_assistant_message 等字段之一。".to_string();
        if let Some(c) = child_exit {
            use std::fmt::Write;
            let _ = write!(&mut detail, "\n子进程退出码: {c}。");
        }
        if let Ok(g) = logs.lock() {
            if !g.is_empty() {
                use std::fmt::Write;
                let _ = write!(&mut detail, "\n已记录 stdout（末 6 行，可能含非 JSON 提示）：\n");
                for ln in g.iter().rev().take(6).rev() {
                    if detail.len() > 3200 {
                        break;
                    }
                    let _ = write!(&mut detail, "{ln}\n");
                }
            }
        } else {
            detail.push_str("\n（无 stdout 记录。）");
        }
        emit_err(&app, &session_id, &detail, "MISSING_RESULT");
        clear_active_demo_session(&state, &session_id);
        return;
    };

    let result_zh = result_en.clone();

    // Update snapshot immediately
    if let Ok(mut s) = snapshot.lock() {
        s.status = AgentStatus::Completed;
        s.last_assistant_message = Some(result_zh.clone());
    }

    // Emit completion immediately — don't block on LLM summarization
    let _ = app.emit(
        "agent://session-complete",
        SessionCompletePayload {
            session_id: session_id.clone(),
            mode: Some("complete".into()),
            emotion: Some("任务完成！".into()),
            summary_translation: None,
            result_raw: Some(result_en.clone()),
            result_zh: Some(result_zh.clone()),
            suggestion_options: None,
        },
    );

    let _ = app.emit(
        "agent-done",
        serde_json::json!({
            "resultRaw": result_en.clone(),
            "resultZh": result_zh.clone(),
            "sessionId": session_id,
        }),
    );

    // Spawn LLM summarization in background thread so it doesn't block UI
    if let Some(cfg) = llm {
        let app2 = app.clone();
        let sid = session_id.clone();
        let user = user_zh.clone();
        let output = result_zh.clone();
        std::thread::spawn(move || {
            match generate_agent_summary(&cfg, &user, &output) {
                Ok(s) => {
                    let _ = app2.emit(
                        "agent://summary-ready",
                        SummaryReadyPayload {
                            session_id: sid.clone(),
                            mode: s.mode.clone(),
                            emotion: s.emotion_speech.clone(),
                            summary_translation: Some(s.summary_translation.clone()),
                            suggestion_options: s.next_options.clone(),
                        },
                    );
                    if !s.next_options.is_empty() {
                        let _ = app2.emit(
                            "suggestion-ready",
                            serde_json::json!({ "options": s.next_options, "sessionId": sid }),
                        );
                    }
                }
                Err(e) => {
                    let _ = app2.emit(
                        "agent://summary-ready",
                        SummaryReadyPayload {
                            session_id: sid.clone(),
                            mode: "error".into(),
                            emotion: format!("总结生成失败: {e}"),
                            summary_translation: None,
                            suggestion_options: vec![],
                        },
                    );
                }
            }
        });
    } else {
        let _ = app.emit(
            "agent://summary-ready",
            SummaryReadyPayload {
                session_id: session_id.clone(),
                mode: "complete".into(),
                emotion: "任务完成！（未配置 LLM）".into(),
                summary_translation: None,
                suggestion_options: vec![],
            },
        );
    }

    clear_active_demo_session(&state, &session_id);
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
