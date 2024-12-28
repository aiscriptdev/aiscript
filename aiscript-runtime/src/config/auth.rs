use serde::Deserialize;

use super::EnvString;

#[derive(Debug, Deserialize, Default)]
pub struct AuthConfig {
    pub jwt: JwtConfig,
    pub basic: Option<BasicAuthConfig>,
}

#[derive(Debug, Deserialize)]
pub struct JwtConfig {
    pub secret: EnvString,
    pub expiration: u64, // expiration time in seconds
}

#[derive(Debug, Deserialize)]
pub struct BasicAuthConfig {
    pub username: EnvString,
    pub password: EnvString,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: EnvString("secret".into()),
            expiration: 24 * 3600,
        }
    }
}
