// 跨 backend 共享的系统工具函数和常量。
// 包含一系列踩坑修出来的细节：
//   - Windows Defender 扫描慢 → command_version 加 5min TTL 缓存
//   - macOS code-signing inode 缓存 → bundled binary 替换前先删后写
//   - 子进程树孤儿 → kill_child_descendants 递归 + SIGTERM→SIGKILL 兜底
//   - PowerShell 冷启动 1.5s → Windows 上用 tasklist/taskkill 替代
//   - PATH 找不到 .cmd wrapper → normalize_requested_binary_path 自动补扩展名
//   - OpenCode XDG 路径 Windows 不规范 → 强制设环境变量到 LocalAppData
//   - HTTP_PROXY 大小写并存 → apply_proxy_env 一次性覆盖 8 个变量
// 修改前请先确认踩坑背景已经消失，否则保留原实现。

use crate::agent::runtime::CliStreamEvent;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{mpsc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------

pub const DEFAULT_NODE_BINARY: &str = "node";
pub const DEFAULT_OPENCODE_BINARY: &str = "opencode";
pub const DEFAULT_CODEX_BINARY: &str = "codex";
pub const DEFAULT_CLAUDE_BINARY: &str = "claude";

pub const CLI_VERIFY_TIMEOUT: Duration = Duration::from_secs(20);
pub const CLI_VERSION_TIMEOUT: Duration = Duration::from_secs(5);

pub const CODEX_APP_SERVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub const CODEX_TURN_TIMEOUT: Duration = Duration::from_secs(1800);
pub const OPENCODE_READY_TIMEOUT: Duration = Duration::from_secs(60);

pub const CLI_STREAM_EVENT: &str = "galcode://cli-output";

// ---------------------------------------------------------------------------
// version 缓存
// ---------------------------------------------------------------------------

// Windows 上 spawn 任何 .exe 都会付出 ~300ms–1.5s 的 Defender 扫描 + 进程初始化开销。
// refreshDesktopIntegration 一次调用会叠加 5 次 --version（node / opencode / claude /
// codex / gemini），bootstrap 完成前还会被 warmup 再补一轮。bundled 二进制在 app 生
// 命周期内是不变的，把成功的版本号缓存起来可以彻底消除这些冗余 spawn。
// TTL 5 分钟是为了给用户通过命令行自升级外部 CLI 一条兜底路径。
const VERSION_CACHE_TTL: Duration = Duration::from_secs(300);
static VERSION_CACHE: LazyLock<Mutex<HashMap<String, (Instant, Option<String>)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn version_cache_get(key: &str) -> Option<Option<String>> {
    let guard = VERSION_CACHE.lock().ok()?;
    let (at, value) = guard.get(key)?.clone();
    if at.elapsed() > VERSION_CACHE_TTL {
        return None;
    }
    Some(value)
}

fn version_cache_put(key: String, value: Option<String>) {
    if let Ok(mut guard) = VERSION_CACHE.lock() {
        guard.insert(key, (Instant::now(), value));
    }
}

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
// CLI 版本检测
// ---------------------------------------------------------------------------

pub fn trim_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

pub fn command_version<S: AsRef<OsStr>>(binary: S, flag: &str, cwd: &Path) -> Option<String> {
    let binary_ref = binary.as_ref();
    let cache_key = format!("v1|{}|{}", binary_ref.to_string_lossy(), flag);
    if let Some(cached) = version_cache_get(&cache_key) {
        return cached;
    }

    let mut command = Command::new(binary_ref);
    configure_background_command(&mut command);
    let result = command
        .arg(flag)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()
        .and_then(|child| wait_child_output_with_timeout(child, CLI_VERSION_TIMEOUT).ok())
        .filter(|output| output.status.success())
        .map(|output| trim_output(&output.stdout))
        .filter(|value| !value.is_empty());

    version_cache_put(cache_key, result.clone());
    result
}

pub fn command_version_with_args(
    binary: &Path,
    leading_args: &[String],
    flag: &str,
    cwd: &Path,
) -> Option<String> {
    let cache_key = format!(
        "va1|{}|{}|{}",
        binary.to_string_lossy(),
        leading_args.join(" "),
        flag
    );
    if let Some(cached) = version_cache_get(&cache_key) {
        return cached;
    }

    let mut command = Command::new(binary);
    configure_background_command(&mut command);
    command.args(leading_args).arg(flag).current_dir(cwd);

    let result = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()
        .and_then(|child| wait_child_output_with_timeout(child, CLI_VERSION_TIMEOUT).ok())
        .filter(|output| output.status.success())
        .map(|output| trim_output(&output.stdout))
        .filter(|value| !value.is_empty());

    version_cache_put(cache_key, result.clone());
    result
}

pub fn opencode_command_version(binary: &Path, cwd: &Path) -> Result<String, String> {
    // 与 command_version 共享同一套缓存，命中则跳过整个 spawn/XDG 准备流程。
    // 只缓存成功结果；失败路径保留原有的 stderr/stdout 诊断细节。
    let cache_key = format!("oc1|{}", binary.to_string_lossy());
    if let Some(Some(cached)) = version_cache_get(&cache_key) {
        return Ok(cached);
    }

    let mut command = Command::new(binary);
    configure_background_command(&mut command);
    apply_opencode_runtime_env(&mut command)?;
    let output = command
        .arg("--version")
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("Failed to inspect OpenCode version: {error}"))?;

    if output.status.success() {
        let version = trim_output(&output.stdout);
        if !version.is_empty() {
            version_cache_put(cache_key, Some(version.clone()));
            return Ok(version);
        }
    }

    let stderr = trim_output(&output.stderr);
    let stdout = trim_output(&output.stdout);
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "unknown error".to_string()
    };

    Err(format!("Failed to inspect OpenCode version: {detail}"))
}

