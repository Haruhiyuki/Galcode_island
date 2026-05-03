use crate::session::state::{AgentStatus, SideEffect};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusChangedPayload {
    pub session_id: String,
    pub status: AgentStatus,
    pub tool_name: Option<String>,
    pub tool_description: Option<String>,
    pub percent: Option<f64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUpdatePayload {
    pub session_id: String,
    pub tool: String,
    pub description: Option<String>,
    pub tool_use_id: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultPayload {
    pub session_id: String,
    pub tool: String,
    pub success: bool,
    pub output: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestPayload {
    pub session_id: String,
    pub tool_name: String,
    pub tool_description: Option<String>,
    pub tool_use_id: String,
    pub raw_input: serde_json::Value,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPayload {
    pub session_id: String,
    pub level: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCompletePayload {
    pub session_id: String,
    pub mode: Option<String>,
    pub emotion: Option<String>,
    pub summary_translation: Option<String>,
    pub result_raw: Option<String>,
    pub result_zh: Option<String>,
    pub suggestion_options: Option<Vec<String>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub session_id: String,
    pub message: String,
    pub code: String,
}

pub fn apply_side_effects(app: &AppHandle, session_id: &str, effects: Vec<SideEffect>) {
    let sid = session_id.to_string();
    for fx in effects {
        match fx {
            SideEffect::EmitStatusChanged {
                status,
                tool_name,
                tool_description,
                percent,
            } => {
                let _ = app.emit(
                    "agent://status-changed",
                    StatusChangedPayload {
                        session_id: sid.clone(),
                        status,
                        tool_name,
                        tool_description,
                        percent,
                    },
                );
            }
            SideEffect::EmitToolUpdate {
                tool,
                description,
                tool_use_id,
            } => {
                let _ = app.emit(
                    "agent://tool-update",
                    ToolUpdatePayload {
                        session_id: sid.clone(),
                        tool,
                        description,
                        tool_use_id,
                    },
                );
            }
            SideEffect::EmitToolResult {
                tool,
                success,
                output,
            } => {
                let _ = app.emit(
                    "agent://tool-result",
                    ToolResultPayload {
                        session_id: sid.clone(),
                        tool,
                        success,
                        output,
                    },
                );
            }
            SideEffect::EmitPermissionRequest {
                tool_name,
                tool_description,
                tool_use_id,
                raw_input,
            } => {
                let _ = app.emit(
                    "agent://permission-request",
                    PermissionRequestPayload {
                        session_id: sid.clone(),
                        tool_name,
                        tool_description,
                        tool_use_id,
                        raw_input,
                    },
                );
            }
            SideEffect::EmitLog { level, message } => {
                let _ = app.emit(
                    "agent://log",
                    LogPayload {
                        session_id: sid.clone(),
                        level,
                        message,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    },
                );
            }
        }
    }
}
