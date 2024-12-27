#![allow(unused)]
use std::{fs, path::Path, sync::OnceLock};

use security::SecurityConfig;
use serde::Deserialize;

use db::DatabaseConfig;

mod db;
mod security;

static CONFIG: OnceLock<Config> = OnceLock::new();

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
