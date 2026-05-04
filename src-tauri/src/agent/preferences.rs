// 三个 backend 的运行时偏好（model / effort / proxy / binary）。
// 单例存内存里，前端在 Settings 修改后通过 update_backend_preferences 命令同步。
//
// 持久化：当前**前端**走 zustand persist 写 localStorage，启动时重新 invoke
// 一次同步过来。后端不写文件——避免 JSON 格式漂移以及多窗口写冲突的麻烦。
// 真要做后端持久化（比如 CLI 用户希望在没有前端时也能跑），未来再加 fs IO。

use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Default)]
pub struct BackendSettings {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub proxy: Option<String>,
    pub binary: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AllBackendPreferences {
    pub claude: BackendSettings,
    pub codex: BackendSettings,
    pub opencode: BackendSettings,
}

static GLOBAL_BACKEND_PREFS: OnceLock<Mutex<AllBackendPreferences>> = OnceLock::new();

fn get_global_prefs() -> &'static Mutex<AllBackendPreferences> {
    GLOBAL_BACKEND_PREFS.get_or_init(|| Mutex::new(AllBackendPreferences::default()))
}

/// 把空字符串归一为 None —— 前端 Settings 表单里的空 input 不应当作"显式空值"。
fn normalize(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn update_backend_preferences(
    backend: &str,
    model: Option<String>,
    effort: Option<String>,
    proxy: Option<String>,
    binary: Option<String>,
) -> Result<(), String> {
    let settings = BackendSettings {
        model: normalize(model),
        effort: normalize(effort),
        proxy: normalize(proxy),
        binary: normalize(binary),
    };

    let mut prefs = get_global_prefs()
        .lock()
        .map_err(|_| "Failed to lock backend preferences.".to_string())?;
    match backend {
        "claude" | "claude-code" => prefs.claude = settings,
        "codex" => prefs.codex = settings,
        "opencode" => prefs.opencode = settings,
        other => return Err(format!("Unknown backend: {other}")),
    }
    Ok(())
}

pub fn load_backend_preferences(backend: &str) -> BackendSettings {
    let prefs = match get_global_prefs().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    match backend {
        "claude" | "claude-code" => prefs.claude.clone(),
        "codex" => prefs.codex.clone(),
        "opencode" => prefs.opencode.clone(),
        _ => BackendSettings::default(),
    }
}