// ---------------------------------------------------------------------------
// 用户目录 / Codex 配置路径
// ---------------------------------------------------------------------------

pub fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

pub fn codex_home_dir() -> Option<PathBuf> {
    user_home_dir().map(|home| home.join(".codex"))
}

pub fn codex_config_file() -> Option<PathBuf> {
    codex_home_dir().map(|home| home.join("config.toml"))
}

pub fn codex_models_cache_file() -> Option<PathBuf> {
    codex_home_dir().map(|home| home.join("models_cache.json"))
}

// ---------------------------------------------------------------------------
// OpenCode XDG 运行时环境 (Windows)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub fn opencode_windows_xdg_home(kind: &str) -> Option<PathBuf> {
    let base = std::env::var_os("GALCODE_OPENCODE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .map(|path| path.join("Galcode").join("opencode"))
        })
        .or_else(|| {
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .map(|path| path.join("Galcode").join("opencode"))
        })
        .or_else(|| user_home_dir().map(|home| home.join(".galcode").join("opencode")))?;

    Some(base.join("xdg").join(kind))
}

pub fn apply_opencode_runtime_env(command: &mut Command) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    let _ = command;

    #[cfg(target_os = "windows")]
    {
        let Some(config_home) = opencode_windows_xdg_home("config") else {
            return Ok(());
        };
        let Some(data_home) = opencode_windows_xdg_home("data") else {
            return Ok(());
        };
        let Some(cache_home) = opencode_windows_xdg_home("cache") else {
            return Ok(());
        };
        let Some(state_home) = opencode_windows_xdg_home("state") else {
            return Ok(());
        };

        for directory in [&config_home, &data_home, &cache_home, &state_home] {
            fs::create_dir_all(directory).map_err(|error| {
                format!(
                    "Failed to prepare OpenCode runtime directory {}: {error}",
                    directory.display()
                )
            })?;
        }

        command.env("XDG_CONFIG_HOME", &config_home);
        command.env("XDG_DATA_HOME", &data_home);
        command.env("XDG_CACHE_HOME", &cache_home);
        command.env("XDG_STATE_HOME", &state_home);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 进程探查 / 杀进程
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub fn listening_process_ids(port: u16) -> Result<Vec<u32>, String> {
    let output = Command::new("lsof")
        .args(["-ti", &format!("tcp:{port}"), "-sTCP:LISTEN"])
        .output()
        .map_err(|error| format!("Failed to inspect listening processes on port {port}: {error}"))?;

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
        .map_err(|error| format!("Failed to inspect listening processes on port {port}: {error}"))?;

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
    // CSV 格式第一列是 image name，如 "opencode.exe"。去掉首尾引号即可。
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

// 启动时把上一轮崩溃 / 强退留下的 runtime 孤儿全部收割掉。
// 识别两类：
//   1) 已解析的 opencode/codex/claude 原生二进制路径
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

// ---------------------------------------------------------------------------
// 平台键 / bundled runtime 路径
// ---------------------------------------------------------------------------

pub fn runtime_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "windows",
        other => other,
    }
}

