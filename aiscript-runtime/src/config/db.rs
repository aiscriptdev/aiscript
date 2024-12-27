use serde::Deserialize;

use super::EnvString;

#[derive(Debug, Deserialize, Default)]
pub struct DatabaseConfig {
    pub sqlite: Option<SqliteConfig>,
    pub postgresql: Option<PostgresConfig>,
    pub mysql: Option<MySqlConfig>,
    pub redis: Option<RedisConfig>,
}

#[derive(Debug, Deserialize)]
pub struct SqliteConfig {
    pub url: Option<EnvString>,
    pub database: Option<EnvString>,
}

#[derive(Debug, Deserialize)]
pub struct PostgresConfig {
    pub url: Option<EnvString>,
    pub host: Option<EnvString>,
    pub port: Option<u16>,
    pub user: Option<EnvString>,
    pub password: Option<EnvString>,
    pub database: Option<EnvString>,
}

#[derive(Debug, Deserialize)]
pub struct MySqlConfig {
    pub url: Option<EnvString>,
    pub host: Option<EnvString>,
    pub port: Option<u16>,
    pub user: Option<EnvString>,
    pub password: Option<EnvString>,
    pub database: Option<EnvString>,
}

#[derive(Debug, Deserialize)]
pub struct RedisConfig {
    pub url: Option<EnvString>,
    pub host: Option<EnvString>,
    pub port: Option<u16>,
    pub password: Option<EnvString>,
}

impl DatabaseConfig {
    pub fn get_sqlite_url(&self) -> Option<String> {
        self.sqlite.as_ref().and_then(|c| {
            c.url
                .clone()
                .map(Into::into)
                .or_else(|| c.database.as_ref().map(|db| format!("sqlite://{}", db)))
        })
    }

    pub fn get_postgres_url(&self) -> Option<String> {
        self.postgresql.as_ref().and_then(|c| {
            c.url.clone().map(Into::into).or_else(|| {
                match (&c.host, &c.port, &c.user, &c.password, &c.database) {
                    (Some(host), Some(port), Some(user), Some(password), Some(database)) => {
                        Some(format!(
                            "postgres://{}:{}@{}:{}/{}",
                            user, password, host, port, database
                        ))
                    }
                    _ => None,
                }
            })
        })
    }

    pub fn get_mysql_url(&self) -> Option<String> {
        self.mysql.as_ref().and_then(|c| {
            c.url.clone().map(Into::into).or_else(|| {
                match (&c.host, &c.port, &c.user, &c.password, &c.database) {
                    (Some(host), Some(port), Some(user), Some(password), Some(database)) => {
                        Some(format!(
                            "mysql://{}:{}@{}:{}/{}",
                            user, password, host, port, database
                        ))
                    }
                    _ => None,
                }
            })
        })
    }

    pub fn get_redis_url(&self) -> Option<String> {
        self.redis.as_ref().and_then(|c| {
            c.url
                .clone()
                .map(Into::into)
                .or_else(|| match (&c.host, &c.port, &c.password) {
                    (Some(host), Some(port), Some(password)) => {
                        Some(format!("redis://:{}@{}:{}", password, host, port))
                    }
                    _ => None,
                })
        })
    }
}
