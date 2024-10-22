# Welcome to AiScript

AiScript is a high-level programming language let you build web applications intuitively, elegantly and blazing-fast.

## Features

- Simple and intuitive syntax
- High-level programming language
- High performance

## Why AiScript

In the past decades, web applications are built using server-side programming languages such as PHP, Java, Python, Ruby, etc. Each language has its web frameworks, ORMs, template engines, etc.
Programmers have to learn different languages and frameworks to build web applications. However, each web framework and ORM essentially does the same thing. They all parse HTTP requests, execute SQL queries, and response the result wit JSON or render HTML templates. There is no need to learn different languages and frameworks to build web applications. So we built AiScript to make web development easier.

For more questions, please refer to [FAQ](https://github.com/ais-one/ais/wiki/FAQ).

## How AiScript works

```
$ cat web.ai
get / {
    query {
        @length(min=3, max=10)
        name: str
    }

    return "Hello, :name!"
}

$ aim server web.ai
Listening on  http://localhost:8000

$ curl http://localhost:8000
{
    "error": "Missing query parameter: name"
}

$ curl http://localhost:8000?name=Li
{
    "error": "Invalid query parameter: name, must be between 3 and 10 characters"
}

$ curl http://localhost:8000?name=AiScript
Hello, AiScript!
```