pub fn runtime_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    }
}

pub fn runtime_key() -> String {
    format!("{}-{}", runtime_platform(), runtime_arch())
}

pub fn runtime_binary_name(base_name: &str) -> String {
    if cfg!(windows) {
        format!("{base_name}.exe")
    } else {
        base_name.to_string()
    }
}

pub fn bundled_runtime_relative_path(kind: &str) -> PathBuf {
    PathBuf::from("runtime")
        .join(runtime_key())
        .join(kind)
        .join(runtime_binary_name(kind))
}

pub fn bundled_runtime_source_candidates(app: &AppHandle, relative: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(relative));
        candidates.push(resource_dir.join("resources").join(relative));
    }

    if let Ok(root) = resolve_project_root(app) {
        candidates.push(root.join("src-tauri").join("resources").join(relative));
    }

    candidates
}

pub fn bundled_runtime_source_path(app: &AppHandle, relative: &Path) -> Option<PathBuf> {
    bundled_runtime_source_candidates(app, relative)
        .into_iter()
        .find(|candidate| candidate.exists())
}

#[cfg(target_os = "windows")]
pub fn is_explicit_binary_path(candidate: &Path) -> bool {
    candidate.is_absolute()
        || candidate
            .parent()
            .map(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(false)
}

pub fn normalize_requested_binary_path(candidate: PathBuf) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if !is_explicit_binary_path(&candidate) {
            return candidate;
        }

        let extension = candidate
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());

        let mut alternatives = Vec::new();
        match extension.as_deref() {
            Some("exe") => return candidate,
            Some("ps1") => {
                alternatives.push(candidate.with_extension("exe"));
                alternatives.push(candidate.with_extension("cmd"));
                alternatives.push(candidate.with_extension("bat"));
            }
            Some("cmd") | Some("bat") => {
                alternatives.push(candidate.with_extension("exe"));
            }
            None => {
                alternatives.push(candidate.with_extension("exe"));
                alternatives.push(candidate.with_extension("cmd"));
                alternatives.push(candidate.with_extension("bat"));
                alternatives.push(candidate.with_extension("ps1"));
            }
            _ => {}
        }

        for alternative in alternatives {
            if alternative.exists() {
                return alternative;
            }
        }
    }

    candidate
}

pub fn stage_bundled_runtime_binary(app: &AppHandle, kind: &str) -> Result<PathBuf, String> {
    let relative = bundled_runtime_relative_path(kind);
    let source = bundled_runtime_source_candidates(app, &relative)
        .into_iter()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| format!("Bundled {kind} runtime is missing."))?;

    // Windows 上不存在 macOS 的代码签名 inode 缓存问题，直接使用安装目录内的运行时
    if cfg!(windows) {
        return Ok(source);
    }

    let data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;
    let destination = data_dir.join(&relative);

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create runtime directory: {error}"))?;
    }

    let should_copy = match (fs::metadata(&source), fs::metadata(&destination)) {
        (Ok(source_meta), Ok(destination_meta)) => {
            source_meta.len() != destination_meta.len()
                || source_meta
                    .modified()
                    .ok()
                    .zip(destination_meta.modified().ok())
                    .map(|(source_time, destination_time)| source_time > destination_time)
                    .unwrap_or(false)
        }
        (Ok(_), Err(_)) => true,
        _ => true,
    };

    if should_copy {
        // 先删除旧文件再复制，确保生成新的 inode。
        // macOS 对同一 inode 的 code-signing 评估结果有缓存，
        // 原地覆写不会刷新缓存，可能导致 SIGKILL。
        let _ = fs::remove_file(&destination);
        fs::copy(&source, &destination)
            .map_err(|error| format!("Failed to stage bundled {kind} runtime: {error}"))?;
    }

    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&destination)
            .map_err(|error| format!("Failed to inspect staged {kind} runtime: {error}"))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&destination, permissions)
            .map_err(|error| format!("Failed to update staged {kind} permissions: {error}"))?;
    }

    Ok(destination)
}

