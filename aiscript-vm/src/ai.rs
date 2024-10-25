use langchain_rust::{language_models::llm::LLM, llm::OpenAI};

pub async fn prompt(message: &str) -> String {
    // let claude = Claude::default()
    //     .with_api_key(std::env::var("CLAUDE_API_KEY").unwrap())
    //     .with_model("claude-3-5-sonnet-20241022");
    // return claude.invoke(message).await.unwrap();

    let open_ai = OpenAI::default().with_model("gpt-3.5-turbo");
    return open_ai.invoke(message).await.unwrap();
}
