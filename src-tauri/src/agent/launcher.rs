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

pub fn spawn_demo_process(cfg: &AgentConfig, cwd: &Path, script: &Path, task_en: &str) -> Result<std::process::Child, String> {
    if !script.exists() {
        return Err(format!(
            "找不到 Demo Agent 脚本: {}（可设置 AGENT_SCRIPT）",
            script.display()
        ));
    }
    let mut cmd = Command::new(&cfg.executable);
    cmd.args(["-u", script.to_str().ok_or("脚本路径无效")?, "--task"])
        .arg(task_en)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    for (k, v) in &cfg.env_vars {
        cmd.env(k, v);
    }
    cmd.spawn()
        .map_err(|e| format!("启动 Agent 失败: {}（executable={}）", e, cfg.executable))
}
