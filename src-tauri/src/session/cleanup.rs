use crate::AppState;
use std::sync::Arc;
use tauri::Emitter;
use tokio::time::{interval, Duration};

pub fn spawn_idle_cleanup_loop(app: tauri::AppHandle, state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(120));
        loop {
            tick.tick().await;
            let removed = {
                let mut mgr = match state.manager.lock() {
                    Ok(g) => g,
                    Err(_) => continue,
                };
                mgr.cleanup_completed_sessions(std::time::Duration::from_secs(30 * 60))
            };
            if !removed.is_empty() {
                let _ = app.emit(
                    "agent://cleanup",
                    serde_json::json!({ "removedSessionIds": removed }),
                );
            }
        }
    });
}
