// 跨平台进程探查与回收工具。
//
// 关键踩坑：
//   - Windows Defender 扫描慢 → command_version 加 5min TTL 缓存（在 binary.rs）
//   - PowerShell 冷启动 1.5s → Windows 上用 tasklist/taskkill 替代
//   - 子进程树孤儿 → kill_child_descendants 递归 + SIGTERM→SIGKILL 兜底
//   - launchd 收养孤儿 → cleanup_stale_runtime_orphans 启动时回收 ppid==1 的残留

use crate::agent::binary::{
    resolve_claude_binary, resolve_codex_binary, resolve_opencode_binary,
};
use std::collections::BTreeSet;
use std::io::Read;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::AppHandle;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// ---------------------------------------------------------------------------
// Windows 控制台窗口压制
// ---------------------------------------------------------------------------

pub fn configure_background_command(command: &mut Command) {
    #[cfg(not(target_os = "windows"))]
    let _ = command;

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

// ---------------------------------------------------------------------------
// 输出辅助
// ---------------------------------------------------------------------------

pub fn trim_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

// ---------------------------------------------------------------------------
// 子进程同步等待（带超时）
// ---------------------------------------------------------------------------

pub fn wait_child_output_with_timeout(
    mut child: Child,
    timeout: Duration,
) -> Result<Output, String> {
    let (stdout_tx, stdout_rx) = mpsc::channel();
    let (stderr_tx, stderr_rx) = mpsc::channel();

    if let Some(mut stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = stdout.read_to_end(&mut buffer);
            let _ = stdout_tx.send(buffer);
        });
    } else {
        let _ = stdout_tx.send(Vec::new());
    }

    if let Some(mut stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = stderr.read_to_end(&mut buffer);
            let _ = stderr_tx.send(buffer);
        });
    } else {
        let _ = stderr_tx.send(Vec::new());
    }

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_rx
                    .recv_timeout(Duration::from_secs(1))
                    .unwrap_or_default();
                let stderr = stderr_rx
                    .recv_timeout(Duration::from_secs(1))
                    .unwrap_or_default();
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let stderr = stderr_rx
                        .recv_timeout(Duration::from_millis(500))
                        .unwrap_or_default();
                    let detail = trim_output(&stderr);
                    return Err(if detail.is_empty() {
                        format!("CLI request timed out after {}s.", timeout.as_secs())
                    } else {
                        format!(
                            "CLI request timed out after {}s. {detail}",
                            timeout.as_secs()
                        )
                    });
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(format!("Failed to poll CLI process: {error}")),
        }
    }
}

// ---------------------------------------------------------------------------
// 端口监听进程探查
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub fn listening_process_ids(port: u16) -> Result<Vec<u32>, String> {
    let output = Command::new("lsof")
        .args(["-ti", &format!("tcp:{port}"), "-sTCP:LISTEN"])
        .output()
        .map_err(|error| {
            format!("Failed to inspect listening processes on port {port}: {error}")
        })?;

    if !output.status.success() && !output.stdout.is_empty() {
        return Err(format!(
            "Failed to inspect listening processes on port {port}: {}",
            trim_output(&output.stderr)
        ));
    }

    Ok(trim_output(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect())
}

#[cfg(target_os = "windows")]
pub fn listening_process_ids(port: u16) -> Result<Vec<u32>, String> {
    let mut command = Command::new("netstat");
    configure_background_command(&mut command);
    let output = command
        .args(["-ano", "-p", "tcp"])
        .output()
        .map_err(|error| {
            format!("Failed to inspect listening processes on port {port}: {error}")
        })?;

    if !output.status.success() {
        return Err(format!(
            "Failed to inspect listening processes on port {port}: {}",
            trim_output(&output.stderr)
        ));
    }

    let port_suffix = format!(":{port}");
    let mut pids = BTreeSet::new();

    for line in trim_output(&output.stdout).lines() {
        let columns: Vec<&str> = line.split_whitespace().collect();
        if columns.len() < 5 {
            continue;
        }

        let local = columns[1];
        let foreign = columns[2];
        let pid = columns[4];

        if !local.ends_with(&port_suffix) {
            continue;
        }

        if foreign != "0.0.0.0:0" && foreign != "[::]:0" && foreign != "*:*" {
            continue;
        }

        if let Ok(pid) = pid.parse::<u32>() {
            pids.insert(pid);
        }
    }

    Ok(pids.into_iter().collect())
}

// ---------------------------------------------------------------------------
// 进程命令行读取
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub fn process_command_line(pid: u32) -> Option<String> {
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| trim_output(&output.stdout))
        .filter(|output| !output.is_empty())
}

