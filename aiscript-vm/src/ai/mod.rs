mod agent;
mod prompt;

use aiscript_common::EnvString;
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

// Ollama
const OLLAMA_DEFAULT_API_ENDPOINT: &str = "http://localhost:11434/v1";
const OLLAMA_DEFAULT_MODEL: &str = "llama3";

#[derive(Debug, Clone, Deserialize)]
pub struct AiConfig {
    pub openai: Option<ModelConfig>,
    pub anthropic: Option<ModelConfig>,
    pub deepseek: Option<ModelConfig>,
    pub ollama: Option<ModelConfig>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            openai: env::var("OPENAI_API_KEY").ok().map(|key| ModelConfig {
                api_key: key.into(),
                api_endpoint: Some(OPENAI_API_ENDPOINT.into()),
                model: Some(OPENAI_DEFAULT_MODEL.into()),
            }),
            anthropic: env::var("CLAUDE_API_KEY").ok().map(|key| ModelConfig {
                api_key: key.into(),
                api_endpoint: Some(ANTHROPIC_API_ENDPOINT.into()),
                model: Some(ANTHROPIC_DEFAULT_MODEL.into()),
            }),
            deepseek: env::var("DEEPKSEEK_API_KEY").ok().map(|key| ModelConfig {
                api_key: key.into(),
                api_endpoint: Some(DEEPSEEK_API_ENDPOINT.into()),
                model: Some(DEEPSEEK_DEFAULT_MODEL.into()),
            }),
            ollama: env::var("OLLAMA_API_ENDPOINT")
                .ok()
                .map(|endpoint| ModelConfig {
                    api_key: EnvString(String::default()), // Ollama does not require an API key
                    api_endpoint: endpoint
                        .parse()
                        .ok()
                        .map(|url: String| url.into())
                        .or(Some(OLLAMA_DEFAULT_API_ENDPOINT.into())),
                    model: Some(OLLAMA_DEFAULT_MODEL.into()),
                }),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    pub api_key: EnvString,
    pub api_endpoint: Option<EnvString>,
    pub model: Option<EnvString>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig {
            #[cfg(feature = "ai_test")]
            api_key: "".into(),
            #[cfg(not(feature = "ai_test"))]
            api_key: EnvString(env::var("OPENAI_API_KEY").unwrap_or_default()),
            api_endpoint: Some(OPENAI_API_ENDPOINT.into()),
            model: Some(OPENAI_DEFAULT_MODEL.into()),
        }
    }
}

impl AiConfig {
    pub(crate) fn get_model_config(
        &self,
        model_name: Option<String>,
    ) -> Result<ModelConfig, String> {
        if let Some(ollama) = self.ollama.as_ref() {
            let model = model_name.as_deref().unwrap_or(OLLAMA_DEFAULT_MODEL);
            let mut config = ollama.clone();
            config.model = Some(EnvString(model.to_string()));
            return Ok(config);
        }
        if let Some(model) = model_name {
            match model {
                m if m.starts_with("gpt") => {
                    if let Some(openai) = self.openai.as_ref() {
                        let mut config = openai.clone();
                        config.model = Some(EnvString(m));
                        Ok(config)
                    } else {
                        Ok(ModelConfig::default())
                    }
                }
                m if m.starts_with("claude") => {
                    if let Some(anthropic) = self.anthropic.as_ref() {
                        let mut config = anthropic.clone();
                        config.model = Some(EnvString(m));
                        Ok(config)
                    } else {
                        Ok(ModelConfig {
                            api_key: env::var("CLAUDE_API_KEY")
                                .expect("Expect `CLAUDE_API_KEY` environment variable.")
                                .into(),
                            api_endpoint: Some(ANTHROPIC_API_ENDPOINT.into()),
                            model: Some(ANTHROPIC_DEFAULT_MODEL.into()),
                        })
                    }
                }
                m if m.starts_with("deepseek") => {
                    if let Some(deepseek) = self.deepseek.as_ref() {
                        let mut config = deepseek.clone();
                        config.model = Some(EnvString(m));
                        Ok(config)
                    } else {
                        Ok(ModelConfig {
                            api_key: env::var("DEEPSEEK_API_KEY")
                                .expect("Expect `DEEPSEEK_API_KEY` environment variable.")
                                .into(),
                            api_endpoint: Some(DEEPSEEK_API_ENDPOINT.into()),
                            model: Some(DEEPSEEK_DEFAULT_MODEL.into()),
                        })
                    }
                }
                m => Err(format!("Unsupported model '{m}'.")),
            }
        } else if let Some(ollama) = self.ollama.as_ref() {
            if let Some(model) = model_name {
                let mut config = ollama.clone();
                config.model = Some(EnvString(model));
                return Ok(config);
            } else {
                return Ok(ollama.clone());
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
        .with_api_key(&*config.api_key)
        .with_endpoint(config.api_endpoint.as_deref().unwrap())
        .build()
        .unwrap()
}
