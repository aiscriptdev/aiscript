
We should only return the fields we want to expose to the user.

## Create User schema

```py
schema UserInfo {
    @skip
    handle: str
    nickname: str
    gender: str
    avatar: str
    bio: str

    @computed
    fn handle() -> str {
        # Add @ prefix to the handle.
        return "@{handle}"
    }
}
```

## Set UserInfo schema

```
post /user -> schema.UserInfo {
}

put /user/<id> -> schema.UserInfo {
}

get /user/<id> -> schema.UserInfo {
}
```

## @skip and @computed

You can use `@skip` to skip the field from the response and `@computed` to dynamically compute the field value.

## Test our user API

```json
$ curl http://localhost:8000/user/1
{
    "handle": "@aiscript",
    "nickname": "AiScript",
    "gender": "male",
    "avatar": "",
    "bio": ""
}
```