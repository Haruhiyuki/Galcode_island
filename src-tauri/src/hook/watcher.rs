//! Optional notify-based watcher when `GALCODE_HOOK_LOG_PATH` points at a JSONL file.

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;

pub fn try_spawn_hook_log_watcher() {
    let Ok(path) = std::env::var("GALCODE_HOOK_LOG_PATH") else {
        return;
    };
    let pb = PathBuf::from(path);
    let parent = pb.parent().unwrap_or_else(|| std::path::Path::new(".")).to_path_buf();
    if !parent.exists() {
        log::warn!("GALCODE_HOOK_LOG_PATH parent missing: {:?}", parent);
        return;
    }

    let (tx, rx) = mpsc::channel();
    let watcher = match RecommendedWatcher::new(tx, Config::default()) {
        Ok(w) => w,
        Err(e) => {
            log::error!("notify watcher init failed: {}", e);
            return;
        }
    };

    let mut watcher = watcher;
    if let Err(e) = watcher.watch(&parent, RecursiveMode::NonRecursive) {
        log::error!("notify watch failed: {}", e);
        return;
    }

    std::thread::spawn(move || {
        let _keep_alive = watcher;
        for evt in rx {
            log::debug!("hook log fs event: {:?}", evt);
        }
    });
}
