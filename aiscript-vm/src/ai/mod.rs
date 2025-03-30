mod agent;
mod prompt;

use std::env;

pub use agent::{Agent, run_agent};
use openai_api_rs::v1::{api::OpenAIClient, common::GPT3_5_TURBO};
pub use prompt::{PromptConfig, prompt_with_config};

use serde::Deserialize;

// Deepseek
const DEEPSEEK_API_ENDPOINT: &str = "https://api.deepseek.com/v1";
const DEEPSEEK_V3: &str = "deepseek-chat";

// Anthropic
const ANTHROPIC_API_ENDPOINT: &str = "https://api.anthropic.com/v1";
const CLAUDE_3_5_SONNET: &str = "claude-3-5-sonnet-latest";

#[derive(Debug, Clone, Deserialize)]
pub enum AiConfig {
    #[serde(rename = "openai")]
    OpenAI(ModelConfig),
    #[serde(rename = "anthropic")]
    Anthropic(ModelConfig),
    #[serde(rename = "deepseek")]
    DeepSeek(ModelConfig),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    pub api_key: String,
    pub model: Option<String>,
}

impl AiConfig {
    pub(crate) fn take_model(&mut self) -> Option<String> {
        match self {
            Self::OpenAI(ModelConfig { model, .. }) => model.take(),
            Self::Anthropic(ModelConfig { model, .. }) => model.take(),
            Self::DeepSeek(ModelConfig { model, .. }) => model.take(),
        }
    }

    pub(crate) fn set_model(&mut self, m: String) {
        match self {
            Self::OpenAI(ModelConfig { model, .. }) => model.replace(m),
            Self::Anthropic(ModelConfig { model, .. }) => model.replace(m),
            Self::DeepSeek(ModelConfig { model, .. }) => model.replace(m),
        };
    }
}

#[allow(unused)]
pub(crate) fn openai_client(config: Option<&AiConfig>) -> OpenAIClient {
    match config {
        None => OpenAIClient::builder()
            .with_api_key(env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set"))
            .build()
            .unwrap(),
        Some(AiConfig::OpenAI(model_config)) => {
            let api_key = if model_config.api_key.is_empty() {
                env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set")
            } else {
                model_config.api_key.clone()
            };
            OpenAIClient::builder()
                .with_api_key(api_key)
                .build()
                .unwrap()
        }
        Some(AiConfig::DeepSeek(ModelConfig { api_key, .. })) => OpenAIClient::builder()
            .with_endpoint(DEEPSEEK_API_ENDPOINT)
            .with_api_key(api_key)
            .build()
            .unwrap(),
        Some(AiConfig::Anthropic(ModelConfig { api_key, .. })) => OpenAIClient::builder()
            .with_endpoint(ANTHROPIC_API_ENDPOINT)
            .with_api_key(api_key)
            .build()
            .unwrap(),
    }
}

pub(crate) fn default_model(config: Option<&AiConfig>) -> String {
    match config {
        None => GPT3_5_TURBO.to_string(),
        Some(AiConfig::OpenAI(ModelConfig { model, .. })) => {
            model.clone().unwrap_or(GPT3_5_TURBO.to_string())
        }
        Some(AiConfig::DeepSeek(ModelConfig { model, .. })) => {
            model.clone().unwrap_or(DEEPSEEK_V3.to_string())
        }
        Some(AiConfig::Anthropic(ModelConfig { model, .. })) => {
            model.clone().unwrap_or(CLAUDE_3_5_SONNET.to_string())
        }
    }
}
