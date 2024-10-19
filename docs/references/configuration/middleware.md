
```toml
[middleware.cors]
allowed_origins = ["http://localhost:3000"]
allowed_methods = ["GET", "POST", "PUT", "DELETE"]
allowed_headers = ["Content-Type", "Authorization"]
allow_credentials = true

[middleware.body_limit]
limit = "100KB"

[middleware.rate_limit]
enabled = true
max_requests = 100
window = 60

[middleware.timeout]
timeout = 5

```

## CORS

## Body limit

## Rate Limit

## Timeout
