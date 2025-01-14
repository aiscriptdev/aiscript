use serde::Deserialize;

use super::EnvString;

#[derive(Debug, Deserialize, Default)]
pub struct SsoConfig {
    pub google: Option<OAuthProviderConfig>,
    pub github: Option<OAuthProviderConfig>,
    pub discord: Option<OAuthProviderConfig>,
    pub facebook: Option<OAuthProviderConfig>,
}

#[derive(Debug, Deserialize)]
pub struct OAuthProviderConfig {
    pub client_id: EnvString,
    pub client_secret: EnvString,
    pub redirect_url: String,
    pub scope: Vec<String>,
}