pub fn resolve_node_binary(app: &AppHandle) -> PathBuf {
    stage_bundled_runtime_binary(app, "node")
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_NODE_BINARY))
}

pub fn resolve_opencode_binary(app: &AppHandle, requested: Option<&str>) -> PathBuf {
    let desired = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_OPENCODE_BINARY);

    if desired == DEFAULT_OPENCODE_BINARY {
        return stage_bundled_runtime_binary(app, "opencode")
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_OPENCODE_BINARY));
    }

    normalize_requested_binary_path(PathBuf::from(desired))
}

pub fn resolve_codex_binary(app: &AppHandle, requested: Option<&str>) -> PathBuf {
    let desired = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CODEX_BINARY);

    if desired == DEFAULT_CODEX_BINARY {
        return stage_bundled_runtime_binary(app, "codex")
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_CODEX_BINARY));
    }

    normalize_requested_binary_path(PathBuf::from(desired))
}

pub fn resolve_claude_binary(app: &AppHandle, requested: Option<&str>) -> PathBuf {
    let desired = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CLAUDE_BINARY);

    if desired == DEFAULT_CLAUDE_BINARY {
        return stage_bundled_runtime_binary(app, "claude")
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_CLAUDE_BINARY));
    }

    normalize_requested_binary_path(PathBuf::from(desired))
}

/// 当前项目的 project root：含有 src-tauri 子目录的最近祖先。
/// dev 时是 cwd 上溯，release 时退化为 resource_dir 的祖先。
pub fn resolve_project_root(app: &AppHandle) -> Result<PathBuf, String> {
    fn push_with_ancestors(candidates: &mut Vec<PathBuf>, path: PathBuf) {
        let mut current = Some(path);
        while let Some(candidate) = current {
            if !candidates.iter().any(|existing| existing == &candidate) {
                candidates.push(candidate.clone());
            }
            current = candidate.parent().map(Path::to_path_buf);
        }
    }

    fn is_valid_project_root(candidate: &Path) -> bool {
        candidate.join("src-tauri").exists() && candidate.join("package.json").exists()
    }

    fn candidate_rank(candidate: &Path) -> u8 {
        let in_target = candidate
            .components()
            .any(|component| component.as_os_str() == "target");
        if in_target {
            1
        } else {
            0
        }
    }

    let mut candidates = Vec::new();

    if let Ok(current) = std::env::current_dir() {
        push_with_ancestors(&mut candidates, current);
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        push_with_ancestors(&mut candidates, resource_dir.clone());
        push_with_ancestors(&mut candidates, resource_dir.join("_up_"));
    }

    candidates
        .into_iter()
        .filter(|candidate| is_valid_project_root(candidate))
        .min_by_key(|candidate| (candidate_rank(candidate), candidate.components().count()))
        .ok_or_else(|| "Unable to resolve Galcode project root.".to_string())
}

// ---------------------------------------------------------------------------
// 输出清洗 (CLI stderr 噪音过滤)
// ---------------------------------------------------------------------------

