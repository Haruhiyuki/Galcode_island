//! Local HTTP endpoint so the OpenCode plugin (galcode-opencode.js) can POST hook payloads.

use crate::hook::ingest::process_hook_json;
use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use tauri::AppHandle;

static HOOK_PORT: OnceLock<u16> = OnceLock::new();

#[derive(Clone)]
struct HookServerState {
    app: AppHandle,
    state: Arc<AppState>,
}

pub fn hook_listen_port() -> Option<u16> {
    HOOK_PORT.get().copied()
}

pub fn hook_post_url() -> Option<String> {
    hook_listen_port().map(|p| format!("http://127.0.0.1:{}/hook", p))
}

/// Bind first free port in range and serve `/hook` + `/health` on a background thread.
pub fn spawn_hook_http_server(app: AppHandle, state: Arc<AppState>) {
    for port in 17888u16..17930 {
        let addr = format!("127.0.0.1:{}", port);
        let listener = match std::net::TcpListener::bind(&addr) {
            Ok(l) => l,
            Err(_) => continue,
        };
        let _ = listener.set_nonblocking(true);
        let _ = HOOK_PORT.set(port);
        log::info!("galcode hook HTTP server on {}", addr);

        let st = HookServerState {
            app: app.clone(),
            state: Arc::clone(&state),
        };

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(1)
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    log::error!("hook server runtime: {}", e);
                    return;
                }
            };
            rt.block_on(async move {
                let tok_listener = match tokio::net::TcpListener::from_std(listener) {
                    Ok(l) => l,
                    Err(e) => {
                        log::error!("hook TcpListener::from_std: {}", e);
                        return;
                    }
                };
                let router = Router::new()
                    .route("/hook", post(post_hook))
                    .route("/health", get(|| async { StatusCode::OK }))
                    .with_state(st);
                let _ = axum::serve(tok_listener, router).await;
            });
        });
        return;
    }
    log::warn!("galcode hook HTTP server: no free port in 17888..17929");
}

async fn post_hook(State(st): State<HookServerState>, body: axum::Json<Value>) -> StatusCode {
    let line = match serde_json::to_string(&body.0) {
        Ok(s) => s,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    process_hook_json(&st.app, &st.state, &line);
    StatusCode::NO_CONTENT
}
