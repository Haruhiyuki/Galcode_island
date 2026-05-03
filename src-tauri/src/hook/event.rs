use super::normalizer::normalize_event_name;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEvent {
    pub event_name: String,
    pub session_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_use_id: Option<String>,
    pub tool_input: Option<Value>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub agent_id: Option<String>,
    pub raw_json: Value,
}

impl HookEvent {
    pub fn from_json_line(line: &str) -> Option<Self> {
        let v: Value = serde_json::from_str(line.trim()).ok()?;

        if let Some(raw_name) = v.get("hook_event_name").and_then(|x| x.as_str()) {
            let event_name = normalize_event_name(raw_name);
            return Some(HookEvent {
                event_name,
                session_id: v
                    .get("session_id")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                tool_name: v
                    .get("tool_name")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                tool_use_id: v
                    .get("tool_use_id")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                tool_input: v.get("tool_input").cloned(),
                cwd: v
                    .get("cwd")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                model: v
                    .get("model")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                agent_id: v
                    .get("agent_id")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                raw_json: v,
            });
        }

        if v.get("type").and_then(|t| t.as_str()) == Some("result") {
            return Some(HookEvent {
                event_name: "Stop".into(),
                session_id: None,
                tool_name: None,
                tool_use_id: None,
                tool_input: None,
                cwd: None,
                model: None,
                agent_id: None,
                raw_json: v,
            });
        }

        if v.get("stage").is_some() || v.get("percent").is_some() {
            return Some(HookEvent {
                event_name: "DemoProgress".into(),
                session_id: None,
                tool_name: None,
                tool_use_id: None,
                tool_input: None,
                cwd: None,
                model: None,
                agent_id: None,
                raw_json: v,
            });
        }

        None
    }

    /// Maps OpenCode `run --format json` JSONL (`serde_json::Value`) to hook-shaped events for shared UI handling.
    pub fn from_opencode_stream_value(v: &Value) -> Option<Self> {
        let typ = v.get("type").and_then(|t| t.as_str())?;
        match typ {
            "tool_use" => {
                let tool = v
                    .pointer("/part/tool")
                    .and_then(|x| x.as_str())
                    .unwrap_or("tool")
                    .to_string();
                let title = v
                    .pointer("/part/state/title")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| tool.clone());
                let msg = format!("{} — {}", tool, title);
                Some(HookEvent {
                    event_name: "DemoProgress".into(),
                    session_id: None,
                    tool_name: Some(tool),
                    tool_use_id: v
                        .pointer("/part/callID")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string()),
                    tool_input: v.pointer("/part/state/input").cloned(),
                    cwd: None,
                    model: None,
                    agent_id: None,
                    raw_json: serde_json::json!({
                        "stage": "working",
                        "message": msg,
                        "percent": serde_json::Value::Null,
                    }),
                })
            }
            "step_start" => Some(HookEvent {
                event_name: "DemoProgress".into(),
                session_id: None,
                tool_name: None,
                tool_use_id: None,
                tool_input: None,
                cwd: None,
                model: None,
                agent_id: None,
                raw_json: serde_json::json!({
                    "stage": "thinking",
                    "message": "OpenCode step started…",
                    "percent": serde_json::Value::Null,
                }),
            }),
            "step_finish" => {
                let reason = v.pointer("/part/reason").and_then(|x| x.as_str());
                if reason == Some("stop") {
                    Some(HookEvent {
                        event_name: "DemoProgress".into(),
                        session_id: None,
                        tool_name: None,
                        tool_use_id: None,
                        tool_input: None,
                        cwd: None,
                        model: None,
                        agent_id: None,
                        raw_json: serde_json::json!({
                            "stage": "done",
                            "message": "OpenCode finished.",
                            "percent": 100.0,
                        }),
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
