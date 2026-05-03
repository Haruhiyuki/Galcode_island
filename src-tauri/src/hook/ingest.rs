//! Apply hook JSON (from OpenCode plugin HTTP POST) into session state and IPC events.

use crate::agent::manager::{AgentSession, finalize_external_stop};
use crate::hook::event::HookEvent;
use crate::ipc::events::apply_side_effects;
use crate::session::state::{reduce_event, AgentStatus};
use crate::AppState;
use std::sync::Arc;
use tauri::AppHandle;

pub fn process_hook_json(app: &AppHandle, state: &Arc<AppState>, line: &str) {
    let Some(ev) = HookEvent::from_json_line(line.trim()) else {
        log::debug!(
            "hook ingest: skipped non-hook JSON {}",
            line.chars().take(120).collect::<String>()
        );
        return;
    };

    let sid = ev
        .session_id
        .clone()
        .unwrap_or_else(|| "external-unknown".to_string());

    if ev.event_name == "PermissionRequest" {
        if let (Some(session_key), Some(req)) = (
            ev.session_id.as_ref(),
            ev.tool_use_id.as_deref().or_else(|| {
                ev.raw_json
                    .get("_opencode_request_id")
                    .and_then(|x| x.as_str())
            }),
        ) {
            if let Some(port) = ev.raw_json.get("_server_port").and_then(|x| x.as_u64()) {
                if let Ok(mut g) = state.manager.lock() {
                    g.permission_opencode_ports
                        .insert((session_key.clone(), req.to_string()), port as u16);
                }
            }
        }
    }

    let effects = {
        let mut mgr = match state.manager.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        mgr.sessions
            .entry(sid.clone())
            .or_insert_with(|| AgentSession::new(sid.clone(), "opencode".into(), ev.cwd.clone()));
        let sess = mgr.sessions.get(&sid).unwrap();
        let mut snap = match sess.snapshot.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if snap.status == AgentStatus::Starting {
            snap.status = AgentStatus::Running;
        }
        reduce_event(&mut snap, &ev)
    };

    apply_side_effects(app, &sid, effects);

    if ev.event_name == "Stop" {
        finalize_external_stop(app, state, &sid);
    }
}
