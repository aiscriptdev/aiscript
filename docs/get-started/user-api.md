Create `user.ai` file under **routes** directory.

```
$ tree routes
routes
└── user.ai
```

## Create user API

```py
# routes/user.ai

post /user {
    @json
    body {
        @length(min=1, max=16)
        handle: str
        @length(min=3, max=20)
        nickname: str
        @format("email")
        email: str
    }

    if not model.User.check_unique_handle(handle.lower()) {
        return "handle already exists", 400
    }

    let user = model.User.create(handle.lower(), nickname, email)
    return user
}
```

## Update user info API

```py
# routes/user.ai

put /user/<id: int> {
    @json
    body {
        nickname: str
        avatar: str
        bio: str
    }

    let user = model.User.update(nickname, avatar, bio)
    return user
}
```

## Get user info API

```py
# routes/user.ai

get /user/<id: int> {
    return model.User.get_by_id(id)
}
```

## Start server

```
$ aim server
Listening on  http://localhost:8000
```

## Debug your API

### With curl

```json
$ curl -X POST -H "Content-Type: application/json" \
  -d '{"handle": "aiscript", "nickname": "AiScript", "email": "hi@aiscript.dev", "password": "aiscript"}' \
  http://localhost:8000/user
{
    "id":1,
    "handle": "aiscript",
    "nickname": "AiScript",
    "email": "hi@aiscript.dev",
    "gender": "male",
    "avatar": "",
    "bio": "",
    "follow_count": 0,
    "follower_count": 0,
    "tweet_count": 0,
    "created_at": "2024-01-01T00:00:00+00:00",
    "updated_at": "2020-01-01T00:00:00+00:00"
}
```

### With aim debug command

```
$ aim debug
Listening on  http://localhost:8001
```

Open `http://localhost:8001` in your browser.

You will found we response the `password` field, which is bad. We can use `Schema` to fix this.

