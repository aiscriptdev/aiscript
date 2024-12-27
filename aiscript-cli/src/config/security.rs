use serde::Deserialize;

use super::EnvString;

#[derive(Debug, Deserialize)]
pub struct SecurityConfig {
    pub jwt: JwtConfig,
}

#[derive(Debug, Deserialize)]
pub struct JwtConfig {
    pub secret: EnvString,
    pub expiration: u64, // expiration time in seconds
}
