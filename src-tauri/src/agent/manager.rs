<<<<<<< Updated upstream
use super::config::preset_demo;
use super::launcher::{resolve_demo_script, spawn_demo_process};
=======
use super::config::{opencode_agent_config, preset_demo};
use super::launcher::{resolve_demo_script, spawn_demo_process, spawn_opencode_process};
>>>>>>> Stashed changes
use crate::hook::event::HookEvent;
use crate::ipc::events::{self, SessionCompletePayload};
use crate::llm::{
    generate_summary_emotion, load_llm_config, suggest_next_steps, translate_en_to_zh,
    translate_zh_to_en, LlmConfig,
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
<<<<<<< Updated upstream
=======
            StdoutAgentKind::Demo,
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
    let task_for_agent = match &llm {
        Some(cfg) => translate_zh_to_en(cfg, &trimmed).unwrap_or_else(|_| trimmed.clone()),
        None => trimmed.clone(),
    };

    let cwd_path = PathBuf::from(&cwd);
    let cfg = opencode_agent_config(&app);

    let mut child = spawn_opencode_process(&cfg, &cwd_path, &task_for_agent).map_err(|e| {
        format!(
            "{}。提示：可设置环境变量 OPENCODE_BIN 或 GALCODE_OPENCODE_BIN，或在应用配置目录创建 opencode_executable.txt（单行 OpenCode CLI 绝对路径），或在项目根目录使用 .env。",
            e
        )
    })?;
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
            tool_description: Some("OpenCode agent started".into()),
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
            StdoutAgentKind::OpenCode,
>>>>>>> Stashed changes
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

    for line in reader.lines().flatten() {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        push_log(&logs, line.clone());

        if let Some(ev) = HookEvent::from_json_line(&line) {
            if ev.event_name == "Stop" {
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

    let result_zh = match &llm {
        Some(cfg) => translate_en_to_zh(cfg, &result_en).unwrap_or_else(|_| result_en.clone()),
        None => result_en.clone(),
    };

    let (summary, emotion, suggestion_zh) = match &llm {
        Some(cfg) => match generate_summary_emotion(cfg, &user_zh, &result_zh) {
            Ok(s) => (
                Some(s.summary.clone()),
                Some(s.emotion.clone()),
                suggest_next_steps(cfg, &user_zh, &result_zh).ok(),
            ),
            Err(e) => (
                Some(result_zh.chars().take(400).collect::<String>()),
                Some(format!("总结生成失败: {}", e)),
                suggest_next_steps(cfg, &user_zh, &result_zh).ok(),
            ),
        },
        None => (
            Some(result_zh.chars().take(400).collect::<String>()),
            Some("任务完成！（未配置 LLM_API_KEY，使用本地占位文案）".into()),
            Some(
                "（未配置 LLM_API_KEY）跳过智能建议。配置环境变量后可获得后续改进提示。".into(),
            ),
        ),
    };

    if let Ok(mut s) = snapshot.lock() {
        s.status = AgentStatus::Completed;
        s.last_assistant_message = Some(result_zh.clone());
    }

    let _ = app.emit(
        "agent://session-complete",
        SessionCompletePayload {
            session_id: session_id.clone(),
            summary: summary.clone(),
            emotion: emotion.clone(),
            result_raw: Some(result_en.clone()),
            result_zh: Some(result_zh.clone()),
            suggestion_zh: suggestion_zh.clone(),
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
    if let Some(sz) = suggestion_zh {
        let _ = app.emit(
            "suggestion-ready",
            serde_json::json!({ "textZh": sz, "sessionId": session_id }),
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
