mod agent;
mod prompt;

use std::env;

pub use agent::{Agent, run_agent};
use openai_api_rs::v1::{api::OpenAIClient, common};
pub use prompt::{PromptConfig, prompt_with_config};

use serde::Deserialize;

// OpenAI
const OPENAI_API_ENDPOINT: &str = "https://api.openai.com/v1";
const OPENAI_DEFAULT_MODEL: &str = common::GPT4;

// Deepseek
const DEEPSEEK_API_ENDPOINT: &str = "https://api.deepseek.com/v1";
const DEEPSEEK_DEFAULT_MODEL: &str = "deepseek-chat";

// Anthropic
const ANTHROPIC_API_ENDPOINT: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_DEFAULT_MODEL: &str = "claude-3-5-sonnet-latest";

#[derive(Debug, Clone, Deserialize)]
pub struct AiConfig {
    pub openai: Option<ModelConfig>,
    pub anthropic: Option<ModelConfig>,
    pub deepseek: Option<ModelConfig>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            openai: env::var("OPENAI_API_KEY").ok().map(|key| ModelConfig {
                api_key: key,
                api_endpoint: Some(OPENAI_API_ENDPOINT.to_string()),
                model: Some(OPENAI_DEFAULT_MODEL.to_string()),
            }),
            anthropic: env::var("CLAUDE_API_KEY").ok().map(|key| ModelConfig {
                api_key: key,
                api_endpoint: Some(ANTHROPIC_API_ENDPOINT.to_string()),
                model: Some(ANTHROPIC_DEFAULT_MODEL.to_string()),
            }),
            deepseek: env::var("DEEPKSEEK_API_KEY").ok().map(|key| ModelConfig {
                api_key: key,
                api_endpoint: Some(DEEPSEEK_API_ENDPOINT.to_string()),
                model: Some(DEEPSEEK_DEFAULT_MODEL.to_string()),
            }),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    pub api_key: String,
    pub api_endpoint: Option<String>,
    pub model: Option<String>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig {
            api_key: env::var("OPENAI_API_KEY")
                .expect("Expect `OPENAI_API_KEY` environment variable."),
            api_endpoint: Some(OPENAI_API_ENDPOINT.to_string()),
            model: Some(OPENAI_DEFAULT_MODEL.to_string()),
        }
    }
}

impl AiConfig {
    pub(crate) fn get_model_config(
        &self,
        model_name: Option<String>,
    ) -> Result<ModelConfig, String> {
        if let Some(model) = model_name {
            match model {
                m if m.starts_with("gpt") => {
                    if let Some(openai) = self.openai.as_ref() {
                        let mut config = openai.clone();
                        config.model = Some(m);
                        Ok(config)
                    } else {
                        Ok(ModelConfig::default())
                    }
                }
                m if m.starts_with("claude") => {
                    if let Some(anthropic) = self.anthropic.as_ref() {
                        let mut config = anthropic.clone();
                        config.model = Some(m);
                        Ok(config)
                    } else {
                        Ok(ModelConfig {
                            api_key: env::var("CLAUDE_API_KEY")
                                .expect("Expect `CLAUDE_API_KEY` environment variable."),
                            api_endpoint: Some(ANTHROPIC_API_ENDPOINT.to_string()),
                            model: Some(ANTHROPIC_DEFAULT_MODEL.to_string()),
                        })
                    }
                }
                m if m.starts_with("deepseek") => {
                    if let Some(deepseek) = self.deepseek.as_ref() {
                        let mut config = deepseek.clone();
                        config.model = Some(m);
                        Ok(config)
                    } else {
                        Ok(ModelConfig {
                            api_key: env::var("DEEPSEEK_API_KEY")
                                .expect("Expect `DEEPSEEK_API_KEY` environment variable."),
                            api_endpoint: Some(DEEPSEEK_API_ENDPOINT.to_string()),
                            model: Some(DEEPSEEK_DEFAULT_MODEL.to_string()),
                        })
                    }
                }
                m => Err(format!("Unsupported model '{m}'.")),
            }
        } else {
            // Default is OpenAI model
            Ok(ModelConfig::default())
        }
    }
}

#[allow(unused)]
pub(crate) fn openai_client(config: &ModelConfig) -> OpenAIClient {
    OpenAIClient::builder()
        .with_api_key(&config.api_key)
        .with_endpoint(config.api_endpoint.as_ref().unwrap())
        .build()
        .unwrap()
}
