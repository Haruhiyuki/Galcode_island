use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub executable: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
}

pub fn preset_demo() -> AgentConfig {
    AgentConfig {
        name: "demo".into(),
        executable: std::env::var("PYTHON").unwrap_or_else(|_| {
            if cfg!(windows) {
                "python".into()
            } else {
                "python3".into()
            }
        }),
        args: vec![],
        env_vars: HashMap::new(),
    }
}
<<<<<<< Updated upstream
<<<<<<< Updated upstream
=======

/// Resolve OpenCode CLI path: `OPENCODE_BIN`, then `GALCODE_OPENCODE_BIN`, then
/// `{app_config_dir}/opencode_executable.txt` (first line, must exist), else `"opencode"`.
pub fn resolve_opencode_executable(app: &AppHandle) -> String {
    if let Ok(v) = std::env::var("OPENCODE_BIN") {
        let t = v.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Ok(v) = std::env::var("GALCODE_OPENCODE_BIN") {
        let t = v.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    match app.path().app_config_dir() {
        Ok(dir) => {
            let file = dir.join("opencode_executable.txt");
            if let Ok(content) = std::fs::read_to_string(&file) {
                let line = content.lines().next().unwrap_or("").trim();
                if !line.is_empty() && Path::new(line).exists() {
                    return line.to_string();
                }
                if !line.is_empty() {
                    log::warn!(
                        "opencode_executable.txt 中的路径不存在，已忽略: {}",
                        line
                    );
                }
            }
        }
        Err(e) => log::warn!("无法解析应用配置目录: {}", e),
    }
    "opencode".into()
}

pub fn opencode_agent_config(app: &AppHandle) -> AgentConfig {
    AgentConfig {
        name: "opencode".into(),
        executable: resolve_opencode_executable(app),
=======

/// Empty `executable` means resolve OpenCode CLI next to the app binary / PATH at spawn time.
pub fn preset_opencode() -> AgentConfig {
    AgentConfig {
        name: "opencode".into(),
        executable: String::new(),
>>>>>>> Stashed changes
        args: vec![],
        env_vars: HashMap::new(),
    }
}
<<<<<<< Updated upstream
>>>>>>> Stashed changes
=======
>>>>>>> Stashed changes
