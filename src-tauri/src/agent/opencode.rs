// OpenCode 运行时子系统。
// 关键设计（踩坑后的成熟方案）：
//   - per-tab 子进程：每个 tab 独立 `opencode serve` + 独立端口（4096+）
//   - HTTP 通信：POST /session 创建会话 → POST /session/<sid>/message 提交 prompt
//   - 实时事件：SSE GET /session/<sid>/event 优先，失败 fallback 到 message 轮询
//   - Auto-approve：背景 poller 监 GET /permission，新审批一律自动 always 放行
//   - 端口冲突：启动前 lsof / netstat 探活，名字含 opencode 才 kill 释放
//   - Windows XDG：手动注入 XDG_*_HOME 到 LocalAppData，不然 opencode 写不进 cache
//   - 子进程清理：递归杀 node MCP 子进程，避免端口被孙子进程占着

use crate::agent::runtime::*;
use crate::agent::sysutils::*;
use reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::watch;
use tokio::time::sleep;

// ---------------------------------------------------------------------------
// API 响应类型
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpencodeStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub running: bool,
    pub managed: bool,
    pub binary: String,
    pub port: u16,
    pub project_dir: String,
    pub session_id: Option<String>,
}

// ---------------------------------------------------------------------------
// HTTP 通信
// ---------------------------------------------------------------------------

pub async fn opencode_health(port: u16) -> bool {
    reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/app"))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

pub async fn opencode_request_with_timeout(
    port: u16,
    method: Method,
    path: &str,
    body: Option<Value>,
    directory: Option<&str>,
    timeout: Duration,
) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{port}{path}");
    let mut request = client.request(method, url);

    if let Some(directory) = directory.filter(|value| !value.trim().is_empty()) {
        request = request.query(&[("directory", directory)]);
    }

    if let Some(payload) = body {
        request = request.json(&payload);
    }

    let response = request
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| format!("OpenCode request failed: {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("OpenCode returned {status}: {body}"));
    }

    response
        .json::<Value>()
        .await
        .map_err(|error| format!("Failed to decode OpenCode response: {error}"))
}

pub async fn opencode_request(
    port: u16,
    method: Method,
    path: &str,
    body: Option<Value>,
    directory: Option<&str>,
) -> Result<Value, String> {
    opencode_request_with_timeout(
        port,
        method,
        path,
        body,
        directory,
        Duration::from_secs(120),
    )
    .await
}

pub fn extract_opencode_session_id(value: &Value) -> Option<String> {
    value
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| value.get("sessionID").and_then(Value::as_str))
        .or_else(|| {
            value
                .get("session")
                .and_then(|session| session.get("id"))
                .and_then(Value::as_str)
        })
        .map(ToOwned::to_owned)
}

#[allow(dead_code)]
pub fn extract_opencode_error(value: &Value) -> Option<String> {
    read_nested_string(value, &["info", "error", "data", "message"])
        .or_else(|| read_nested_string(value, &["error", "data", "message"]))
        .or_else(|| read_nested_string(value, &["error", "message"]))
}

// ---------------------------------------------------------------------------
// 状态查询
// ---------------------------------------------------------------------------

pub fn refresh_child_state(state: &mut OpencodeState) {
    if let Some(child) = state.child.as_mut() {
        if child.try_wait().ok().flatten().is_some() {
            state.child = None;
            state.session_id = None;
            state.managed = false;
        }
    }
}

pub async fn snapshot_opencode(
    app: &AppHandle,
    state: &RuntimeState,
    run_id: &str,
) -> Result<OpencodeStatus, String> {
    let root = resolve_project_root(app)?;
    let (binary, port, managed, session_id) = with_opencode_state(state, run_id, |opencode| {
        refresh_child_state(opencode);
        (
            opencode.binary.clone(),
            opencode.port,
            opencode.managed,
            opencode.session_id.clone(),
        )
    })?;
    let resolved_binary = resolve_opencode_binary(app, Some(binary.as_str()));

    let version = opencode_command_version(&resolved_binary, &root).ok();
    let installed = version.is_some();
    let running = opencode_health(port).await;

    Ok(OpencodeStatus {
        installed,
        version,
        running,
        managed,
        binary: resolved_binary.display().to_string(),
        port,
        project_dir: root.display().to_string(),
        session_id,
    })
}

// ---------------------------------------------------------------------------
// 启动 / 停止 OpenCode 服务
// ---------------------------------------------------------------------------

