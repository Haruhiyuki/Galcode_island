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
        let mut v: Value = serde_json::from_str(line.trim()).ok()?;

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

        // OpenCode / agent JSONL format
        if let Some(typ) = v
            .get("type")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
        {
            match typ.as_str() {
                "tool_use" => {
                    return Some(HookEvent {
                        event_name: "PreToolUse".into(),
                        session_id: None,
                        tool_name: v
                            .get("tool")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string()),
                        tool_use_id: v
                            .get("tool_use_id")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string()),
                        tool_input: v.get("input").cloned(),
                        cwd: None,
                        model: None,
                        agent_id: None,
                        raw_json: v,
                    });
                }
                "text" => {
                    return Some(HookEvent {
                        event_name: "Notification".into(),
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
                "step_start" => {
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("stage".into(), "thinking".into());
                    }
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
                "step_finish" => {
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("stage".into(), "working".into());
                    }
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
                "result" => {
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
                "error" => {
                    if let Some(obj) = v.as_object_mut() {
                        let msg = obj
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown error")
                            .to_string();
                        obj.insert("output_en".into(), msg.into());
                    }
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
                _ => {}
            }
        }

        // Legacy fallback: explicit stage/percent fields
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
}
