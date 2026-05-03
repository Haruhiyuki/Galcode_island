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

/// Resolve OpenCode CLI: `OPENCODE_CLI` if set, else `opencode-cli.exe` (Windows) / `opencode-cli` next to the app exe.
pub fn resolve_opencode_cli_executable() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("OPENCODE_CLI") {
        let pb = PathBuf::from(p.trim());
        if pb.is_file() {
            return Ok(pb);
        }
        return Err(format!(
            "环境变量 OPENCODE_CLI 指向的文件不存在: {}",
            pb.display()
        ));
    }
    let exe = std::env::current_exe().map_err(|e| format!("无法获取当前可执行文件路径: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "当前可执行文件没有父目录".to_string())?;
    let name = if cfg!(windows) {
        "opencode-cli.exe"
    } else {
        "opencode-cli"
    };
    let candidate = dir.join(name);
    if candidate.is_file() {
        return Ok(candidate);
    }
    Err(format!(
        "未找到 OpenCode CLI：请将 {} 与本应用可执行文件放在同一目录（{}），或设置环境变量 OPENCODE_CLI 为 CLI 的绝对路径。",
        name,
        dir.display()
    ))
}

/// `opencode-cli … run --format json --dir <cwd> <task>` — project cwd and process cwd both set to `cwd`.
pub fn spawn_opencode_process(exe: &Path, cwd: &Path, task_en: &str) -> Result<std::process::Child, String> {
    let cwd_str = cwd.to_str().ok_or("工程路径无效（非 UTF-8）")?;
    let mut cmd = Command::new(exe);
    cmd.args(["run", "--format", "json", "--dir", cwd_str])
        .arg(task_en)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    cmd.spawn()
        .map_err(|e| format!("启动 OpenCode 失败: {e}（exe={}）", exe.display()))
}
