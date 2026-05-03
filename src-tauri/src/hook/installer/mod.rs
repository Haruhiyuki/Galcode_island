//! Detect installed AI tools and merge hook configs (OpenCode first).

mod opencode;

use crate::hook::server::hook_post_url;
use tauri::{AppHandle, Emitter};

pub fn install_detected_hooks(app: &AppHandle) {
    let Some(url) = hook_post_url() else {
        log::warn!("hook 安装跳过：本地 HTTP 服务未启动");
        let _ = app.emit(
            "galcode://hook-install",
            serde_json::json!({
                "tool": "galcode-hook-server",
                "ok": false,
                "message": "无法在 17888–17929 端口绑定 Hook HTTP 服务",
            }),
        );
        return;
    };

    match opencode::install_opencode_plugin(&url) {
        Ok(()) => {
            let _ = app.emit(
                "galcode://hook-install",
                serde_json::json!({
                    "tool": "opencode",
                    "ok": true,
                    "message": format!("已注册插件，ingest={}", url),
                }),
            );
        }
        Err(e) => {
            let _ = app.emit(
                "galcode://hook-install",
                serde_json::json!({
                    "tool": "opencode",
                    "ok": false,
                    "message": e,
                }),
            );
        }
    }
}
