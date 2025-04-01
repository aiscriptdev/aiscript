use std::{fs, path::Path, sync::OnceLock};

use auth::AuthConfig;
use serde::Deserialize;

use aiscript_vm::AiConfig;
use db::DatabaseConfig;
pub use sso::{SsoConfig, get_sso_fields};

mod auth;
mod db;
mod sso;
#[cfg(test)]
mod tests;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub apidoc: ApiDocConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub sso: SsoConfig,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ApiDocType {
    Swagger,
    #[default]
    Redoc,
}

#[derive(Debug, Deserialize)]
pub struct ApiDocConfig {
    pub enabled: bool,
    #[serde(rename = "type", default)]
    pub doc_type: ApiDocType,
    #[serde(default = "default_path")]
    pub path: String,
}

fn default_path() -> String {
    "/doc".to_string()
}

impl Default for ApiDocConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            doc_type: ApiDocType::default(),
            path: default_path(),
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

    pub fn load() -> &'static Config {
        CONFIG.get_or_init(|| {
            Config::new("project.toml").unwrap_or_else(|e| {
                eprintln!("Error loading config file: {}", e);
                Config::default()
            })
        })
    }

    pub fn get() -> &'static Config {
        CONFIG.get().expect("Config not initialized")
    }
}
