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
        use tokio::sync::oneshot;
        // Create a channel to receive the result
        let (tx, rx) = oneshot::channel();

        // Spawn a new task to handle the async work
        tokio::spawn(async move {
            let open_ai = OpenAI::default().with_model("gpt-3.5-turbo");
            let result = open_ai.invoke(&message).await.unwrap();
            let _ = tx.send(result); // Ignore send errors since rx is definitely alive
        });

        // Wait for the result synchronously
        rx.blocking_recv()
            .unwrap_or_else(|_| "Error getting data".to_string())
    }
}