pub async fn opencode_start(
    app: &AppHandle,
    state: &RuntimeState,
    run_id: &str,
    binary: Option<&str>,
    proxy: Option<&str>,
    port: Option<u16>,
    cwd_for_serve: Option<&str>,
) -> Result<OpencodeStatus, String> {
    let root_pathbuf = resolve_project_root(app)?;
    let serve_cwd: std::path::PathBuf = cwd_for_serve
        .map(std::path::PathBuf::from)
        .unwrap_or(root_pathbuf.clone());
    // 每个 tab 启动 OpenCode 时从端口池分配独立端口，避免多 tab 抢同一个 4096
    let desired_port = allocate_opencode_port(state, port);
    let desired_binary = resolve_opencode_binary(app, binary);
    let desired_proxy = proxy
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let version = opencode_command_version(&desired_binary, &root_pathbuf)?;

    if opencode_health(desired_port).await {
        with_opencode_state(state, run_id, |opencode| {
            opencode.binary = desired_binary.display().to_string();
            opencode.port = desired_port;
            opencode.managed = false;
        })?;
        let mut status = snapshot_opencode(app, state, run_id).await?;
        status.version = Some(version);
        return Ok(status);
    }

    let existing_listeners = listening_process_ids(desired_port).unwrap_or_default();
    if !existing_listeners.is_empty() {
        let _ = kill_opencode_listeners(&[desired_port, 1455]);

        for _ in 0..20 {
            let remaining = listening_process_ids(desired_port).unwrap_or_default();
            if remaining.is_empty() {
                break;
            }
            sleep(Duration::from_millis(250)).await;
        }

        let remaining = listening_process_ids(desired_port).unwrap_or_default();
        if !remaining.is_empty() {
            return Err(format!(
                "OpenCode port {desired_port} is already occupied by another process. Stop the existing listener and retry."
            ));
        }
    }

    let mut command = Command::new(&desired_binary);
    configure_background_command(&mut command);
    apply_opencode_runtime_env(&mut command)?;
    command
        .current_dir(&serve_cwd)
        .arg("serve")
        .arg("--hostname")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(desired_port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    apply_proxy_env(&mut command, desired_proxy.as_deref());

    let child = command
        .spawn()
        .map_err(|error| format!("Failed to start OpenCode: {error}"))?;

    with_opencode_state(state, run_id, |opencode| {
        opencode.child = Some(child);
        opencode.port = desired_port;
        opencode.binary = desired_binary.display().to_string();
        opencode.managed = true;
    })?;

    let started_at = Instant::now();
    while started_at.elapsed() < OPENCODE_READY_TIMEOUT {
        if opencode_health(desired_port).await {
            let mut status = snapshot_opencode(app, state, run_id).await?;
            status.version = Some(version);
            return Ok(status);
        }

        let child_alive = with_opencode_state(state, run_id, |opencode| {
            refresh_child_state(opencode);
            opencode.child.is_some()
        })?;
        if !child_alive {
            return Err("OpenCode exited before becoming ready.".to_string());
        }
        sleep(Duration::from_millis(500)).await;
    }

    Err("OpenCode did not become ready in time.".to_string())
}

pub async fn opencode_stop(
    state: &RuntimeState,
    run_id: &str,
) -> Result<OpencodeStatus, String> {
    let port_to_stop = with_opencode_state(state, run_id, |opencode| {
        let port = opencode.port;
        if let Some(child) = opencode.child.as_mut() {
            kill_child_descendants(child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
        opencode.child = None;
        opencode.session_id = None;
        opencode.managed = false;
        port
    })?;

    let _ = kill_opencode_listeners(&[port_to_stop, 1455]);
    release_opencode_port(state, port_to_stop);

    for _ in 0..20 {
        if !opencode_health(port_to_stop).await {
            break;
        }
        sleep(Duration::from_millis(250)).await;
    }

    Ok(OpencodeStatus {
        installed: true,
        version: None,
        running: false,
        managed: false,
        binary: String::new(),
        port: port_to_stop,
        project_dir: String::new(),
        session_id: None,
    })
}

// ---------------------------------------------------------------------------
// Session 管理
// ---------------------------------------------------------------------------

pub async fn opencode_create_session(
    app: &AppHandle,
    state: &RuntimeState,
    run_id: &str,
    title: Option<&str>,
    directory: Option<&str>,
) -> Result<String, String> {
    let status = snapshot_opencode(app, state, run_id).await?;
    if !status.running {
        return Err("OpenCode server is not running.".to_string());
    }

    let payload = json!({
        "title": title.unwrap_or("Galcode Local Agent")
    });

    let response = opencode_request(
        status.port,
        Method::POST,
        "/session",
        Some(payload),
        directory,
    )
    .await?;

    let session_id = extract_opencode_session_id(&response)
        .ok_or_else(|| "OpenCode session creation did not return a session id.".to_string())?;

    with_opencode_state(state, run_id, |opencode| {
        opencode.session_id = Some(session_id.clone());
    })?;

    Ok(session_id)
}

// ---------------------------------------------------------------------------
// 权限自动批准（auto-approve poller）
// ---------------------------------------------------------------------------

pub fn extract_opencode_permission_entries(value: &Value) -> Vec<Value> {
    if let Some(array) = value.as_array() {
        return array.clone();
    }

    for key in ["items", "permissions", "requests", "data"] {
        if let Some(array) = value.get(key).and_then(Value::as_array) {
            return array.clone();
        }
    }

    Vec::new()
}

pub fn opencode_permission_id(value: &Value) -> Option<String> {
    read_nested_string(value, &["id"])
        .or_else(|| read_nested_string(value, &["permissionID"]))
        .or_else(|| read_nested_string(value, &["permissionId"]))
        .or_else(|| read_nested_string(value, &["requestID"]))
        .or_else(|| read_nested_string(value, &["requestId"]))
}

pub fn opencode_permission_session_id(value: &Value) -> Option<String> {
    read_nested_string(value, &["sessionID"])
        .or_else(|| read_nested_string(value, &["sessionId"]))
        .or_else(|| read_nested_string(value, &["session", "id"]))
}

pub fn opencode_permission_outcome(value: &Value) -> Option<String> {
    if let Some(reply) = read_nested_string(value, &["reply"]) {
        return Some(reply);
    }

    if let Some(response) = value.get("response") {
        if let Some(text) = response.as_str() {
            return Some(text.to_string());
        }
        if let Some(boolean) = response.as_bool() {
            return Some(if boolean { "once" } else { "reject" }.to_string());
        }
    }

    read_nested_string(value, &["status"])
}

pub fn filter_opencode_permissions(entries: Vec<Value>, session_id: Option<&str>) -> Vec<Value> {
    let Some(session_id) = session_id.filter(|value| !value.trim().is_empty()) else {
        return entries;
    };

    entries
        .into_iter()
        .filter(|entry| {
            opencode_permission_session_id(entry)
                .as_deref()
                .map(|value| value == session_id)
                .unwrap_or(true)
        })
        .collect()
}

pub async fn opencode_permissions_request(
    port: u16,
    session_id: Option<&str>,
    directory: Option<&str>,
) -> Result<Vec<Value>, String> {
    let mut last_error = None;

    match opencode_request(port, Method::GET, "/permission", None, directory).await {
        Ok(response) => {
            return Ok(filter_opencode_permissions(
                extract_opencode_permission_entries(&response),
                session_id,
            ))
        }
        Err(error) => {
            if !error.contains("404") {
                last_error = Some(error);
            }
        }
    }

    if let Some(session_id) = session_id {
        match opencode_request(
            port,
            Method::GET,
            &format!("/session/{session_id}"),
            None,
            directory,
        )
        .await
        {
            Ok(response) => {
                return Ok(filter_opencode_permissions(
                    extract_opencode_permission_entries(&response),
                    Some(session_id),
                ))
            }
            Err(error) => {
                if !error.contains("404") {
                    last_error = Some(error);
                }
            }
        }
    }

    if let Some(error) = last_error {
        return Err(error);
    }

    Ok(Vec::new())
}

pub fn opencode_permission_signature(permission: &Value) -> Option<String> {
    let approval_id = opencode_permission_id(permission)?;
    let status = opencode_permission_outcome(permission).unwrap_or_else(|| "waiting".to_string());
    Some(format!("{approval_id}:{status}"))
}

pub async fn opencode_reply_permission(
    port: u16,
    session_id: Option<&str>,
    approval_id: &str,
    decision: &str,
    directory: Option<&str>,
) -> Result<Value, String> {
    let normalized = match decision.trim() {
        "allow-session" | "always" | "approved_for_session" => "always",
        "confirm" | "approve" | "approved" | "once" => "once",
        "cancel" | "deny" | "denied" | "reject" | "abort" => "reject",
        other => other,
    };

    let attempts = [
        json!({ "reply": normalized }),
        json!({ "response": normalized, "remember": normalized == "always" }),
        json!({ "response": normalized != "reject", "remember": normalized == "always" }),
    ];

    let mut last_error = None;
    for body in attempts {
        match opencode_request(
            port,
            Method::POST,
            &format!("/permission/{approval_id}/reply"),
            Some(body.clone()),
            directory,
        )
        .await
        {
            Ok(value) => return Ok(value),
            Err(error) => {
                if !error.contains("404") {
                    last_error = Some(error);
                    break;
                }
            }
        }

        if let Some(session_id) = session_id.filter(|value| !value.trim().is_empty()) {
            match opencode_request(
                port,
                Method::POST,
                &format!("/session/{session_id}/permissions/{approval_id}"),
                Some(body),
                directory,
            )
            .await
            {
                Ok(value) => return Ok(value),
                Err(error) => {
                    if !error.contains("404") {
                        last_error = Some(error);
                        break;
                    }
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "当前 OpenCode 版本未暴露可用的审批回复接口。".to_string()))
}

/// Auto-approve poller：每 900ms 拉一次待审批列表，看到新审批就直接 always 回复。
/// 第一版策略全自动放行，未来需要审批 UI 时可以切换成 emit 让前端处理。
pub fn spawn_opencode_auto_approve_poller(
    app: &AppHandle,
    port: u16,
    session_id: &str,
    directory: Option<&str>,
) -> (watch::Sender<bool>, tokio::task::JoinHandle<()>) {
    let _ = app;
    let session_id = session_id.to_string();
    let directory = directory.map(ToOwned::to_owned);
    let (stop_tx, stop_rx) = watch::channel(false);

    let handle = tokio::spawn(async move {
        let mut seen = BTreeSet::new();

        loop {
            if let Ok(entries) =
                opencode_permissions_request(port, Some(session_id.as_str()), directory.as_deref())
                    .await
            {
                for permission in entries {
                    let Some(signature) = opencode_permission_signature(&permission) else {
                        continue;
                    };
                    if !seen.insert(signature) {
                        continue;
                    }
                    let outcome = opencode_permission_outcome(&permission);
                    // 已经处理过的（resolved/approved/rejected）不再回复
                    if outcome
                        .as_deref()
                        .map(|value| !value.eq_ignore_ascii_case("waiting"))
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    let Some(approval_id) = opencode_permission_id(&permission) else {
                        continue;
                    };
                    let _ = opencode_reply_permission(
                        port,
                        Some(session_id.as_str()),
                        &approval_id,
                        "always",
                        directory.as_deref(),
                    )
                    .await;
                }
            }

            if *stop_rx.borrow() {
                break;
            }

            sleep(Duration::from_millis(900)).await;
        }
    });

    (stop_tx, handle)
}

// ---------------------------------------------------------------------------
// 消息部分解析（snapshot / SSE 复用）
// ---------------------------------------------------------------------------

pub fn extract_opencode_messages(response: &Value) -> Vec<Value> {
    if let Some(array) = response.as_array() {
        return array.clone();
    }

    if response.get("parts").and_then(Value::as_array).is_some() {
        return vec![response.clone()];
    }

    if let Some(message) = response.get("message") {
        if message.is_object() {
            return vec![message.clone()];
        }
    }

    for key in ["messages", "items", "data"] {
        if let Some(value) = response.get(key) {
            if let Some(array) = value.as_array() {
                return array.clone();
            }
            if let Some(array) = value.get("messages").and_then(Value::as_array) {
                return array.clone();
            }
            if let Some(message) = value.get("message") {
                if message.is_object() {
                    return vec![message.clone()];
                }
            }
            if value.get("parts").and_then(Value::as_array).is_some() {
                return vec![value.clone()];
            }
        }
    }
    Vec::new()
}

pub fn opencode_message_parts(message: &Value) -> Vec<Value> {
    if let Some(array) = message.get("parts").and_then(Value::as_array) {
        return array.clone();
    }
    if let Some(array) = message.get("content").and_then(Value::as_array) {
        return array.clone();
    }
    Vec::new()
}

pub fn opencode_part_id(part: &Value) -> String {
    read_nested_string(part, &["id"])
        .or_else(|| read_nested_string(part, &["partID"]))
        .or_else(|| read_nested_string(part, &["partId"]))
        .or_else(|| read_nested_string(part, &["toolCallId"]))
        .or_else(|| read_nested_string(part, &["tool_call_id"]))
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn extract_opencode_text_from_messages(response: &Value) -> String {
    let mut chunks: Vec<String> = Vec::new();
    for message in extract_opencode_messages(response) {
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        if !role.is_empty() && role != "assistant" {
            continue;
        }
        for part in opencode_message_parts(&message) {
            let part_type = part.get("type").and_then(Value::as_str).unwrap_or("");
            if part_type == "text" {
                if let Some(text) = part
                    .get("text")
                    .or_else(|| part.get("content"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    chunks.push(text.to_string());
                }
            }
        }
    }
    chunks.join("\n\n")
}

fn emit_opencode_message_part_internal(
    app: &AppHandle,
    stream_id: &str,
    part: &Value,
    suppress_log_line: bool,
    visible_log_counter: Option<&Arc<AtomicUsize>>,
) {
    let part_type = part.get("type").and_then(Value::as_str).unwrap_or("");
    let part_id = opencode_part_id(part);
    let emit = |value: Value| {
        if !suppress_log_line {
            if let Some(counter) = visible_log_counter {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }
        emit_cli_stream_json_event(app, "opencode", stream_id, &value);
    };

    match part_type {
        "text" => {
            let text = part
                .get("text")
                .or_else(|| part.get("content"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if text.is_empty() {
                return;
            }
            emit(json!({
                "type": "galcode.block",
                "block": {
                    "id": format!("opencode-text-{part_id}"),
                    "type": "text",
                    "content": text,
                    "backend": "opencode",
                    "suppressLogLine": suppress_log_line
                }
            }));
        }
        "tool" | "tool-invocation" | "tool-call" | "tool_call" => {
            let tool_name = part
                .get("toolName")
                .or_else(|| part.get("name"))
                .or_else(|| part.get("tool"))
                .and_then(Value::as_str)
                .unwrap_or("tool");
            let tool_name_normalized = tool_name.to_ascii_lowercase();
            let state = part.get("state");
            let args = state
                .and_then(|value| value.get("input"))
                .or_else(|| part.get("args"))
                .or_else(|| part.get("input"));
            let command = args
                .and_then(|a| {
                    a.get("command")
                        .or_else(|| a.get("cmd"))
                        .or_else(|| a.get("commandLine"))
                })
                .and_then(Value::as_str);
            let file_path = args.and_then(|a| {
                a.get("filePath")
                    .or_else(|| a.get("file_path"))
                    .or_else(|| a.get("path"))
                    .and_then(Value::as_str)
            });
            let detail = args
                .and_then(|a| {
                    a.get("pattern")
                        .or_else(|| a.get("query"))
                        .or_else(|| a.get("url"))
                        .or_else(|| a.get("task"))
                })
                .and_then(Value::as_str);
            let result = state
                .and_then(|value| value.get("output"))
                .or_else(|| part.get("result"))
                .and_then(Value::as_str);
            let error = state
                .and_then(|value| value.get("error"))
                .and_then(Value::as_str);
            let status = state
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
                .unwrap_or_else(|| {
                    if error.is_some() {
                        "error"
                    } else if result.is_some() {
                        "completed"
                    } else {
                        "running"
                    }
                });
            let tool_id = part_id.as_str();

            if command.is_some()
                || tool_name.eq_ignore_ascii_case("bash")
                || tool_name.eq_ignore_ascii_case("shell")
            {
                emit(json!({
                    "type": "galcode.block",
                    "block": {
                        "id": format!("opencode-cmd-{tool_id}"),
                        "type": "command",
                        "command": command.unwrap_or(tool_name),
                        "output": error.unwrap_or(result.unwrap_or("")),
                        "status": if status == "completed" { "success" } else { status },
                        "backend": "opencode",
                        "suppressLogLine": suppress_log_line
                    }
                }));
                return;
            }

            if file_path.is_some()
                || tool_name_normalized.contains("file")
                || tool_name_normalized.contains("write")
                || tool_name_normalized.contains("read")
                || tool_name_normalized.contains("edit")
            {
                emit(json!({
                    "type": "opencode.file",
                    "id": format!("opencode-file-{tool_id}"),
                    "tool": tool_name,
                    "path": file_path.unwrap_or(""),
                    "detail": detail,
                    "message": error,
                    "status": status,
                    "suppressLogLine": suppress_log_line
                }));
                return;
            }

            emit(json!({
                "type": "opencode.tool",
                "id": format!("opencode-tool-{tool_id}"),
                "tool": tool_name,
                "detail": detail,
                "message": error,
                "status": status,
                "suppressLogLine": suppress_log_line
            }));
        }
        "patch" => {
            let files = part
                .get("files")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();

            if files.is_empty() {
                emit(json!({
                    "type": "opencode.tool",
                    "id": format!("opencode-patch-{part_id}"),
                    "tool": "patch",
                    "status": "completed",
                    "suppressLogLine": suppress_log_line
                }));
                return;
            }

            for (index, file) in files.iter().enumerate() {
                let Some(path) = file.as_str() else {
                    continue;
                };
                emit(json!({
                    "type": "opencode.file",
                    "id": format!("opencode-patch-{part_id}-{index}"),
                    "tool": "patch",
                    "path": path,
                    "status": "completed",
                    "suppressLogLine": suppress_log_line
                }));
            }
        }
        "tool-result" | "tool_result" => {
            // 工具结果通常已合并到 tool-invocation 中
        }
        "reasoning" | "thinking" => {
            let content = part
                .get("text")
                .or_else(|| part.get("content"))
                .or_else(|| part.get("thinking"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if !content.is_empty() {
                emit(json!({
                    "type": "galcode.block",
                    "block": {
                        "id": format!("opencode-thought-{part_id}"),
                        "type": "thought",
                        "content": content,
                        "backend": "opencode",
                        "suppressLogLine": suppress_log_line
                    }
                }));
            }
        }
        "step-start" | "step_start" => {
            emit(json!({
                "type": "opencode.status",
                "id": format!("opencode-status-{part_id}"),
                "message": "Thinking...",
                "suppressLogLine": suppress_log_line
            }));
        }
        _ => {}
    }
}

pub fn emit_opencode_message_snapshot_internal(
    app: &AppHandle,
    stream_id: &str,
    response: &Value,
    suppress_log_line: bool,
) -> usize {
    let mut emitted = 0usize;

    for message in extract_opencode_messages(response) {
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        if !role.is_empty() && role != "assistant" {
            continue;
        }

        for part in opencode_message_parts(&message) {
            emit_opencode_message_part_internal(app, stream_id, &part, suppress_log_line, None);
            emitted += 1;
        }
    }

    emitted
}

pub async fn emit_opencode_session_snapshot_internal(
    app: &AppHandle,
    port: u16,
    session_id: &str,
    directory: Option<&str>,
    stream_id: &str,
    suppress_log_line: bool,
) -> usize {
    match opencode_request(
        port,
        Method::GET,
        &format!("/session/{session_id}/message"),
        None,
        directory,
    )
    .await
    {
        Ok(response) => {
            emit_opencode_message_snapshot_internal(app, stream_id, &response, suppress_log_line)
        }
        Err(_) => 0,
    }
}

// ---------------------------------------------------------------------------
// SSE 事件流
// ---------------------------------------------------------------------------

/// 解析单个 SSE 事件块并发射对应的 CLI 流事件。
fn process_sse_block(
    app: &AppHandle,
    stream_id: &str,
    block: &str,
    visible_log_counter: &Arc<AtomicUsize>,
) {
    let mut data_lines: Vec<&str> = Vec::new();
    let mut event_name = String::new();

    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(':') || trimmed.is_empty() {
            continue;
        }
        if let Some(event) = trimmed.strip_prefix("event:") {
            event_name = event.trim().to_string();
            continue;
        }
        if let Some(data) = trimmed.strip_prefix("data:") {
            data_lines.push(data.trim());
        }
    }

    if data_lines.is_empty() {
        return;
    }

    let data_str = data_lines.join("\n");
    let Ok(mut json) = serde_json::from_str::<Value>(&data_str) else {
        return;
    };

    if !event_name.is_empty() && json.get("type").is_none() {
        if let Some(object) = json.as_object_mut() {
            object.insert("type".to_string(), Value::String(event_name));
        }
    }

    emit_opencode_sse_json(app, stream_id, &json, visible_log_counter);
}

/// 处理解析后的 SSE JSON 事件。
fn emit_opencode_sse_json(
    app: &AppHandle,
    stream_id: &str,
    json: &Value,
    visible_log_counter: &Arc<AtomicUsize>,
) {
    let event_type = json.get("type").and_then(Value::as_str).unwrap_or("");
    let properties = json
        .get("properties")
        .cloned()
        .unwrap_or_else(|| json.clone());
    let emit = |value: Value| {
        visible_log_counter.fetch_add(1, Ordering::Relaxed);
        emit_cli_stream_json_event(app, "opencode", stream_id, &value);
    };

    match event_type {
        "part.updated" | "message.part.delta" | "message.part.start" | "message.part.stop" => {
            if let Some(part) = properties.get("part").or(Some(&properties)) {
                emit_opencode_message_part_internal(
                    app,
                    stream_id,
                    part,
                    false,
                    Some(visible_log_counter),
                );
            }
        }
        "message.updated" | "message.created" => {
            if let Some(message) = properties.get("message").or(Some(&properties)) {
                let role = message.get("role").and_then(Value::as_str).unwrap_or("");
                if role.is_empty() || role == "assistant" {
                    for part in opencode_message_parts(message) {
                        emit_opencode_message_part_internal(
                            app,
                            stream_id,
                            &part,
                            false,
                            Some(visible_log_counter),
                        );
                    }
                }
            }
        }
        "message.start" => {
            emit(json!({ "type": "opencode.status", "message": "正在处理..." }));
        }
        "error" => {
            let message = read_nested_string(&properties, &["message"])
                .or_else(|| read_nested_string(&properties, &["error"]))
                .unwrap_or_else(|| "Unknown error".to_string());
            emit(json!({ "type": "opencode.error", "message": message }));
        }
        "tool.start" | "tool.end" | "step.start" | "step.end" => {
            let tool = read_nested_string(&properties, &["name"])
                .or_else(|| read_nested_string(&properties, &["tool"]))
                .unwrap_or_default();
            if !tool.is_empty() {
                let event_id = read_nested_string(&properties, &["id"])
                    .or_else(|| read_nested_string(&properties, &["toolCallId"]))
                    .unwrap_or_else(|| format!("{}-{}", event_type.replace('.', "-"), tool));
                let status = if event_type.ends_with(".start") {
                    "running"
                } else {
                    "completed"
                };
                emit(json!({
                    "type": "opencode.tool",
                    "id": event_id,
                    "tool": tool,
                    "status": status
                }));
            }
        }
        "event" => {
            if let Some(nested_type) = read_nested_string(&properties, &["type"]) {
                let mut nested = properties.clone();
                if let Some(obj) = nested.as_object_mut() {
                    obj.insert("type".to_string(), Value::String(nested_type));
                }
                emit_opencode_sse_json(app, stream_id, &nested, visible_log_counter);
            }
        }
        _ => {
            if let Some(part) = properties.get("part") {
                emit_opencode_message_part_internal(
                    app,
                    stream_id,
                    part,
                    false,
                    Some(visible_log_counter),
                );
            }
        }
    }
}

/// 尝试通过 SSE 端点接收事件流。成功连接返回 `true`。
async fn try_opencode_sse_stream(
    app: &AppHandle,
    port: u16,
    session_id: &str,
    directory: Option<&str>,
    stream_id: &str,
    stop_rx: &watch::Receiver<bool>,
    visible_log_counter: &Arc<AtomicUsize>,
) -> bool {
    let url = format!("http://127.0.0.1:{port}/session/{session_id}/event");
    let client = reqwest::Client::new();
    let mut request = client
        .get(&url)
        .header("Accept", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .timeout(Duration::from_secs(1800));

    if let Some(dir) = directory.filter(|v| !v.trim().is_empty()) {
        request = request.query(&[("directory", dir)]);
    }

    let response = match request.send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return false,
    };

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.contains("text/event-stream") && !content_type.contains("text/plain") {
        return false;
    }

    let mut response = response;
    let mut buffer = String::new();
    loop {
        if *stop_rx.borrow() {
            break;
        }

        match tokio::time::timeout(Duration::from_secs(2), response.chunk()).await {
            Ok(Ok(Some(bytes))) => {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    buffer.push_str(text);
                    while let Some(pos) = buffer.find("\n\n") {
                        let event_block = buffer[..pos].to_string();
                        buffer = buffer[pos + 2..].to_string();
                        process_sse_block(app, stream_id, &event_block, visible_log_counter);
                    }
                }
            }
            Ok(Ok(None)) => break,
            Ok(Err(_)) => break,
            Err(_) => continue,
        }
    }

    true
}

/// 轮询消息端点获取增量更新（SSE 不可用时的回退方案）。
async fn poll_opencode_messages_stream(
    app: &AppHandle,
    port: u16,
    session_id: &str,
    directory: Option<&str>,
    stream_id: &str,
    stop_rx: &watch::Receiver<bool>,
    visible_log_counter: &Arc<AtomicUsize>,
) {
    let mut seen_parts: HashMap<String, String> = HashMap::new();
    let sync_messages = |response: &Value, seen_parts: &mut HashMap<String, String>| {
        let messages = extract_opencode_messages(response);
        for (message_index, message) in messages.iter().enumerate() {
            let msg_id = message
                .get("id")
                .or_else(|| message.get("messageID"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let role = message.get("role").and_then(Value::as_str).unwrap_or("");
            if !role.is_empty() && role != "assistant" {
                continue;
            }
            let message_key = if msg_id.is_empty() {
                format!("message-{message_index}")
            } else {
                msg_id.to_string()
            };

            let parts = opencode_message_parts(message);
            for (index, part) in parts.iter().enumerate() {
                let part_id = opencode_part_id(part);
                let signature_id = if part_id == "unknown" {
                    format!("{message_key}:{index}")
                } else {
                    format!("{message_key}:{part_id}")
                };
                let Ok(signature) = serde_json::to_string(part) else {
                    continue;
                };

                match seen_parts.get(&signature_id) {
                    Some(previous) if previous == &signature => {}
                    Some(_) => {
                        emit_opencode_message_part_internal(
                            app,
                            stream_id,
                            part,
                            true,
                            Some(visible_log_counter),
                        );
                        seen_parts.insert(signature_id, signature);
                    }
                    None => {
                        emit_opencode_message_part_internal(
                            app,
                            stream_id,
                            part,
                            false,
                            Some(visible_log_counter),
                        );
                        seen_parts.insert(signature_id, signature);
                    }
                }
            }
        }
    };

    // 初始化已有消息的部件计数，仅追踪后续新增内容
    if let Ok(response) = opencode_request(
        port,
        Method::GET,
        &format!("/session/{session_id}/message"),
        None,
        directory,
    )
    .await
    {
        for (message_index, message) in extract_opencode_messages(&response).iter().enumerate() {
            let msg_id = message
                .get("id")
                .or_else(|| message.get("messageID"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let role = message.get("role").and_then(Value::as_str).unwrap_or("");
            if !role.is_empty() && role != "assistant" {
                continue;
            }
            let message_key = if msg_id.is_empty() {
                format!("message-{message_index}")
            } else {
                msg_id.to_string()
            };

            for (index, part) in opencode_message_parts(message).iter().enumerate() {
                let part_id = opencode_part_id(part);
                let signature_id = if part_id == "unknown" {
                    format!("{message_key}:{index}")
                } else {
                    format!("{message_key}:{part_id}")
                };
                if let Ok(signature) = serde_json::to_string(part) {
                    seen_parts.insert(signature_id, signature);
                }
            }
        }
    }

    loop {
        if *stop_rx.borrow() {
            if let Ok(response) = opencode_request(
                port,
                Method::GET,
                &format!("/session/{session_id}/message"),
                None,
                directory,
            )
            .await
            {
                sync_messages(&response, &mut seen_parts);
            }
            break;
        }

        if let Ok(response) = opencode_request(
            port,
            Method::GET,
            &format!("/session/{session_id}/message"),
            None,
            directory,
        )
        .await
        {
            sync_messages(&response, &mut seen_parts);
        }

        // 分段休眠以便及时响应停止信号
        let mut stop_requested = false;
        for _ in 0..5 {
            if *stop_rx.borrow() {
                stop_requested = true;
                break;
            }
            sleep(Duration::from_millis(300)).await;
        }
        if stop_requested {
            continue;
        }
    }
}

/// 启动 OpenCode 会话事件流监听任务。
/// 优先使用 SSE 端点，不可用时回退为消息轮询。
pub fn spawn_opencode_event_stream(
    app: &AppHandle,
    port: u16,
    session_id: &str,
    directory: Option<&str>,
    stream_id: &str,
) -> (
    watch::Sender<bool>,
    tokio::task::JoinHandle<()>,
    Arc<AtomicUsize>,
) {
    let app = app.clone();
    let session_id = session_id.to_string();
    let directory = directory.map(ToOwned::to_owned);
    let stream_id = stream_id.to_string();
    let visible_log_counter = Arc::new(AtomicUsize::new(0));
    let (stop_tx, stop_rx) = watch::channel(false);
    let visible_log_counter_for_task = visible_log_counter.clone();

    let handle = tokio::spawn(async move {
        let sse_ok = try_opencode_sse_stream(
            &app,
            port,
            &session_id,
            directory.as_deref(),
            &stream_id,
            &stop_rx,
            &visible_log_counter_for_task,
        )
        .await;

        if !sse_ok && !*stop_rx.borrow() {
            poll_opencode_messages_stream(
                &app,
                port,
                &session_id,
                directory.as_deref(),
                &stream_id,
                &stop_rx,
                &visible_log_counter_for_task,
            )
            .await;
        }
    });

    (stop_tx, handle, visible_log_counter)
}

// ---------------------------------------------------------------------------
// 高层组合：发 prompt + 等响应（含 SSE/poller 自动管理）
// ---------------------------------------------------------------------------

/// 阻塞式 turn：在 OpenCode session 上发一个 prompt，并等到 HTTP 响应回来。
/// 期间自动启动 SSE event stream 和 auto-approve permission poller，结束时清理。
pub async fn run_opencode_turn(
    app: &AppHandle,
    state: &RuntimeState,
    run_id: &str,
    session_id: &str,
    text: &str,
    system: Option<&str>,
    directory: Option<&str>,
    stream_id: Option<&str>,
) -> Result<(String, Value), String> {
    let status = snapshot_opencode(app, state, run_id).await?;
    if !status.running {
        return Err("OpenCode server is not running.".to_string());
    }

    let mut payload = json!({
        "parts": [
            {
                "type": "text",
                "text": text
            }
        ],
        "agent": "build"
    });

    if let Some(system) = system.filter(|value| !value.trim().is_empty()) {
        payload["system"] = Value::String(system.to_string());
    }

    let poller =
        spawn_opencode_auto_approve_poller(app, status.port, session_id, directory);

    let event_stream = stream_id.map(|stream_id| {
        spawn_opencode_event_stream(app, status.port, session_id, directory, stream_id)
    });

    let result = opencode_request_with_timeout(
        status.port,
        Method::POST,
        &format!("/session/{session_id}/message"),
        Some(payload),
        directory,
        Duration::from_secs(1800),
    )
    .await;

    let mut visible_opencode_log_events = 0usize;
    if let Some((stop_tx, handle, counter)) = event_stream {
        let _ = stop_tx.send(true);
        let _ = handle.await;
        visible_opencode_log_events = counter.load(Ordering::Relaxed);
    }
    {
        let (stop_tx, handle) = poller;
        let _ = stop_tx.send(true);
        let _ = handle.await;
    }

    let result = result?;

    if let Some(stream_id) = stream_id {
        if visible_opencode_log_events == 0 {
            let _ = emit_opencode_session_snapshot_internal(
                app,
                status.port,
                session_id,
                directory,
                stream_id,
                false,
            )
            .await;
        } else {
            let emitted = emit_opencode_message_snapshot_internal(app, stream_id, &result, true);
            if emitted == 0 {
                let _ = emit_opencode_session_snapshot_internal(
                    app,
                    status.port,
                    session_id,
                    directory,
                    stream_id,
                    true,
                )
                .await;
            }
        }
    }

    let final_text = extract_opencode_text_from_messages(&result);
    Ok((final_text, result))
}
