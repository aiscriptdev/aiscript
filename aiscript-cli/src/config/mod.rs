#![allow(unused)]
use std::{env, fmt::Display, fs, path::Path, sync::OnceLock};

use security::SecurityConfig;
use serde::Deserialize;

use db::DatabaseConfig;

mod db;
mod security;

static CONFIG: OnceLock<Config> = OnceLock::new();

// Custom string type that handles environment variable substitution
#[derive(Debug, Clone, Deserialize)]
#[serde(from = "String")]
pub struct EnvString(String);

impl From<String> for EnvString {
    fn from(s: String) -> Self {
        if let Some(env_key) = s.strip_prefix('$') {
            match env::var(env_key) {
                Ok(val) => EnvString(val),
                Err(_) => {
                    // If env var is not found, use the original string
                    // This allows for better error handling at runtime
                    EnvString(s)
                }
            }
        } else {
            EnvString(s)
        }
    }
}

impl Display for EnvString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<EnvString> for String {
    fn from(s: EnvString) -> Self {
        s.0
    }
}

impl AsRef<str> for EnvString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub database: DatabaseConfig,
    pub apidoc: Option<ApiDocConfig>,
    pub security: Option<SecurityConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiDocType {
    Swagger,
    Redoc,
}

#[derive(Debug, Deserialize)]
pub struct ApiDocConfig {
    pub enabled: bool,
    #[serde(rename = "type")]
    pub doc_type: ApiDocType,
    pub path: String,
}

impl Default for ApiDocConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            doc_type: ApiDocType::Redoc,
            path: "/reddoc".to_string(),
        }
    }
}

impl Config {
    fn new(source: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = source.as_ref();
        if path.exists() {
            let content = fs::read_to_string(path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Config::default())
        }
    }

    pub fn load(path: &str) -> &Config {
        CONFIG.get_or_init(|| {
            Config::new(path).unwrap_or_else(|e| {
                eprintln!("Error loading config file: {}", e);
                Config::default()
            })
        })
    }
}
