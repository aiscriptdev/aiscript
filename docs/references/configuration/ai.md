> For AI observability, please refer to [observability](./observability.md).

```toml
[ai.embedding]
model = "gpt-3.5-turbo"

[ai.openai]
api_key = "YOUR_API_KEY"
completion_model = "gpt-3.5-turbo"

[ai.anthropic]
api_key = "YOUR_API_KEY"
completion_model = "claude-2"

[ai.cohere]
api_key = "YOUR_API_KEY"
completion_model = "command"

[ai.huggingface]
api_key = "YOUR_API_KEY"
completion_model = "gpt2"

```