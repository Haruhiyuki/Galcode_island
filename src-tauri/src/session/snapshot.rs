use super::state::AgentStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHistoryEntry {
    pub tool: String,
    pub description: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub agent_type: String,
    pub status: AgentStatus,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub current_tool: Option<String>,
    pub tool_description: Option<String>,
    pub tool_history: Vec<ToolHistoryEntry>,
    pub last_user_prompt: Option<String>,
    pub last_assistant_message: Option<String>,
    pub start_time: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub pid: Option<u32>,
    pub interrupted: bool,
}

impl SessionSnapshot {
    pub fn new(session_id: String, agent_type: String, cwd: Option<String>, pid: Option<u32>) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            agent_type,
            status: AgentStatus::Starting,
            cwd,
            model: None,
            current_tool: None,
            tool_description: None,
            tool_history: Vec::new(),
            last_user_prompt: None,
            last_assistant_message: None,
            start_time: now,
            last_activity: now,
            pid,
            interrupted: false,
        }
    }
}
