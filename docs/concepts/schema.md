
## Define schema

Schema is defined with `schema` keyword, fields are defined with `name: type` syntax.

```py
# schemas/base.ai

schema BasicUser {
    id: int
    handle: str
    nickname: str
}

```

## Extend schema

You can extend schema by "inheriting" other schemas. AiScript don't support extend from multiple schemas.

```py
## schemas/user.ai

schema UserInfo(schema.BasicUser) {
    gender: str
    avatar: str
    bio: str
}
```

Since `UserInfo` extends `BasicUser`, it will inherit all fields from `BasicUser`, and you can override fields in `UserInfo`.


## Directives

AiScript has some directives to control the behavior of the schema.

```py
## schemas/user.ai

schema UserInfo(schema.BasicUser) {
    @skip
    handle: str
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

For example, `@skip` directive will skip the field from the response. You can also use `@computed` to dynamically compute the field value.

## Return schema in model

See [model](./model.md) for more details.

## Return schema in route

```py
# routes/user.ai

get /user/<id: int> -> schema.UserInfo {
    return model.User.get_by_id(id)
}
```