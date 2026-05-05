// CLI 流事件发射 + JSON 提取工具 + 通用超时常量。
//
// 所有 backend (claude / opencode / codex) 的 stdout/stderr 流式输出
// 统一通过 emit_cli_stream_line 发到前端 `galcode://cli-output` 频道。
// 前端按 stream_id 路由到对应会话的日志面板 / 块渲染器。

use crate::agent::runtime::CliStreamEvent;
use serde_json::Value;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

// ---------------------------------------------------------------------------
// 超时常量
// ---------------------------------------------------------------------------

pub const CODEX_APP_SERVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub const CODEX_TURN_TIMEOUT: Duration = Duration::from_secs(1800);
pub const OPENCODE_READY_TIMEOUT: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// 事件名常量
// ---------------------------------------------------------------------------

pub const CLI_STREAM_EVENT: &str = "galcode://cli-output";

// ---------------------------------------------------------------------------
// 输出清洗
// ---------------------------------------------------------------------------

pub fn strip_cli_warning_lines(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("WARNING:")
                && trimmed != "Loaded cached credentials."
                && !trimmed.contains("[STARTUP] Phase 'cli_startup' was started but never ended.")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// CLI 流事件发射
// ---------------------------------------------------------------------------

pub fn emit_cli_stream_line(
    app: &AppHandle,
    backend: &str,
    run_id: &str,
    stream_id: &str,
    channel: &str,
    line: &str,
) {
    let _ = app.emit(
        CLI_STREAM_EVENT,
        CliStreamEvent {
            stream_id: stream_id.to_string(),
            backend: backend.to_string(),
            channel: channel.to_string(),
            line: line.to_string(),
            run_id: run_id.to_string(),
        },
    );
}

pub fn emit_cli_stream_json_event(
    app: &AppHandle,
    backend: &str,
    run_id: &str,
    stream_id: &str,
    value: &Value,
) {
    if let Ok(line) = serde_json::to_string(value) {
        emit_cli_stream_line(app, backend, run_id, stream_id, "stdout", &line);
    }
}

// ---------------------------------------------------------------------------
// JSON 辅助 (RPC id / 嵌套字段读取)
// ---------------------------------------------------------------------------

pub fn json_rpc_id_string(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    None
}

pub fn read_json_string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }

    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn read_nested_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(ToOwned::to_owned)
}
