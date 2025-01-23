use std::collections::HashMap;

use aiscript_directive::route::SsoProvider;
use serde::Deserialize;

use super::{Config, EnvString};

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
    pub scopes: Vec<String>,
}

#[inline]
fn sso_extra_fields(provider: SsoProvider) -> HashMap<&'static str, serde_json::Value> {
    match provider {
        SsoProvider::Facebook => [
            (
                "auth_url",
                "https://www.facebook.com/v9.0/dialog/oauth".into(),
            ),
            (
                "token_url",
                "https://graph.facebook.com/v19.0/oauth/access_token".into(),
            ),
            (
                "userinfo_url",
                "https://graph.facebook.com/v19.0/me?fields=id,name,email,first_name,last_name,picture".into(),
            ),
        ]
        .into_iter()
        .collect(),
        SsoProvider::Google => [
            (
                "auth_url",
                "https://accounts.google.com/o/oauth2/v2/auth".into(),
            ),
            (
                "token_url",
                "https://www.googleapis.com/oauth2/v3/token".into(),
            ),
            (
                "userinfo_url",
                "https://openidconnect.googleapis.com/v1/userinfo".into(),
            ),
        ]
        .into_iter()
        .collect(),
        SsoProvider::Discord => [
            ("auth_url", "https://discord.com/oauth2/authorize".into()),
            ("token_url", "https://discord.com/api/oauth2/token".into()),
            ("userinfo_url", "https://discord.com/api/users/@me".into()),
        ]
        .into_iter()
        .collect(),
        SsoProvider::GitHub => [
            (
                "auth_url",
                "https://github.com/login/oauth/authorize".into(),
            ),
            (
                "token_url",
                "https://github.com/login/oauth/access_token".into(),
            ),
            ("userinfo_url", "https://api.github.com/user".into()),
        ]
        .into_iter()
        .collect(),
    }
}

pub fn get_sso_fields(provider: SsoProvider) -> Option<HashMap<&'static str, serde_json::Value>> {
    let config = &Config::get().sso;
    let sso_provider = match provider {
        SsoProvider::Facebook => config.facebook.as_ref(),
        SsoProvider::Google => config.google.as_ref(),
        SsoProvider::Discord => config.discord.as_ref(),
        SsoProvider::GitHub => config.github.as_ref(),
    }?;

    let mut fields: HashMap<&'static str, serde_json::Value> = [
        (
            "client_id",
            serde_json::Value::String(sso_provider.client_id.to_string()),
        ),
        (
            "client_secret",
            serde_json::Value::String(sso_provider.client_secret.to_string()),
        ),
        (
            "redirect_url",
            serde_json::Value::String(sso_provider.redirect_url.to_string()),
        ),
        (
            "scopes",
            serde_json::Value::Array(
                sso_provider
                    .scopes
                    .iter()
                    .map(|v| serde_json::Value::String(v.to_string()))
                    .collect(),
            ),
        ),
    ]
    .into_iter()
    .collect();

    fields.extend(sso_extra_fields(provider));

    Some(fields)
}
