use crate::session::state::AgentStatus;
use serde::Serialize;

/// 多 tab 路由：所有 agent://* 事件 payload 都带 run_id（None 表示不属于任何 tab，
/// 通常是兜底 / 老调用路径）。前端按 run_id 把事件分发到对应 tab slice。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusChangedPayload {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub status: AgentStatus,
    pub tool_name: Option<String>,
    pub tool_description: Option<String>,
    pub percent: Option<f64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCompletePayload {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub message: String,
    pub code: String,
}
