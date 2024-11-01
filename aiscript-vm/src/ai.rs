pub fn prompt(message: String) -> String {
    // let claude = Claude::default()
    //     .with_api_key(std::env::var("CLAUDE_API_KEY").unwrap())
    //     .with_model("claude-3-5-sonnet-20241022");
    // return claude.invoke(message).await.unwrap();
    #[cfg(feature = "ai_test")]
    return format!("AI: {message}");
    #[cfg(not(feature = "ai_test"))]
    {
        use langchain_rust::{language_models::llm::LLM, llm::OpenAI};
        use tokio::runtime::Handle;

        async fn _prompt(message: String) -> String {
            let open_ai = OpenAI::default().with_model("gpt-3.5-turbo");
            open_ai.invoke(&message).await.unwrap()
        }

        if Handle::try_current().is_ok() {
            // We're in an async context, use await
            Handle::current().block_on(async { _prompt(message).await })
        } else {
            // We're in a sync context, create a new runtime
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async { _prompt(message).await })
        }
    }
}
