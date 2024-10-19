## Setu up the project

> Before you start, make sure you have installed AiScript. If not, please refer to [Installation](../installation.md).

Let's create a new project.

```
$ aim new my-project
```

This will create a new project named `my-project` in the current directory.

```
$ cd my-project

$ tree
my-project
├── lib/
├── migrations/
├── models/
├── routes/
├── schemas/
└── project.toml
```

## Project structure

AiScript project is convention over configuration. The project directory structure is as follows:

### project.toml

The `project.toml` file is a **TOML** file to configure the project.

```toml
[project]
name = "my-project"
description = "My project"
version = "0.1.0"

[apidoc]
enabled = true
type = "swagger"
path = "/docs"

[network]
host = "0.0.0.0"
port = 8000
```

You can configure the project name, description, version, port, etc. For more information, please refer to [Configuration Reference](../references/configuration.md).

### routes

The routes directory contains the route files. The route files are used to define the routes of the project.

### models

The models directory contains the model files. Model is to define the data structure of the data mapping to the database table.

### schemas

The schemas directory contains the schema files. Schema is to define how you response to the request.

### lib

The lib directory contains the library files. You can define the library functions in the library files.

### migrations

The migrations directory contains the migration SQL files.

## Database

```toml
[database.sqlite]
file = "db.sqlite"
```

### Migrate tables

```
$ aim migrate add "user-table"
Creating migrations/20241001154420-user-table.up.sql
Creating migrations/20241001154420-user-table.down.sql
```

Edit `20241001154420-user-table.up.sql`:

```sql
CREATE TABLE IF NOT EXISTS "user" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "handle" TEXT UNIQUE NOT NULL,
    "nickname" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "password" TEXT NOT NULL,
    "gender" TEXT NOT NULL,
    "avatar" TEXT,
    "bio" TEXT,
    "follow_count" INTEGER NOT NULL DEFAULT 0,
    "follower_count" INTEGER NOT NULL DEFAULT 0,
    "tweet_count" INTEGER NOT NULL DEFAULT 0,
    "created_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

Run the migration:

```
$ aim migrate run
Applied migrations/20241001154420 user-table (3.517835ms)
```
