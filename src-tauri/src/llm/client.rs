use super::prompt;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct GlobalLlmSettings {
    pub base_url: String,
    pub api_key: String,
    pub nickname: String,
    pub system_prompt: String,
    /// "openai" / "deepseek" / "moonshot" / "qwen" / "zhipu" / "custom" 等。
    /// 仅用作前端 UI hint，后端 chat_completion 用的是 OpenAI 兼容格式，
    /// 真正决定行为的是 base_url + model + thinking。
    pub provider: String,
    /// 模型 ID，如 "deepseek-chat" / "deepseek-reasoner" / "gpt-4o-mini"。
    pub model: String,
    /// 思考模式（reasoning / chain-of-thought），默认关。
    /// 启用时 chat body 里加 `enable_thinking: true` 字段——DeepSeek 等服务商
    /// 识别它启用 reasoning_content；OpenAI 等忽略未知字段，无害。
    pub thinking: bool,
}

static GLOBAL_LLM_SETTINGS: OnceLock<Mutex<GlobalLlmSettings>> = OnceLock::new();

fn get_global_settings() -> &'static Mutex<GlobalLlmSettings> {
    GLOBAL_LLM_SETTINGS.get_or_init(|| Mutex::new(GlobalLlmSettings::default()))
}

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub thinking: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn update_global_settings(
    base_url: String,
    api_key: String,
    nickname: String,
    system_prompt: String,
    provider: String,
    model: String,
    thinking: bool,
) {
    if let Ok(mut settings) = get_global_settings().lock() {
        if !base_url.is_empty() {
            settings.base_url = base_url;
        }
        if !api_key.is_empty() {
            settings.api_key = api_key;
        }
        settings.nickname = nickname;
        settings.system_prompt = system_prompt;
        settings.provider = provider;
        if !model.is_empty() {
            settings.model = model;
        }
        settings.thinking = thinking;
    }
}

pub fn load_llm_config() -> Option<LlmConfig> {
    let mut api_key = String::new();
    let mut base_url = String::new();
    let mut model = String::new();
    let mut thinking = false;

    if let Ok(settings) = get_global_settings().lock() {
        api_key = settings.api_key.clone();
        base_url = settings.base_url.clone();
        model = settings.model.clone();
        thinking = settings.thinking;
    }

    if api_key.is_empty() {
        api_key = std::env::var("LLM_API_KEY").ok()?.trim().to_string();
    }
    if api_key.is_empty() {
        return None;
    }

    if base_url.is_empty() {
        base_url = std::env::var("LLM_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
            .trim_end_matches('/')
            .to_string();
    }
    if model.is_empty() {
        model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    }
    Some(LlmConfig {
        base_url,
        api_key,
        model,
        thinking,
    })
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    temperature: f32,
    /// DeepSeek 等服务商识别此字段开启 reasoning；OpenAI 兼容服务忽略未知字段。
    /// 只在用户开启思考模式时序列化（false 时跳过避免改变 default 行为）。
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    enable_thinking: bool,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: RespMessage,
}

#[derive(Deserialize)]
struct RespMessage {
    content: String,
}

fn chat_completion(
    cfg: &LlmConfig,
    base_system: &str,
    user: &str,
    thinking_override: Option<bool>,
) -> Result<String, String> {
    let effective_thinking = thinking_override.unwrap_or(cfg.thinking);
    let client = http_client()?;
    let url = format!("{}/chat/completions", cfg.base_url);

    let mut custom_system = String::new();
    if let Ok(settings) = get_global_settings().lock() {
        let nickname = if settings.nickname.is_empty() {
            "部员"
        } else {
            &settings.nickname
        };
        if !settings.system_prompt.is_empty() {
            custom_system = format!(
                "用户称呼：{}\n用户设定的悄悄话(系统提示词)：{}\n\n---\n",
                nickname, settings.system_prompt
            );
        } else {
            custom_system = format!("用户称呼：{}\n\n---\n", nickname);
        }
    }
    let final_system = format!("{}{}", custom_system, base_system);

    let body = ChatRequest {
        model: &cfg.model,
        messages: vec![
            Message {
                role: "system",
                content: &final_system,
            },
            Message {
                role: "user",
                content: user,
            },
        ],
        temperature: 0.3,
        enable_thinking: effective_thinking,
    };
    eprintln!(
        "[llm] POST {} model={} thinking={} prompt_chars={}",
        url,
        cfg.model,
        effective_thinking,
        user.chars().count()
    );
    let started = std::time::Instant::now();
    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", cfg.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| {
            eprintln!("[llm] send error: {}", e);
            e.to_string()
        })?;
    let status = res.status();
    let txt = res.text().map_err(|e| e.to_string())?;
    eprintln!(
        "[llm] response {} in {}ms (body_len={})",
        status,
        started.elapsed().as_millis(),
        txt.len()
    );
    if !status.is_success() {
        return Err(format!("LLM HTTP {}: {}", status, txt));
    }
    let parsed: ChatResponse = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Empty LLM response".to_string())
}

pub fn translate_zh_to_en(cfg: &LlmConfig, text: &str) -> Result<String, String> {
    // 翻译是机械任务，强制关思考——否则 DeepSeek 等会把简单翻译当 reasoning 跑
    // 几十秒，整个 launch 卡住没反馈。
    chat_completion(cfg, prompt::translate_zh_to_en_system(), text, Some(false))
}

pub fn translate_en_to_zh(cfg: &LlmConfig, text: &str) -> Result<String, String> {
    chat_completion(cfg, prompt::translate_en_to_zh_system(), text, Some(false))
}

/// LLM 输出契约用 snake_case（见 prompt 模板里要求的 JSON 结构），
/// 这里直接默认 snake_case，不要加 rename_all。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummaryResult {
    pub mode: String,
    pub emotion_speech: String,
    pub summary_translation: String,
    pub next_options: Vec<String>,
}

pub fn generate_agent_summary(
    cfg: &LlmConfig,
    user_zh: &str,
    agent_output_zh: &str,
) -> Result<AgentSummaryResult, String> {
    let user = format!(
        "【用户原始需求】\n{}\n\n【Agent 输出】\n{}",
        user_zh, agent_output_zh
    );
    let text = chat_completion(cfg, prompt::haruhi_system_prompt(), &user, None)?;
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: AgentSummaryResult = serde_json::from_str(cleaned)
        .map_err(|e| format!("JSON Parse Error: {}, Raw: {}", e, cleaned))?;

    Ok(parsed)
}

// ---------------------------------------------------------------------------
// 模型列表拉取（OpenAI 兼容 /v1/models 端点）
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ModelListResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

/// 从给定 `base_url + api_key` 拉取 `GET /v1/models` 列表。
/// DeepSeek / OpenAI / Moonshot / 通义 / 智谱 等都遵循 OpenAI 兼容格式。
/// 返回模型 id 列表（按字典序排序）；服务商如果返回不规范，前端按需 fallback 到手输。
pub fn list_models(base_url: &str, api_key: &str) -> Result<Vec<String>, String> {
    if api_key.trim().is_empty() {
        return Err("API Key 为空".into());
    }
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let res = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key.trim()))
        .send()
        .map_err(|e| format!("拉取模型列表失败: {e}"))?;
    let status = res.status();
    let txt = res.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status, txt));
    }
    let parsed: ModelListResponse = serde_json::from_str(&txt)
        .map_err(|e| format!("解析模型列表失败: {} / Raw: {}", e, txt.chars().take(200).collect::<String>()))?;
    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    Ok(ids)
}
