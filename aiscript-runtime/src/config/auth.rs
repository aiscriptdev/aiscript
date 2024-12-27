use serde::Deserialize;

use super::EnvString;

#[derive(Debug, Deserialize, Default)]
pub struct AuthConfig {
    pub jwt: JwtConfig,
}

#[derive(Debug, Deserialize)]
pub struct JwtConfig {
    pub secret: EnvString,
    pub expiration: u64, // expiration time in seconds
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: EnvString("secret".into()),
            expiration: 24 * 3600,
        }
    }
}