pub fn strip_cli_warning_lines(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("WARNING:")
                && trimmed != "Loaded cached credentials."
                && !trimmed.contains("[STARTUP] Phase 'cli_startup' was started but never ended.")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// 代理环境变量
// ---------------------------------------------------------------------------

pub fn merge_no_proxy() -> String {
    let existing_no_proxy = std::env::var("NO_PROXY").unwrap_or_default();
    let base_no_proxy = "127.0.0.1,localhost";
    if existing_no_proxy.trim().is_empty() {
        base_no_proxy.to_string()
    } else if existing_no_proxy.contains("127.0.0.1") || existing_no_proxy.contains("localhost") {
        existing_no_proxy
    } else {
        format!("{base_no_proxy},{}", existing_no_proxy)
    }
}

pub fn resolve_proxy_value(proxy: Option<&str>) -> Option<String> {
    proxy
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn replace_proxy_scheme(proxy_url: &str, scheme: &str) -> String {
    if let Some((_current_scheme, rest)) = proxy_url.split_once("://") {
        format!("{scheme}://{rest}")
    } else {
        format!("{scheme}://{proxy_url}")
    }
}

pub fn resolve_proxy_urls(proxy: Option<&str>) -> Option<(String, String)> {
    let proxy_url = resolve_proxy_value(proxy)?;
    let lower = proxy_url.to_ascii_lowercase();

    let (http_proxy_url, socks_proxy_url) = if lower.starts_with("socks5://")
        || lower.starts_with("socks5h://")
        || lower.starts_with("socks://")
    {
        (
            replace_proxy_scheme(&proxy_url, "http"),
            replace_proxy_scheme(&proxy_url, "socks5"),
        )
    } else if lower.starts_with("http://") || lower.starts_with("https://") {
        (proxy_url.clone(), replace_proxy_scheme(&proxy_url, "socks5"))
    } else {
        (
            format!("http://{proxy_url}"),
            format!("socks5://{proxy_url}"),
        )
    };

    Some((http_proxy_url, socks_proxy_url))
}

pub fn clear_proxy_env(command: &mut Command) {
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
    ] {
        command.env_remove(key);
    }
}

pub fn apply_proxy_env(command: &mut Command, proxy: Option<&str>) {
    clear_proxy_env(command);
    if let Some((http_proxy_url, socks_proxy_url)) = resolve_proxy_urls(proxy) {
        let no_proxy = merge_no_proxy();
        command.env("HTTP_PROXY", &http_proxy_url);
        command.env("HTTPS_PROXY", &http_proxy_url);
        command.env("ALL_PROXY", &socks_proxy_url);
        command.env("http_proxy", &http_proxy_url);
        command.env("https_proxy", &http_proxy_url);
        command.env("all_proxy", &socks_proxy_url);
        command.env("NO_PROXY", &no_proxy);
        command.env("no_proxy", &no_proxy);
    }
}

// ---------------------------------------------------------------------------
// 终端命令拼接 (用于 open_terminal_command)
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
pub fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(target_os = "windows")]
pub fn terminal_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(not(target_os = "windows"))]
pub fn terminal_quote(value: &str) -> String {
    shell_single_quote(value)
}

#[cfg(target_os = "windows")]
pub fn terminal_env_assignment(key: &str, value: &str) -> String {
    format!(
        "set \"{key}={}\"",
        value.replace('%', "%%").replace('"', "\"\"")
    )
}

pub fn shell_command_text(
    binary: &Path,
    leading_args: &[String],
    trailing_args: &[String],
) -> String {
    let mut parts = Vec::with_capacity(2 + leading_args.len() + trailing_args.len());

    #[cfg(target_os = "windows")]
    {
        let needs_call = binary
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("cmd") || value.eq_ignore_ascii_case("bat"))
            .unwrap_or(false);
        if needs_call {
            parts.push("call".to_string());
        }
    }

    parts.push(terminal_quote(&binary.display().to_string()));
    parts.extend(leading_args.iter().map(|value| terminal_quote(value)));
    parts.extend(trailing_args.iter().map(|value| terminal_quote(value)));
    parts.join(" ")
}

