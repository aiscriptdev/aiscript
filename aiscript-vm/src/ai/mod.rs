mod agent;
mod prompt;

use std::env;

pub use agent::{run_agent, Agent};
use openai_api_rs::v1::api::OpenAIClient;
pub use prompt::prompt;

pub(crate) fn openai_client() -> OpenAIClient {
    OpenAIClient::builder()
        .with_api_key(env::var("OPENAI_API_KEY").unwrap().to_string())
        .build()
        .unwrap()
}
