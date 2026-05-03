//! Line-oriented stdout parsing; converts JSON lines into [`crate::hook::event::HookEvent`].
//!
//! Supported shapes:
//! - **Demo agent** — progress `{ "stage", "message", "percent" }`; completion `{ "type": "result", "output_en" }` (or `result` / `message` / … — see `stop_output_from_raw`).
//! - **OpenCode plugin (`galcode-opencode.js`)** — `{ "hook_event_name", "session_id", ... }` as forwarded to HTTP `/hook`
//!   (e.g. `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Stop`).
