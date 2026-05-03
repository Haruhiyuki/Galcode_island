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
        /* 实际可执行名由 `spawn_demo_process` 在 Windows 上尝试 `py`/`python` 等；此处仅作日志/兼容。 */
        executable: std::env::var("PYTHON").unwrap_or_else(|_| {
            if cfg!(windows) {
                "py".into()
            } else {
                "python3".into()
            }
        }),
        args: vec![],
        env_vars: HashMap::new(),
    }
}
