// CLI binary 解析与 bundled runtime 管理。
//
// 关键踩坑：
//   - Windows Defender 扫描慢 → command_version 加 5min TTL 缓存，消除冗余 spawn
//   - macOS code-signing inode 缓存 → bundled binary 替换前先删后写，确保新 inode
//   - .cmd / .bat / .ps1 wrapper → normalize_requested_binary_path 自动补扩展名
//   - OpenCode XDG Windows 路径不规范 → 强制注入 XDG_*_HOME 到 LocalAppData

use crate::agent::proc::{configure_background_command, trim_output, wait_child_output_with_timeout};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------

pub const DEFAULT_NODE_BINARY: &str = "node";
pub const DEFAULT_OPENCODE_BINARY: &str = "opencode";
pub const DEFAULT_CODEX_BINARY: &str = "codex";
pub const DEFAULT_CLAUDE_BINARY: &str = "claude";

pub const CLI_VERIFY_TIMEOUT: Duration = Duration::from_secs(20);
pub const CLI_VERSION_TIMEOUT: Duration = Duration::from_secs(5);

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
// CLI 版本检测
// ---------------------------------------------------------------------------

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
