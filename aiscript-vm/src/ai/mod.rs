mod agent;
mod prompt;

use std::env;

pub use agent::{Agent, run_agent};
use openai_api_rs::v1::api::OpenAIClient;
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

#[allow(unused)]
pub(crate) fn openai_client() -> OpenAIClient {
    OpenAIClient::builder()
        .with_api_key(env::var("OPENAI_API_KEY").unwrap().to_string())
        .build()
        .unwrap()
}
