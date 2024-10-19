
Middleware is a function that has access to the request object (req), the response object (res), and the next middleware function in the application’s request-response cycle. If you familiar web frameworks in other languages, AiScript's middleware is a similar thing.

## Use builtin middleware

AiScript provides some builtin middleware, you can use them by adding them to the `middleware` section in the `project.toml` file.

```toml
# project.toml

[middleware.cors]
allowed_origins = ["http://localhost:3000"]
allowed_methods = ["GET", "POST", "PUT", "DELETE"]
allowed_headers = ["Content-Type", "Authorization"]
allow_credentials = true
```

The `cors` middleware is used to enable CORS. You can also use other middleware, such as `body_limit`, `rate_limit`, `timeout`, etc. You can refer to [Middleware Configuration](../references/configuration/middleware.md) for more details.

## Custom middleware

To be continued...