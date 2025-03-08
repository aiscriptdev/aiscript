use openai_api_rs::v1::common::GPT3_5_TURBO;
use tokio::runtime::Handle;

pub struct PromptConfig {
    pub input: String,
    pub model: Option<String>,
    pub max_tokens: Option<i64>,
    pub temperature: Option<f64>,
    pub system_prompt: Option<String>,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            input: String::new(),
            model: Some(GPT3_5_TURBO.to_string()),
            max_tokens: Default::default(),
            temperature: Default::default(),
            system_prompt: Default::default(),
        }
    }
}

#[cfg(feature = "ai_test")]
async fn _prompt_with_config(config: PromptConfig) -> String {
    return format!("AI: {}", config.input);
}

#[cfg(not(feature = "ai_test"))]
async fn _prompt_with_config(mut config: PromptConfig) -> String {
    use openai_api_rs::v1::{
        chat_completion::{self, ChatCompletionRequest},
        common::GPT3_5_TURBO,
    };

    let mut client = super::openai_client();

    // Create system message if provided
    let mut messages = Vec::new();
    if let Some(system_prompt) = config.system_prompt.take() {
        messages.push(chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(system_prompt),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Add user message
    messages.push(chat_completion::ChatCompletionMessage {
        role: chat_completion::MessageRole::user,
        content: chat_completion::Content::Text(config.input),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    });

    // Build the request
    let mut req = ChatCompletionRequest::new(
        config
            .model
            .take()
            .unwrap_or_else(|| GPT3_5_TURBO.to_string()),
        messages,
    );

    if let Some(max_tokens) = config.max_tokens {
        req.max_tokens = Some(max_tokens);
    }

    if let Some(temperature) = config.temperature {
        req.temperature = Some(temperature);
    }

    let result = client.chat_completion(req).await.unwrap();
    result.choices[0]
        .message
        .content
        .clone()
        .unwrap_or_default()
}

pub fn prompt_with_config(config: PromptConfig) -> String {
    if Handle::try_current().is_ok() {
        // We're in an async context, use await
        Handle::current().block_on(async { _prompt_with_config(config).await })
    } else {
        // We're in a sync context, create a new runtime
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async { _prompt_with_config(config).await })
    }
}