#[cfg(target_os = "windows")]
pub fn process_command_line(pid: u32) -> Option<String> {
    // 原实现每次 spawn powershell.exe（冷启动 0.8-1.5s），Windows 上频繁调用
    // （kill_opencode_listeners 要对每个 port 命中的 pid 查一次）会严重拖慢
    // 运行时清理。tasklist.exe 是内建工具，冷启动只需 ~50ms。
    let mut command = Command::new("tasklist.exe");
    configure_background_command(&mut command);
    let output = command
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let line = trim_output(&output.stdout);
    if line.is_empty() || line.contains("No tasks are running") {
        return None;
    }
    let first_field = line.split(',').next()?.trim().trim_matches('"');
    if first_field.is_empty() {
        None
    } else {
        Some(first_field.to_string())
    }
}

// ---------------------------------------------------------------------------
// 杀进程
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub fn kill_pid(pid: u32, signal: &str) -> Result<(), String> {
    let status = Command::new("kill")
        .args([signal, &pid.to_string()])
        .status()
        .map_err(|error| format!("Failed to send {signal} to pid {pid}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("Failed to send {signal} to pid {pid}."))
    }
}

#[cfg(target_os = "windows")]
pub fn kill_pid(pid: u32, _signal: &str) -> Result<(), String> {
    // taskkill /F /PID <pid>：内建工具，~50ms 起；PowerShell 冷启动慢一个数量级，
    // 关闭/切换运行时需要对几个 pid 连续 kill，PowerShell 叠加 1~2 秒卡顿非常明显。
    let mut command = Command::new("taskkill.exe");
    configure_background_command(&mut command);
    let output = command
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .output()
        .map_err(|error| format!("Failed to terminate pid {pid}: {error}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Failed to terminate pid {pid}: {}",
            trim_output(&output.stderr)
        ))
    }
}

