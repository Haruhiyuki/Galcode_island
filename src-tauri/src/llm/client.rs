use super::prompt;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

pub fn load_llm_config() -> Option<LlmConfig> {
    let api_key = std::env::var("LLM_API_KEY").ok()?.trim().to_string();
    if api_key.is_empty() {
        return None;
    }
    let base_url = std::env::var("LLM_BASE_URL")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
        .trim_end_matches('/')
        .to_string();
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

fn chat_completion(cfg: &LlmConfig, system: &str, user: &str) -> Result<String, String> {
    let client = http_client()?;
    let url = format!("{}/chat/completions", cfg.base_url);
    let body = ChatRequest {
        model: &cfg.model,
        messages: vec![
            Message {
                role: "system",
                content: system,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryResult {
    pub summary: String,
    pub emotion: String,
}

pub fn generate_summary_emotion(cfg: &LlmConfig, user_zh: &str, agent_output_zh: &str) -> Result<SummaryResult, String> {
    let user = format!(
        "【用户原始需求（中文）】\n{}\n\n【Agent 输出（中文）】\n{}",
        user_zh, agent_output_zh
    );
    let text = chat_completion(cfg, prompt::summary_system_prompt(), &user)?;
    let mut parts = text.split("\n\n").map(|s| s.trim().to_string());
    let summary = parts.next().unwrap_or_default();
    let emotion = parts.next().unwrap_or_else(|| "任务搞定啦，继续保持！".into());
    Ok(SummaryResult { summary, emotion })
}

pub fn suggest_next_steps(cfg: &LlmConfig, user_zh: &str, agent_output: &str) -> Result<String, String> {
    let system = "You are a senior mentor. Reply in lively Simplified Chinese. Give concise bullets: improvements, next steps, risks. No markdown code fences unless showing short snippets.";
    let user = format!(
        "【用户原始需求（中文）】\n{}\n\n【Agent 输出】\n{}",
        user_zh, agent_output
    );
    chat_completion(cfg, system, &user)
}
