use super::prompt;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Default)]
pub struct GlobalLlmSettings {
    pub base_url: String,
    pub api_key: String,
    pub nickname: String,
    pub system_prompt: String,
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

pub fn update_global_settings(base_url: String, api_key: String, nickname: String, system_prompt: String) {
    if let Ok(mut settings) = get_global_settings().lock() {
        if !base_url.is_empty() {
            settings.base_url = base_url;
        }
        if !api_key.is_empty() {
            settings.api_key = api_key; // Update or keep old
        }
        settings.nickname = nickname;
        settings.system_prompt = system_prompt;
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
    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    Some(LlmConfig {
        base_url,
        api_key,
        model,
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
    chat_completion(cfg, prompt::translate_zh_to_en_system(), text)
}

pub fn translate_en_to_zh(cfg: &LlmConfig, text: &str) -> Result<String, String> {
    chat_completion(cfg, prompt::translate_en_to_zh_system(), text)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSummaryResult {
    pub mode: String,
    pub emotion_speech: String,
    pub summary_translation: String,
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
