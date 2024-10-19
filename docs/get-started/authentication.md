## Configure JWT Authentication

```toml
# project.toml

[security.jwt]
secret = "secret"
expiration = 3600
header = "Authorization"
```

## Response JWT

```py
# routes/user.ai

post /user {
    @json
    body {
        @length(min=1, max=16)
        handle: str
        @length(min=3, max=20)
        nickname: str
        @format(type="email")
        email: str
    }

    if not model.User.check_unique_handle(handle.lower()) {
        return "handle already exists", 400
    }

    let user = model.User.create(handle.lower(), nickname, email)
    let token = std.security.encode_jwt(user.id)
    # set the JWT token in the response header
    header.Authorization = token
    cookie.accessToken = token
    return user
}
```

## Add @auth directive to tweet API

```py

@auth
post /tweet (user_id: int) {
    @json
    body {
        @lenngth(min=1, max=140)
        content: str
    }

    let tweet = model.Tweet.create(user_id, content)
    return tweet
}
```

## Test tweet api

```json
$ curl -X POST -H "Content-Type: application/json" \
  -d '{"content": "Hello World"}' \
  http://localhost:8000/tweet
{
    "error": "Unauthorized",
}

$ curl -v -X POST -H "Content-Type: application/json" \
  -d '{"handle": "alice", "nickname": "Alice", "email": "alice@aiscript.dev", "password": "alice123"}' \
  http://localhost:8000/user
> User-Agent: curl/8.7.1
> Authorization: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpZCI6MSwiaWF0IjoxNjg2MzYzMzYzLCJleHAiOjE2ODYzNjY5NjN9.q40QfXfXZ
{
    "id": 2,
    "handle": "alice",
    "nickname": "Alice",
    "email": "alice@aiscript.dev",
    "gender": "male",
    "avatar": null
}

$ curl -X POST -H "Content-Type: application/json" \
  -d '{"content": "Hello World"}' \
  -H "Authorization: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpZCI6MSwiaWF0IjoxNjg2MzYzMzYzLCJleHAiOjE2ODYzNjY5NjN9.q40QfXfXZ" \
  http://localhost:8000/tweet
{
    "id": 1,
    "user_id": 1,
    "content": "Hello World",
    "created_at": "2023-06-07T15:16:03.951651+00:00",
    "updated_at": "2023-06-07T15:16:03.951651+00:00"
}
```