/// 递归杀掉指定 PID 下的整棵子进程树。
/// Unix 上用 pgrep 拿子进程列表，先广播 SIGTERM 给一段缓冲时间，再 SIGKILL 兜底。
/// Windows 上 taskkill /T 一步到位。
pub fn kill_child_descendants(pid: u32) {
    #[cfg(unix)]
    {
        fn child_process_ids(parent_pid: u32) -> Vec<u32> {
            let output = match Command::new("pgrep")
                .arg("-P")
                .arg(parent_pid.to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
            {
                Ok(result) => result,
                Err(_) => return Vec::new(),
            };

            if !output.status.success() {
                return Vec::new();
            }

            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.trim().parse::<u32>().ok())
                .collect()
        }

        fn descendant_process_ids(root_pid: u32) -> Vec<u32> {
            let mut pending = vec![root_pid];
            let mut descendants = Vec::new();

            while let Some(current_pid) = pending.pop() {
                let children = child_process_ids(current_pid);
                for child_pid in children {
                    descendants.push(child_pid);
                    pending.push(child_pid);
                }
            }

            descendants
        }

        let descendants = descendant_process_ids(pid);
        for child_pid in descendants.iter().rev() {
            let _ = Command::new("kill")
                .arg("-TERM")
                .arg(child_pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        std::thread::sleep(Duration::from_millis(120));
        for child_pid in descendants.iter().rev() {
            let _ = Command::new("kill")
                .arg("-KILL")
                .arg(child_pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }

    #[cfg(windows)]
    {
        let mut command = Command::new("taskkill");
        configure_background_command(&mut command);
        let _ = command
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/T")
            .arg("/F")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(any(unix, target_os = "windows"))]
pub fn kill_opencode_listeners(ports: &[u16]) -> Result<Vec<u32>, String> {
    let mut candidate_pids = BTreeSet::new();
    for port in ports {
        for pid in listening_process_ids(*port)? {
            candidate_pids.insert(pid);
        }
    }

    let mut killed = Vec::new();
    for pid in candidate_pids {
        let Some(command) = process_command_line(pid) else {
            continue;
        };

        if !command.contains("opencode") {
            continue;
        }

        // opencode 进程树里还挂着 node 子进程（MCP servers、shell 工具等），
        // 先递归清理子孙再杀主进程，避免 grandchildren 被 launchd 收养成残留
        kill_child_descendants(pid);
        let _ = kill_pid(pid, "-TERM");
        killed.push(pid);
    }

    if !killed.is_empty() {
        // 给 SIGTERM 一点时间让 opencode flush；然后 SIGKILL 兜底，避免
        // 某些子进程忽略 TERM 把端口继续占着
        std::thread::sleep(Duration::from_millis(150));
        for pid in &killed {
            let _ = kill_pid(*pid, "-KILL");
        }
    }

    Ok(killed)
}

#[cfg(not(any(unix, target_os = "windows")))]
pub fn kill_opencode_listeners(_ports: &[u16]) -> Result<Vec<u32>, String> {
    Ok(Vec::new())
}

// ---------------------------------------------------------------------------
// 启动孤儿回收
// ---------------------------------------------------------------------------

// 启动时把上一轮崩溃 / 强退留下的 runtime 孤儿全部收割掉。
// 识别已解析的 opencode/codex/claude 原生二进制路径。
// 只要 ppid == 1（被 launchd 收养 = 我们死了它还活着）且 command 命中上面任一
// 标记，就递归杀进程树。
#[cfg(unix)]
pub fn cleanup_stale_runtime_orphans(app: &AppHandle) {
    let opencode_marker = resolve_opencode_binary(app, None)
        .to_string_lossy()
        .to_string();
    let codex_marker = resolve_codex_binary(app, None).to_string_lossy().to_string();
    let claude_marker = resolve_claude_binary(app, None).to_string_lossy().to_string();

    let output = match Command::new("ps")
        .args(["-A", "-o", "pid=,ppid=,command="])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        Ok(result) => result,
        Err(_) => return,
    };
    if !output.status.success() {
        return;
    }

    let current_pid = std::process::id();
    let mut target_pids = BTreeSet::new();
    for raw_line in String::from_utf8_lossy(&output.stdout).lines() {
        // ps 列之间是变长空白，不能用 splitn(whitespace) —— 那样相邻的空白
        // 会被当成空字段，把 ppid 和 command 糊到一起。手动切两刀。
        let line = raw_line.trim_start();
        let (pid_text, rest) = match line.find(char::is_whitespace) {
            Some(idx) => (&line[..idx], line[idx..].trim_start()),
            None => continue,
        };
        let (ppid_text, command) = match rest.find(char::is_whitespace) {
            Some(idx) => (&rest[..idx], rest[idx..].trim_start()),
            None => continue,
        };

        let Ok(pid) = pid_text.parse::<u32>() else {
            continue;
        };
        let Ok(ppid) = ppid_text.parse::<u32>() else {
            continue;
        };
        if pid == current_pid || ppid != 1 {
            continue;
        }

        let hits_native = command.contains(&opencode_marker)
            || command.contains(&codex_marker)
            || command.contains(&claude_marker);

        if hits_native {
            target_pids.insert(pid);
        }
    }

    for pid in target_pids {
        kill_child_descendants(pid);
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        std::thread::sleep(Duration::from_millis(80));
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(not(unix))]
pub fn cleanup_stale_runtime_orphans(_app: &AppHandle) {}
