use super::normalizer::normalize_event_name;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Assistant-visible text from a `Stop` line or `{ "type": "result", ... }` payload.
pub fn stop_output_from_raw(raw: &Value) -> Option<String> {
    // OpenCode `--format json` stream: assistant turn is often only `{ "type":"text", "part": { "text": "..." } }`
    if let Some(part) = raw.get("part") {
        if let Some(s) = part.get("text").and_then(|x| x.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    const KEYS: &[&str] = &[
        "output_en",
        "output",
        "last_assistant_message",
        "result",
        "message",
        "text",
        "content",
    ];
    for k in KEYS {
        if let Some(s) = raw.get(*k).and_then(|x| x.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

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
        let t = line.trim().trim_start_matches('\u{feff}').trim();
        if t.is_empty() {
            return None;
        }
        let v: Value = serde_json::from_str(t).ok()?;

        if let Some(raw_name) = v.get("hook_event_name").and_then(|x| x.as_str()) {
            let event_name = normalize_event_name(raw_name);
            let tool_use_id = v
                .get("tool_use_id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    v.get("_opencode_request_id")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                });
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
                tool_use_id,
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

        // OpenCode CLI JSONL (`run --format json`): step deltas, assistant text, etc.
        if v.get("type").and_then(|ty| ty.as_str()) == Some("text") {
            let piece = v
                .get("part")
                .and_then(|p| p.get("text"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if !piece.is_empty() {
                return Some(HookEvent {
                    event_name: "OpenCodeStreamText".into(),
                    session_id: v
                        .get("sessionID")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string()),
                    tool_name: None,
                    tool_use_id: None,
                    tool_input: None,
                    cwd: None,
                    model: None,
                    agent_id: None,
                    raw_json: v,
                });
            }
        }

        if v.get("type").and_then(|ty| ty.as_str()) == Some("step_start") {
            return Some(HookEvent {
                event_name: "OpenCodeStepStart".into(),
                session_id: v
                    .get("sessionID")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                tool_name: None,
                tool_use_id: None,
                tool_input: None,
                cwd: None,
                model: None,
                agent_id: None,
                raw_json: v,
            });
        }

        if v.get("type").and_then(|ty| ty.as_str()) == Some("step_finish") {
            let reason_stop = v
                .get("part")
                .and_then(|p| p.get("reason"))
                .and_then(|x| x.as_str())
                == Some("stop");
            return Some(HookEvent {
                event_name: if reason_stop {
                    "OpenCodeStepFinishStop".into()
                } else {
                    "OpenCodeStepFinish".into()
                },
                session_id: v
                    .get("sessionID")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
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