pub fn proxy_env_prefix(proxy: Option<&str>) -> String {
    #[cfg(target_os = "windows")]
    {
        return resolve_proxy_value(proxy)
            .map(|proxy_url| {
                let no_proxy = merge_no_proxy();
                [
                    terminal_env_assignment("HTTP_PROXY", &proxy_url),
                    terminal_env_assignment("HTTPS_PROXY", &proxy_url),
                    terminal_env_assignment("ALL_PROXY", &proxy_url),
                    terminal_env_assignment("http_proxy", &proxy_url),
                    terminal_env_assignment("https_proxy", &proxy_url),
                    terminal_env_assignment("all_proxy", &proxy_url),
                    terminal_env_assignment("NO_PROXY", &no_proxy),
                    terminal_env_assignment("no_proxy", &no_proxy),
                ]
                .join(" && ")
                    + " && "
            })
            .unwrap_or_default();
    }

    #[cfg(not(target_os = "windows"))]
    resolve_proxy_urls(proxy)
        .map(|(http_proxy_url, socks_proxy_url)| {
            let no_proxy = merge_no_proxy();
            format!(
                "HTTP_PROXY={} HTTPS_PROXY={} ALL_PROXY={} http_proxy={} https_proxy={} all_proxy={} NO_PROXY={} no_proxy={} ",
                shell_single_quote(&http_proxy_url),
                shell_single_quote(&http_proxy_url),
                shell_single_quote(&socks_proxy_url),
                shell_single_quote(&http_proxy_url),
                shell_single_quote(&http_proxy_url),
                shell_single_quote(&socks_proxy_url),
                shell_single_quote(&no_proxy),
                shell_single_quote(&no_proxy)
            )
        })
        .unwrap_or_default()
}

pub fn open_terminal_command(command_text: &str, success_message: &str) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("osascript")
            .arg("-e")
            .arg(format!(
                "tell application \"Terminal\" to do script {}",
                serde_json::to_string(command_text)
                    .map_err(|error| format!("Failed to encode terminal command: {error}"))?
            ))
            .arg("-e")
            .arg("tell application \"Terminal\" to activate")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("Failed to open terminal: {error}"))?;

        return Ok(success_message.to_string());
    }

    #[cfg(target_os = "windows")]
    {
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        let mut command = Command::new("cmd.exe");
        command.creation_flags(CREATE_NEW_CONSOLE);
        command
            .args(["/D", "/K", command_text])
            .spawn()
            .map_err(|error| format!("Failed to open terminal: {error}"))?;
        return Ok(success_message.to_string());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("x-terminal-emulator")
            .arg("-e")
            .arg(command_text)
            .spawn()
            .map_err(|error| format!("Failed to open terminal: {error}"))?;
        return Ok(success_message.to_string());
    }
}

// ---------------------------------------------------------------------------
// CLI 流事件发射
// ---------------------------------------------------------------------------

pub fn emit_cli_stream_line(
    app: &AppHandle,
    backend: &str,
    stream_id: &str,
    channel: &str,
    line: &str,
) {
    let _ = app.emit(
        CLI_STREAM_EVENT,
        CliStreamEvent {
            stream_id: stream_id.to_string(),
            backend: backend.to_string(),
            channel: channel.to_string(),
            line: line.to_string(),
            // 多 tab 路由：当前 emit 链路尚未带上 run_id；前端按 stream_id 兜底分发。
            // 后续可以让各 backend 模块在创建 stream 时把 run_id 注册到一个映射，
            // 这里再查映射回填 run_id。先发空字符串维持兼容。
            run_id: String::new(),
        },
    );
}

pub fn emit_cli_stream_json_event(app: &AppHandle, backend: &str, stream_id: &str, value: &Value) {
    if let Ok(line) = serde_json::to_string(value) {
        emit_cli_stream_line(app, backend, stream_id, "stdout", &line);
    }
}

// ---------------------------------------------------------------------------
// JSON 辅助 (RPC id / 嵌套字段读取)
// ---------------------------------------------------------------------------

pub fn json_rpc_id_string(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    None
}

pub fn read_json_string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }

    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn read_nested_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(ToOwned::to_owned)
}

// ---------------------------------------------------------------------------
// 子进程同步等待 (带超时)
// ---------------------------------------------------------------------------

pub fn wait_child_output_with_timeout(mut child: Child, timeout: Duration) -> Result<Output, String> {
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
                let stdout = stdout_rx.recv_timeout(Duration::from_secs(1)).unwrap_or_default();
                let stderr = stderr_rx.recv_timeout(Duration::from_secs(1)).unwrap_or_default();
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
                        format!("CLI request timed out after {}s. {detail}", timeout.as_secs())
                    });
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(format!("Failed to poll CLI process: {error}")),
        }
    }
}
