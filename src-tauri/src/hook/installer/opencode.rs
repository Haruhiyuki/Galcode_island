use std::fs;
use std::path::{Path, PathBuf};

const PLUGIN_TEMPLATE: &str = include_str!("../../../resources/hooks/galcode-opencode.js");
/// Must match the default URL string embedded in `galcode-opencode.js`.
const PLACEHOLDER_HOOK_URL: &str = "http://127.0.0.1:17888/hook";

pub fn install_opencode_plugin(hook_url: &str) -> Result<(), String> {
    let home = dirs::home_dir().ok_or_else(|| "无法解析用户主目录".to_string())?;
    let config_dir = home.join(".config").join("opencode");
    if !config_dir.is_dir() {
        log::info!("未检测到 OpenCode 配置目录，跳过插件安装 ({})", config_dir.display());
        return Ok(());
    }

    let plugins_dir = config_dir.join("plugins");
    fs::create_dir_all(&plugins_dir).map_err(|e| e.to_string())?;

    let plugin_js = PLUGIN_TEMPLATE.replace(PLACEHOLDER_HOOK_URL, hook_url);
    let plugin_path = plugins_dir.join("galcode-opencode.js");
    fs::write(&plugin_path, plugin_js.as_bytes()).map_err(|e| e.to_string())?;

    let file_url = path_to_file_url(&plugin_path)?;

    let jsonc = config_dir.join("opencode.jsonc");
    let json_new = config_dir.join("opencode.json");
    let target = if jsonc.exists() {
        jsonc
    } else {
        json_new
    };

    merge_plugin_entry(&target, &file_url)?;

    let legacy = config_dir.join("config.json");
    if legacy.exists() {
        strip_our_refs_from_legacy(&legacy)?;
    }

    log::info!("OpenCode 插件已写入 {}", plugin_path.display());
    Ok(())
}

fn path_to_file_url(p: &Path) -> Result<String, String> {
    let abs = p.canonicalize().map_err(|e| e.to_string())?;
    let mut s = abs.to_string_lossy().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_string();
    }
    Ok(format!("file:///{}", s))
}

fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escape {
                escape = false;
                continue;
            }
            if c == '\\' {
                escape = true;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            while let Some(x) = chars.next() {
                if x == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(x) = chars.next() {
                if x == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn backup_once(path: &Path, original: &str) -> Result<(), String> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("config");
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let fname = e.file_name().to_string_lossy().to_string();
            if fname.starts_with(&format!("{}.galcode.bak.", name)) {
                return Ok(());
            }
        }
    }
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let bak: PathBuf = dir.join(format!("{}.galcode.bak.{}", name, stamp));
    fs::write(&bak, original.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

fn merge_plugin_entry(config_path: &Path, plugin_ref: &str) -> Result<(), String> {
    let original = if config_path.exists() {
        fs::read_to_string(config_path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };

    let stripped = strip_jsonc_comments(&original);
    let mut map: serde_json::Map<String, serde_json::Value> = if stripped.trim().is_empty() {
        serde_json::Map::new()
    } else {
        serde_json::from_str(&stripped).map_err(|_| {
            format!(
                "拒绝写入：{} 不是合法 JSON（请先修复后再试）",
                config_path.display()
            )
        })?
    };

    let plugins_val = map
        .entry("plugin".to_string())
        .or_insert(serde_json::json!([]));
    let arr = plugins_val
        .as_array_mut()
        .ok_or_else(|| "\"plugin\" 字段必须是数组".to_string())?;

    arr.retain(|v| {
        v.as_str()
            .map(|s| {
                !s.contains("galcode-opencode") && !s.contains("codeisland-opencode")
            })
            .unwrap_or(true)
    });
    if !arr.iter().any(|v| v.as_str() == Some(plugin_ref)) {
        arr.push(serde_json::Value::String(plugin_ref.to_string()));
    }

    map.entry("$schema".to_string())
        .or_insert(serde_json::json!("https://opencode.ai/config.json"));

    let merged =
        serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(|e| e.to_string())?;

    if !original.trim().is_empty() {
        backup_once(config_path, &original)?;
    }
    fs::write(config_path, format!("{}\n", merged)).map_err(|e| e.to_string())?;
    Ok(())
}

fn strip_our_refs_from_legacy(legacy: &Path) -> Result<(), String> {
    let original = fs::read_to_string(legacy).map_err(|e| e.to_string())?;
    let stripped = strip_jsonc_comments(&original);
    let Ok(mut map) =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&stripped)
    else {
        return Ok(());
    };
    if let Some(serde_json::Value::Array(arr)) = map.get_mut("plugin") {
        arr.retain(|v| {
            v.as_str()
                .map(|s| {
                    !s.contains("galcode-opencode") && !s.contains("codeisland-opencode")
                })
                .unwrap_or(true)
        });
    }
    let merged =
        serde_json::to_string_pretty(&serde_json::Value::Object(map)).map_err(|e| e.to_string())?;
    backup_once(legacy, &original)?;
    fs::write(legacy, format!("{}\n", merged)).map_err(|e| e.to_string())?;
    Ok(())
}
