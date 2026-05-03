use super::snapshot::{SessionSnapshot, ToolHistoryEntry};
use crate::hook::event::{stop_output_from_raw, HookEvent};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentStatus {
    Idle,
    Starting,
    Running,
    Thinking,
    Processing,
    WaitingApproval,
    Completed,
    Error,
}

#[derive(Debug, Clone)]
pub enum SideEffect {
    EmitStatusChanged {
        status: AgentStatus,
        tool_name: Option<String>,
        tool_description: Option<String>,
        percent: Option<f64>,
    },
    EmitToolUpdate {
        tool: String,
        description: Option<String>,
        tool_use_id: Option<String>,
    },
    EmitToolResult {
        tool: String,
        success: bool,
        output: Option<String>,
    },
    EmitPermissionRequest {
        tool_name: String,
        tool_description: Option<String>,
        tool_use_id: String,
        raw_input: serde_json::Value,
    },
    EmitLog {
        level: String,
        message: String,
    },
}

pub fn reduce_event(snapshot: &mut SessionSnapshot, event: &HookEvent) -> Vec<SideEffect> {
    snapshot.last_activity = Utc::now();
    let mut effects = Vec::new();

    let name = event.event_name.as_str();

    match name {
        "SessionStart" => {
            snapshot.status = AgentStatus::Running;
            effects.push(SideEffect::EmitStatusChanged {
                status: snapshot.status,
                tool_name: None,
                tool_description: None,
                percent: None,
            });
        }
        "UserPromptSubmit" => {
            snapshot.status = AgentStatus::Processing;
            effects.push(SideEffect::EmitStatusChanged {
                status: snapshot.status,
                tool_name: None,
                tool_description: None,
                percent: None,
            });
        }
        "OpenCodeStreamText" => {
            let msg = event
                .raw_json
                .get("part")
                .and_then(|p| p.get("text"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if !msg.is_empty() {
                snapshot.status = AgentStatus::Processing;
                snapshot.last_assistant_message = Some(msg.clone());
                effects.push(SideEffect::EmitStatusChanged {
                    status: snapshot.status,
                    tool_name: None,
                    tool_description: Some(
                        msg.chars().take(200).collect::<String>()
                            + if msg.chars().count() > 200 {
                                "…"
                            } else {
                                ""
                            },
                    ),
                    percent: None,
                });
                effects.push(SideEffect::EmitLog {
                    level: "info".into(),
                    message: msg,
                });
            }
        }
        "OpenCodeStepStart" => {
            snapshot.status = AgentStatus::Thinking;
            effects.push(SideEffect::EmitStatusChanged {
                status: snapshot.status,
                tool_name: None,
                tool_description: Some("OpenCode step…".into()),
                percent: None,
            });
            effects.push(SideEffect::EmitLog {
                level: "info".into(),
                message: "OpenCode: step_start".into(),
            });
        }
        "OpenCodeStepFinish" => {
            snapshot.status = AgentStatus::Processing;
            effects.push(SideEffect::EmitStatusChanged {
                status: snapshot.status,
                tool_name: None,
                tool_description: None,
                percent: None,
            });
            effects.push(SideEffect::EmitLog {
                level: "info".into(),
                message: "OpenCode: step_finish".into(),
            });
        }
        "OpenCodeStepFinishStop" => {
            snapshot.status = AgentStatus::Completed;
            effects.push(SideEffect::EmitStatusChanged {
                status: snapshot.status,
                tool_name: None,
                tool_description: snapshot.last_assistant_message.clone(),
                percent: Some(100.0),
            });
            effects.push(SideEffect::EmitLog {
                level: "info".into(),
                message: "OpenCode: step_finish (stop)".into(),
            });
        }
        "DemoProgress" => {
            let stage = event
                .raw_json
                .get("stage")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            snapshot.status = match stage {
                "thinking" => AgentStatus::Thinking,
                "working" | "executing" => AgentStatus::Processing,
                "done" => AgentStatus::Completed,
                _ => AgentStatus::Running,
            };
            let pct = event.raw_json.get("percent").and_then(|v| v.as_f64());
            let msg = event
                .raw_json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            effects.push(SideEffect::EmitStatusChanged {
                status: snapshot.status,
                tool_name: None,
                tool_description: if msg.is_empty() {
                    None
                } else {
                    Some(msg.clone())
                },
                percent: pct,
            });
            if !msg.is_empty() {
                effects.push(SideEffect::EmitLog {
                    level: "info".into(),
                    message: msg,
                });
            }
        }
        "PreToolUse" => {
            snapshot.status = AgentStatus::Processing;
            snapshot.current_tool = event.tool_name.clone();
            snapshot.tool_description = event
                .tool_input
                .as_ref()
                .map(|v| v.to_string())
                .or_else(|| {
                    event
                        .raw_json
                        .get("tool_description")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                });
            let desc = snapshot.tool_description.clone();
            if let Some(ref tool) = event.tool_name {
                snapshot.tool_history.push(ToolHistoryEntry {
                    tool: tool.clone(),
                    description: desc.clone(),
                    timestamp: Utc::now(),
                    success: true,
                });
                effects.push(SideEffect::EmitToolUpdate {
                    tool: tool.clone(),
                    description: desc.clone(),
                    tool_use_id: event.tool_use_id.clone(),
                });
            }
            effects.push(SideEffect::EmitStatusChanged {
                status: snapshot.status,
                tool_name: event.tool_name.clone(),
                tool_description: desc,
                percent: None,
            });
        }
        "PostToolUse" => {
            snapshot.status = AgentStatus::Processing;
            let tool = event.tool_name.clone().unwrap_or_else(|| "Tool".into());
            effects.push(SideEffect::EmitToolResult {
                tool,
                success: true,
                output: None,
            });
        }
        "PermissionRequest" => {
            snapshot.status = AgentStatus::WaitingApproval;
            if let (Some(tool), Some(id)) = (&event.tool_name, &event.tool_use_id) {
                effects.push(SideEffect::EmitPermissionRequest {
                    tool_name: tool.clone(),
                    tool_description: snapshot.tool_description.clone(),
                    tool_use_id: id.clone(),
                    raw_input: event.tool_input.clone().unwrap_or(event.raw_json.clone()),
                });
            }
            effects.push(SideEffect::EmitStatusChanged {
                status: snapshot.status,
                tool_name: event.tool_name.clone(),
                tool_description: snapshot.tool_description.clone(),
                percent: None,
            });
        }
        "Stop" => {
            snapshot.status = AgentStatus::Completed;
            let out = stop_output_from_raw(&event.raw_json);
            if let Some(ref text) = out {
                snapshot.last_assistant_message = Some(text.clone());
            }
            effects.push(SideEffect::EmitStatusChanged {
                status: snapshot.status,
                tool_name: None,
                tool_description: out.clone(),
                percent: Some(100.0),
            });
        }
        "Notification" => {
            let msg = event
                .raw_json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !msg.is_empty() {
                effects.push(SideEffect::EmitLog {
                    level: "info".into(),
                    message: msg,
                });
            }
        }
        _ => {
            effects.push(SideEffect::EmitLog {
                level: "debug".into(),
                message: format!("unhandled hook event: {}", name),
            });
        }
    }

    effects
}
