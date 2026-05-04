// Demo Agent 启动器 — 纯 python 脚本路径，用于不依赖外部 CLI 的烟测。
// 真正的 Claude / OpenCode / Codex 接入位于 agent/{claude,opencode,codex}.rs，
// 走 stream-json / HTTP / JSON-RPC 直连，不再经过这个文件。

use super::config::AgentConfig;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn resolve_demo_script() -> PathBuf {
    if let Ok(p) = std::env::var("AGENT_SCRIPT") {
        let pb = PathBuf::from(&p);
        if pb.exists() {
            return pb;
        }
    }
    for candidate in [
        Path::new("scripts/demo_agent.py"),
        Path::new("../scripts/demo_agent.py"),
    ] {
        if candidate.exists() {
            return candidate.to_path_buf();
        }
    }
    PathBuf::from("scripts/demo_agent.py")
}

pub fn spawn_demo_process(
    cfg: &AgentConfig,
    cwd: &Path,
    script: &Path,
    task_en: &str,
) -> Result<std::process::Child, String> {
    if !script.exists() {
        return Err(format!(
            "Cannot find Demo Agent script: {} (set AGENT_SCRIPT env var to override)",
            script.display()
        ));
    }
    let mut cmd = Command::new(&cfg.executable);
    cmd.args([
        "-u",
        script.to_str().ok_or("Invalid script path")?,
        "--task",
    ])
    .arg(task_en)
    .current_dir(cwd)
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit());
    for (k, v) in &cfg.env_vars {
        cmd.env(k, v);
    }
    cmd.spawn()
        .map_err(|e| format!("Failed to start Agent: {} (executable={})", e, cfg.executable))
}
