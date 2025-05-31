<p align="center">
  <img width="500" src="https://aiscript.dev/aiscript-logo.svg" alt="AIScript Logo">
</p>

<h1 align="center">AIScript</h1>
<p align="center"><em>The next generation language for human and AI.</em></p>
<p align="center"><a href="https://aiscript.dev">https://aiscript.dev</a></p>

<p align="center">
  <a href="https://github.com/aiscript-dev/aiscript/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License">
  </a>
  <!-- <a href="https://github.com/aiscript-dev/aiscript/actions">
    <img src="https://github.com/aiscript-dev/aiscript/actions/workflows/ci.yml/badge.svg" alt="Build Status">
  </a> -->
  <a href="https://discord.gg/bXRqsweNPb">
    <img src="https://img.shields.io/discord/711895914494558250?label=chat&logo=discord" alt="Discord">
  </a>
</p>

## Introduction

AIScript is a unique combination of interpreter programming language and web framework, both written in Rust, designed to help developers build AI applications effortlessly. The language syntax draws inspiration from Python, JavaScript, and Rust, combining their strengths to create something that's intuitive, powerful, and easy to use.

> [!WARNING]
> `AIScript` is in early development. Please do not use it in production yet.

## Programming Language

As a programming language, AIScript is built with a custom interpreter from the ground up:

- Functions are first-class citizens with support for object-oriented programming paradigms
- Built-in AI primitives, including prompt, AI functions, and agent capabilities
- Dynamic typing system with targeted static type checking for improved safety
- Built-in data validation similar to Python's [Pydantic](https://docs.pydantic.dev/latest/)
- Simple yet powerful error handling, inspired by Rust, Go, and Zig
- Rich standard library leveraging Rust's ecosystem underneath
- Automatic garbage collection for memory management

## Web Framework

AIScript isn't just a language—it's a complete web development solution:

- Elegant and intuitive route DSL for defining endpoints
- Automatic parameter validation with clear error messages
- Automatic OpenAPI schema and documentation generation
- Built on Rust's best practices, using [axum](https://github.com/tokio-rs/axum) and [sqlx](https://github.com/launchbadge/sqlx) under the hood
- Combines Rust's axum performance with Python's Flask-like simplicity
- Built-in database modules ([`std.db.pg`](https://aiscript.dev/std/db/pg) and [`std.db.redis`](https://aiscript.dev/std/db/redis))
- Built-in authentication and social login capabilities
- Battery-included features easily enabled through simple configuration

## How AIScript works

```javascript
$ export OPENAI_API_KEY=<your-openai-api-key>

$ cat web.ai
get / {
    """An api to ask LLM"""

    query {
        """the question to ask"""
        @string(min_len=3, max_len=100) // validate params with builtin directive @string
        question: str
    }

    // `ai` and `prompt` are keywords
    ai fn ask(question: str) -> str {
        let answer = prompt question;
        return answer;
    }

    // use query.name or query["name"] to access query parameter
    let answer = ask(query.question);
    return { answer };
}

$ aiscript serve web.ai
Listening on http://localhost:8080

$ curl http://localhost:8080
{
    "error": "Missing required field: question"
}

$ curl http://localhost:8080?question=Hi
{
    "error": "Field validation failed: question: String length is less than the minimum length of 3"
}

$ curl http://localhost:8080?question=What is the capital of France?
{
    "answer": "The capital of France is Paris."
}
```

You can open [http://localhost:8080/redoc](http://localhost:8080/redoc) to see the automatically generated API documentation.

![](https://aiscript.dev/guide/open-api.png)

## Use Cases

AIScript excels in these scenarios:

- **AI-powered APIs**: When you need to build APIs that leverage LLMs and other AI services
- **Prototyping**: Rapidly build and test ideas without configuration overhead
- **Microservices**: Create lightweight, focused services with minimal boilerplate
- **Data validation**: When request/response validation is critical to your application
- **Internal tools**: Build tools quickly with integrated documentation

## Examples

Check out the [examples](./examples) directory for more sample code.

## Supported AI Models

AIScript supports the following AI models:

- [x] OpenAI (uses `OPENAI_API_KEY` environment variable by default or local models with Ollama)
- [x] DeepSeek
- [x] Anthropic
- [x] Ollama (100+ local models from various providers)

Configuration by `project.toml`:

```toml
# use OpenAI
[ai.openai]
api_key = "YOUR_API_KEY"
model = "gpt-4"

# or use DeepSeek
[ai.deepseek]
api_key = "YOUR_API_KEY"
model = "deepseek-chat"

# or use Anthropic
[ai.anthropic]
api_key = "YOUR_API_KEY"
model = "claude-3-5-sonnet-latest"

# or use Ollama (local models)
[ai.ollama]
api_endpoint = "http://localhost:11434/v1"  # Default Ollama endpoint
model = "llama3.2"  # or any other model installed in your Ollama instance
```

### Using Ollama

[Ollama](https://ollama.ai/) allows you to run local AI models on your own hardware. To use Ollama with AIScript:

1. Install Ollama from [ollama.ai](https://ollama.ai/)
2. Pull your desired models (e.g., `ollama pull llama3.2`)
3. Make sure Ollama is running locally
4. Configure AIScript to use Ollama as shown above or by setting the `OLLAMA_API_ENDPOINT` environment variable.

Ollama provides access to 100+ models ranging from small 135M parameter models to massive 671B parameter models, including:
- Llama family (llama4, llama3.2, codellama)
- DeepSeek models (deepseek-r1, deepseek-v3)
- And [many more specialized models](https://ollama.com/search)

## Roadmap

See our [roadmap](https://aiscript.dev/guide/contribution/roadmap) for upcoming features and improvements.

## Contributing

We welcome contributions to AIScript! Please see our [Contribution Guide](https://aiscript.dev/guide/contribution/interpreter-architecture) for more information.

## License

AIScript is released under the MIT License. See [LICENSE](LICENSE) for details.
