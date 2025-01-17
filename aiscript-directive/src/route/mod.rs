use serde_json::Value;

use crate::{Directive, FromDirective};

#[derive(Debug, Copy, Clone, Default)]
pub struct RouteAnnotation {
    pub auth: Auth,
    pub deprecated: bool,
    pub sso_provider: Option<SsoProvider>,
}

#[derive(Debug, Copy, Clone, Default)]
pub enum Auth {
    Jwt,
    Basic,
    #[default]
    None,
}

#[derive(Debug, Copy, Clone)]
pub enum SsoProvider {
    Facebook,
    Google,
    Discord,
    GitHub,
}

impl SsoProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            SsoProvider::Facebook => "facebook",
            SsoProvider::Google => "google",
            SsoProvider::Discord => "discord",
            SsoProvider::GitHub => "github",
        }
    }
}

impl TryFrom<&String> for SsoProvider {
    type Error = String;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.as_ref() {
            "facebook" => Ok(Self::Facebook),
            "google" => Ok(Self::Google),
            "discord" => Ok(Self::Discord),
            "github" => Ok(Self::GitHub),
            _ => Err("Invalid SSO provider".into()),
        }
    }
}

impl RouteAnnotation {
    pub fn is_auth_required(&self) -> bool {
        match self.auth {
            Auth::Jwt | Auth::Basic => true,
            Auth::None => false,
        }
    }

    pub fn is_jwt_auth(&self) -> bool {
        matches!(self.auth, Auth::Jwt)
    }

    pub fn or(mut self, other: RouteAnnotation) -> Self {
        if matches!(self.auth, Auth::None) {
            self.auth = other.auth;
        }
        if !self.deprecated {
            self.deprecated = other.deprecated
        }
        self
    }
}

impl FromDirective for Auth {
    fn from_directive(directive: Directive) -> Result<Self, String> {
        match directive.name.as_str() {
            "auth" => Ok(Auth::Jwt),
            "basic_auth" => Ok(Auth::Basic),
            _ => Ok(Auth::None),
        }
    }
}

impl RouteAnnotation {
    pub fn parse_directive(&mut self, directive: Directive) -> Result<(), String> {
        match directive.name.as_str() {
            "auth" | "basic_auth" => {
                if matches!(self.auth, Auth::None) {
                    self.auth = Auth::from_directive(directive)?;
                } else {
                    return Err("Duplicate auth directive".into());
                }
            }
            "deprecated" => {
                if self.deprecated {
                    return Err("Duplicate deprecated directive".into());
                } else {
                    self.deprecated = true;
                }
            }
            "sso" => {
                if let Some(Value::String(provider)) = directive.get_arg_value("provider") {
                    self.sso_provider = Some(SsoProvider::try_from(provider)?);
                } else {
                    return Err("@sso required 'provider' argument.".into());
                }
            }
            _ => {
                return Err(format!("Invalid directive: @{}", directive.name));
            }
        }
        Ok(())
    }
}
