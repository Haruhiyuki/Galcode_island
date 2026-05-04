use crate::session::state::AgentStatus;
use serde::Serialize;

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
