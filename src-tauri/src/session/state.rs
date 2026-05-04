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
