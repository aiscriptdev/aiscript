mod agent;
mod prompt;

use std::env;

pub use agent::{Agent, run_agent};
use openai_api_rs::v1::{api::OpenAIClient, common::GPT3_5_TURBO};
pub use prompt::{PromptConfig, prompt_with_config};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AiConfig {
    pub openai: Option<ModelConfig>,
    pub anthropic: Option<ModelConfig>,
    pub deepseek: Option<ModelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    pub api_key: String,
    pub model: Option<String>,
}

// deepseek-chat
const DEEPSEEK_CHAT: &str = "deepseek-chat";

/// We use OPENAI_API_KEY as default,
/// buf if don't have OPENAI_API_KEY, we use DEEPSEEK_API_KEY and DEEPSEEK_API_ENDPOINT both
#[allow(unused)]
pub(crate) fn openai_client() -> OpenAIClient {
    if let Ok(api_key) = env::var("OPENAI_API_KEY") {
        return OpenAIClient::builder()
            .with_api_key(api_key)
            .build()
            .unwrap();
    }
    if let Ok(api_key) = env::var("DEEPSEEK_API_KEY") {
        let api_endpoint =
            env::var("DEEPSEEK_API_ENDPOINT").unwrap_or("https://api.deepseek.com".to_string());
        return OpenAIClient::builder()
            .with_api_key(api_key)
            .with_endpoint(api_endpoint)
            .build()
            .unwrap();
    }
    panic!("No API key or endpoint found.");
}

pub(crate) fn default_model() -> &'static str {
    if env::var("OPENAI_API_KEY").is_ok() {
        GPT3_5_TURBO
    } else if env::var("DEEPSEEK_API_KEY").is_ok() {
        DEEPSEEK_CHAT
    } else {
        panic!("No API key found.");
    }
}
