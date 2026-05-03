use super::config::AgentConfig;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Absolute path to `scripts/demo_agent.py` (must not depend on process `cwd` — Python is spawned with `current_dir` = 用户项目).
pub fn resolve_demo_script() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("AGENT_SCRIPT") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return pb.canonicalize().map_err(|e| format!("AGENT_SCRIPT 无法解析: {e}"));
        }
    }
    for candidate in [Path::new("scripts/demo_agent.py"), Path::new("../scripts/demo_agent.py")] {
        if candidate.is_file() {
            return candidate
                .canonicalize()
                .map_err(|e| format!("Demo 脚本绝对路径失败: {e}"));
        }
    }
    let from_repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("internal: CARGO_MANIFEST_DIR has no parent")?
        .join("scripts")
        .join("demo_agent.py");
    if from_repo.is_file() {
        return from_repo
            .canonicalize()
            .map_err(|e| format!("Demo 脚本绝对路径失败: {e}"));
    }
    Err(
        "找不到 scripts/demo_agent.py。请在应用仓库根保留 scripts/，或设置环境变量 AGENT_SCRIPT 为脚本的绝对路径。".into(),
    )
}

fn run_demo_python(
    program: &str,
    args: &[&str],
    script: &str,
    task_en: &str,
    cwd: &Path,
    env: &std::collections::HashMap<String, String>,
) -> std::io::Result<std::process::Child> {
    let mut cmd = Command::new(program);
    /* 任务经 `GALCODE_TASK` 传 UTF-8 全文，避免 Windows 命令行 argv 上中文被错误解码为 `` 等。 */
    cmd.env("GALCODE_TASK", task_en)
        .env("PYTHONIOENCODING", "utf-8");
    if cfg!(windows) {
        cmd.env("PYTHONUTF8", "1");
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.args(args)
        .arg(script)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    cmd.spawn()
}

/// Windows 上未配置 `PYTHON` 时，`python` 常不在 PATH（会报 9009）。按顺序尝试 `py -3`、Python Launcher、`python`。
pub fn spawn_demo_process(cfg: &AgentConfig, cwd: &Path, script: &Path, task_en: &str) -> Result<std::process::Child, String> {
    if !script.exists() {
        return Err(format!(
            "找不到 Demo Agent 脚本: {}（可设置 AGENT_SCRIPT）",
            script.display()
        ));
    }
    let script = script.to_str().ok_or("脚本路径无效")?;
    if let Ok(py) = std::env::var("PYTHON") {
        return run_demo_python(&py, &["-u"], script, task_en, cwd, &cfg.env_vars).map_err(|e| {
            format!(
                "启动 Agent 失败: {e}（PYTHON={py}。若提示找不到命令，请改为 python.exe 的绝对路径。）"
            )
        });
    }
    /* 未设 PYTHON 时，按平台依次尝试，避免 Windows 上仅安装「python.org 安装包」时只有 `py` 在 PATH 而无 `python`。 */
    let mut last_err: Option<String> = None;
    let attempts: &[(&str, &[&str])] = if cfg!(windows) {
        &[
            ("py", &["-3", "-u"]),
            ("python", &["-u"]),
            ("python3", &["-u"]),
        ]
    } else {
        &[
            ("python3", &["-u"]),
            ("python", &["-u"]),
        ]
    };
    for &(program, args) in attempts {
        match run_demo_python(program, args, script, task_en, cwd, &cfg.env_vars) {
            Ok(child) => return Ok(child),
            Err(e) => {
                last_err = Some(format!("{program}: {e}"));
            }
        }
    }
    Err(format!(
        "未找到可用 Python 解释器。已尝试: {}。请安装 Python 3 并勾选 “Add to PATH”，或设置环境变量 PYTHON=python.exe 的绝对路径。",
        last_err.unwrap_or_else(|| "（无）".to_string())
    ))
}

