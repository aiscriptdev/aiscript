mod agent;
mod prompt;

use std::env;

pub use agent::{Agent, run_agent};
use openai_api_rs::v1::api::OpenAIClient;
pub use prompt::{PromptConfig, prompt_with_config};

#[allow(unused)]
pub(crate) fn openai_client() -> OpenAIClient {
    OpenAIClient::builder()
        .with_api_key(env::var("OPENAI_API_KEY").unwrap().to_string())
        .build()
        .unwrap()
}
