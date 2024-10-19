Add new table `tweet`.

## Migrate

```
$ aim migrate add "tweet-table"
Creating migrations/20241001160410-tweet-table.up.sql
Creating migrations/20241001160410-tweet-table.down.sql
```

Edit `migrations/20241001160410-tweet-table.up.sql`

```sql
CREATE TABLE IF NOT EXISTS "tweet" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "user_id" INTEGER NOT NULL,
    "content" TEXT NOT NULL,
    "created_at" TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP DEFAULT CURRENT_TIMESTAMP
};
```

Run the migration:

```
$ aim migrate run
Applied migrations/20241001160410 tweet-table (2.547ms)
```

## Add model

```py
# models/tweet.ai

@table(name="tweet")
model Tweet {
    @primary
    id: int
    user_id: int
    content: str
    created_at: datetime
    updated_at: datetime
}

with Tweet {
    auto fn get_by_id(id: int) -> Tweet

    fn create(user_id: int, content: str) -> Tweet {
        return sql {
            INSERT INTO tweet (user_id, content) VALUES (:user_id, :content) RETURNING *
        }
    }
}
```

## Add route

```py
# routes/tweet.ai

post /tweet {
    @json
    body {
        @lenngth(min=1, max=140)
        content: str
    }

    let tweet = model.Tweet.create(user_id, content)
    return tweet
}

get /tweet/<id> {
    return model.Tweet.get_by_id(id)
}
```

## Test tweet api

```json
$ curl -X POST -H "Content-Type: application/json" -d '{"content": "Hello World"}' http://localhost:8000/tweet
{
    "id":1,
    "user_id":1,
    "content":"Hello World",
    "created_at":"2023-10-01T16:11:11.000000",
    "updated_at":"2023-10-01T16:11:11.000000"
}

$ curl http://localhost:8000/tweet/1
{
    "id":1,
    "user_id":1,
    "content":"Hello World",
    "created_at":"2023-10-01T16:11:11.000000",
    "updated_at":"2023-10-01T16:11:11.000000"
}
```