use tokio::runtime::Handle;

use crate::string::InternedString;

#[cfg(feature = "ai_test")]
async fn _prompt(message: String, _model: Option<InternedString<'_>>) -> String {
    return format!("AI: {message}");
}

#[cfg(not(feature = "ai_test"))]
async fn _prompt(message: String, _model: Option<InternedString<'_>>) -> String {
    use openai_api_rs::v1::{
        chat_completion::{self, ChatCompletionRequest},
        common::GPT3_5_TURBO,
    };
    let client = super::openai_client();
    let req = ChatCompletionRequest::new(
        GPT3_5_TURBO.to_string(),
        vec![chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(message),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }],
    );
    let result = client.chat_completion(req).await.unwrap();
    result.choices[0].message.content.clone().unwrap()
}

pub fn prompt(message: String, model: Option<InternedString>) -> String {
    if Handle::try_current().is_ok() {
        // We're in an async context, use await
        Handle::current().block_on(async { _prompt(message, model).await })
    } else {
        // We're in a sync context, create a new runtime
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async { _prompt(message, model).await })
    }
}
