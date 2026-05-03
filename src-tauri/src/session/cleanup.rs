use crate::AppState;
use std::sync::Arc;
use tauri::Emitter;

pub fn spawn_idle_cleanup_loop(app: tauri::AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(120));
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
