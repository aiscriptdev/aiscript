> For AI observability, please refer to [observability](./observability.md).

```toml
[ai.embedding]
model = "gpt-3.5-turbo"

[ai.openai]
api_key = "YOUR_API_KEY"
completion_model = "gpt-3.5-turbo"
embedding_model = "text-embedding-ada-002"

[ai.anthropic]
api_key = "YOUR_API_KEY"
completion_model = "claude-2"
embedding_model = "claude-2"

[ai.cohere]
api_key = "YOUR_API_KEY"
completion_model = "command"
embedding_model = "embed-english-v2.0"

[ai.huggingface]
api_key = "YOUR_API_KEY"
completion_model = "gpt2"
embedding_model = "sentence-transformers/all-MiniLM-L6-v2"

```