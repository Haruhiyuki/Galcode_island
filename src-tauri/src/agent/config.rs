use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

pub fn preset_opencode() -> AgentConfig {
    AgentConfig {
        name: "opencode".into(),
        executable: "opencode".into(),
        args: vec![
            "run".into(),
            "--format".into(),
            "json".into(),
        ],
        env_vars: HashMap::new(),
    }
}

#[allow(dead_code)]
pub fn preset_claude_code() -> AgentConfig {
    AgentConfig {
        name: "claude-code".into(),
        executable: "claude".into(),
        args: vec![],
        env_vars: HashMap::new(),
    }
}
