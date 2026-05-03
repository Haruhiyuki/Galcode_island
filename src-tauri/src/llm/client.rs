use super::prompt;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct GlobalLlmSettings {
    pub base_url: String,
    pub api_key: String,
    pub nickname: String,
    pub system_prompt: String,
    /// OpenAI-compatible `model` field; empty → infer from base URL or use generic default.
    pub model: String,
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
}

pub fn update_global_settings(
    base_url: String,
    api_key: String,
    nickname: String,
    system_prompt: String,
    model: String,
) {
    if let Ok(mut settings) = get_global_settings().lock() {
        if !base_url.is_empty() {
            settings.base_url = base_url;
        }
        if !api_key.is_empty() {
            settings.api_key = api_key; // Update or keep old
        }
        settings.nickname = nickname;
        settings.system_prompt = system_prompt;
        settings.model = model;
    }
}

fn infer_default_model(base_url: &str) -> String {
    let u = base_url.to_lowercase();
    if u.contains("deepseek") {
        "deepseek-v4-flash".to_string()
    } else {
        "gpt-4o-mini".to_string()
    }
}

pub fn load_llm_config() -> Option<LlmConfig> {
    let mut api_key = String::new();
    let mut base_url = String::new();
    
    if let Ok(settings) = get_global_settings().lock() {
        api_key = settings.api_key.clone();
        base_url = settings.base_url.clone();
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

    let model = std::env::var("LLM_MODEL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            get_global_settings().lock().ok().and_then(|g| {
                let m = g.model.trim();
                if m.is_empty() {
                    None
                } else {
                    Some(m.to_string())
                }
            })
        })
        .unwrap_or_else(|| infer_default_model(&base_url));

    Some(LlmConfig {
        base_url,
        api_key,
        model,
    })
}

static HTTP_CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();

fn http_client() -> Result<&'static Client, String> {
    let result = HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(120))
            .pool_max_idle_per_host(4)
            .build()
            .map_err(|e| e.to_string())
    });
    result.as_ref().map_err(|e| e.clone())
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    temperature: f32,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

/// Normalize `message.content`: string, multimodal array, or null; plus DeepSeek-style fallbacks.
fn text_from_message_content_field(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(s.clone())
            }
        }
        Value::Object(_) => v
            .get("text")
            .or_else(|| v.get("value"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty()),
        Value::Array(parts) => {
            let mut out = String::new();
            for p in parts {
                if let Some(t) = p.get("text").and_then(|x| x.as_str()) {
                    out.push_str(t);
                } else if let Some(Value::String(t)) = p.get("content") {
                    out.push_str(t);
                } else if let Some(t) = p.as_str() {
                    out.push_str(t);
                }
            }
            let t = out.trim();
            if t.is_empty() {
                None
            } else {
                Some(out)
            }
        }
        Value::Null => None,
        _ => None,
    }
}

fn assistant_text_from_choices_body(resp: &Value) -> Option<String> {
    let choice = resp.get("choices")?.as_array()?.first()?;

    if let Some(msg) = choice.get("message") {
        if let Some(c) = msg.get("content") {
            if let Some(s) = text_from_message_content_field(c) {
                return Some(s);
            }
        }
        for key in ["reasoning_content", "reasoning", "thinking"] {
            if let Some(Value::String(s)) = msg.get(key) {
                let t = s.trim();
                if !t.is_empty() {
                    return Some(s.clone());
                }
            }
        }
    }

    if let Some(Value::String(s)) = choice.get("text") {
        let t = s.trim();
        if !t.is_empty() {
            return Some(s.clone());
        }
    }

    None
}

fn assistant_text_from_chat_completion_json(resp: &Value) -> Option<String> {
    assistant_text_from_choices_body(resp).or_else(|| {
        resp.get("data")
            .and_then(|d| assistant_text_from_choices_body(d))
    }).or_else(|| {
        for key in ["output", "result", "text"] {
            if let Some(Value::String(s)) = resp.get(key) {
                let t = s.trim();
                if !t.is_empty() {
                    return Some(s.clone());
                }
            }
        }
        None
    })
}

fn chat_completion(cfg: &LlmConfig, base_system: &str, user: &str) -> Result<String, String> {
    let client = http_client()?;
    let url = format!("{}/chat/completions", cfg.base_url);
    
    // Combine custom system prompt and base system prompt
    let mut custom_system = String::new();
    if let Ok(settings) = get_global_settings().lock() {
        let nickname = if settings.nickname.is_empty() { "部员" } else { &settings.nickname };
        if !settings.system_prompt.is_empty() {
            custom_system = format!("用户称呼：{}\n用户设定的悄悄话(系统提示词)：{}\n\n---\n", nickname, settings.system_prompt);
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
    };
    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", cfg.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let txt = res.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("LLM HTTP {}: {}", status, txt));
    }
    let parsed: Value =
        serde_json::from_str(&txt).map_err(|e| format!("LLM 响应非 JSON: {e}"))?;

    assistant_text_from_chat_completion_json(&parsed).ok_or_else(|| {
        let preview: String = txt.chars().take(1600).collect();
        format!(
            "Empty LLM response（未解析到 choices[].message 的正文）。若使用推理模型，请确认网关返回 content / reasoning_content。原始片段：{}",
            preview
        )
    })
}

pub fn translate_zh_to_en(cfg: &LlmConfig, text: &str) -> Result<String, String> {
    chat_completion(cfg, prompt::translate_zh_to_en_system(), text)
}

pub fn translate_en_to_zh(cfg: &LlmConfig, text: &str) -> Result<String, String> {
    chat_completion(cfg, prompt::translate_en_to_zh_system(), text)
}

/// 与 `prompt::haruhi_system_prompt` 约定的 snake_case JSON 一致；另兼容部分模型返回的 camelCase。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummaryResult {
    pub mode: String,
    #[serde(alias = "emotionSpeech")]
    pub emotion_speech: String,
    #[serde(alias = "summaryTranslation")]
    pub summary_translation: String,
    #[serde(alias = "nextOptions")]
    pub next_options: Vec<String>,
}

pub fn generate_agent_summary(cfg: &LlmConfig, user_zh: &str, agent_output_zh: &str) -> Result<AgentSummaryResult, String> {
    let user = format!(
        "【用户原始需求】\n{}\n\n【Agent 输出】\n{}",
        user_zh, agent_output_zh
    );
    let text = chat_completion(cfg, prompt::haruhi_system_prompt(), &user)?;
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
