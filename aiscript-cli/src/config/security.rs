use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SecurityConfig {
    pub jwt: JwtConfig,
}

#[derive(Debug, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub expiration: u64, // expiration time in seconds
}
