use super::config::AgentConfig;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const ENV_OPENCODE_CLI: &str = "OPENCODE_CLI";

fn sibling_opencode_names() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["opencode-cli.exe", "opencode.exe"]
    }
    #[cfg(not(windows))]
    {
        &["opencode-cli", "opencode"]
    }
}

fn path_lookup_names() -> &'static [&'static str] {
    sibling_opencode_names()
}

/// Resolve OpenCode CLI binary: `OPENCODE_CLI` → next to current exe → PATH.
pub fn resolve_opencode_executable() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var(ENV_OPENCODE_CLI) {
        let pb = PathBuf::from(p.trim());
        if pb.is_file() {
            return Ok(pb);
        }
        return Err(format!(
            "{} 指向的文件不存在: {}",
            ENV_OPENCODE_CLI,
            pb.display()
        ));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for name in sibling_opencode_names() {
                let cand = parent.join(name);
                if cand.is_file() {
                    return Ok(cand);
                }
            }
        }
    }

    if let Some(found) = find_executable_in_path(path_lookup_names()) {
        return Ok(found);
    }

    Err(
        "找不到 OpenCode CLI。请将 opencode-cli.exe（Windows）或 opencode 与 galcode_island 放在同一目录，或设置环境变量 OPENCODE_CLI 为可执行文件完整路径。"
            .into(),
    )
}

fn find_executable_in_path(names: &[&str]) -> Option<PathBuf> {
    let path_os = std::env::var_os("PATH")?;
    #[cfg(windows)]
    let sep = ';';
    #[cfg(not(windows))]
    let sep = ':';

    for dir in path_os.to_string_lossy().split(sep) {
        let dir = Path::new(dir.trim());
        if dir.as_os_str().is_empty() {
            continue;
        }
        for name in names {
            let p = dir.join(name);
            if is_executable_candidate(&p) {
                return Some(p);
            }
        }
    }
    None
}

fn is_executable_candidate(p: &Path) -> bool {
    match std::fs::metadata(p) {
        Ok(m) => m.is_file(),
        Err(_) => false,
    }
}

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

/// Spawn `opencode run --format json …`. Uses `cfg.executable` when non-empty, otherwise [`resolve_opencode_executable`].
pub fn spawn_opencode_process(
    cfg: &AgentConfig,
    cwd: &Path,
    task_en: &str,
) -> Result<std::process::Child, String> {
    let exe: PathBuf = if cfg.executable.trim().is_empty() {
        resolve_opencode_executable()?
    } else {
        PathBuf::from(cfg.executable.trim())
    };
    if !exe.is_file() {
        return Err(format!("OpenCode 可执行文件不存在: {}", exe.display()));
    }

    let mut cmd = Command::new(&exe);
    cmd.args(["run", "--format", "json", "--dir"])
        .arg(cwd.as_os_str())
        .arg("--dangerously-skip-permissions")
        .arg(task_en)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    for (k, v) in &cfg.env_vars {
        cmd.env(k, v);
    }
    cmd.spawn()
        .map_err(|e| format!("启动 OpenCode 失败: {}（executable={}）", e, exe.display()))
}
