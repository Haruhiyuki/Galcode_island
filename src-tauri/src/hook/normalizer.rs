pub fn normalize_event_name(raw: &str) -> String {
    match raw.to_lowercase().as_str() {
        "pre_tool_use" | "pretooluse" => "PreToolUse".into(),
        "post_tool_use" | "posttooluse" => "PostToolUse".into(),
        "post_tool_use_failure" => "PostToolUseFailure".into(),
        "user_prompt_submit" => "UserPromptSubmit".into(),
        "stop" | "session_end" => "Stop".into(),
        "session_start" => "SessionStart".into(),
        "subagent_start" => "SubagentStart".into(),
        "subagent_stop" => "SubagentStop".into(),
        "permission_request" => "PermissionRequest".into(),
        "notification" => "Notification".into(),
        _ => raw.to_string(),
    }
}
