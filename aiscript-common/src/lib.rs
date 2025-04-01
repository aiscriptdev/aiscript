use std::{env, fmt::Display, ops::Deref};

use serde::Deserialize;

// Custom string type that handles environment variable substitution
#[derive(Debug, Clone, Deserialize)]
#[serde(from = "String")]
pub struct EnvString(pub String);

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
impl<'a> From<&'a str> for EnvString {
    fn from(s: &'a str) -> Self {
        if let Some(env_key) = s.strip_prefix('$') {
            match env::var(env_key) {
                Ok(val) => EnvString(val),
                Err(_) => {
                    // If env var is not found, use the original string
                    // This allows for better error handling at runtime
                    EnvString(s.to_owned())
                }
            }
        } else {
            EnvString(s.to_owned())
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

impl Deref for EnvString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for EnvString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[test]
fn test_envstring_regular_value() {
    let regular = EnvString::from("regular_value".to_string());
    assert_eq!(regular.as_ref(), "regular_value");
}

#[test]
fn test_envstring_env_var_exists() {
    unsafe {
        env::set_var("TEST_ENV_VAR", "value_from_env");
        let env_string = EnvString::from("$TEST_ENV_VAR".to_string());
        assert_eq!(env_string.as_ref(), "value_from_env");
        env::remove_var("TEST_ENV_VAR");
    };
}

#[test]
fn test_envstring_env_var_not_exists() {
    unsafe { env::remove_var("NONEXISTENT_VAR") };
    let env_string = EnvString::from("$NONEXISTENT_VAR".to_string());
    assert_eq!(env_string.as_ref(), "$NONEXISTENT_VAR");
}

#[test]
fn test_envstring_dollar_in_middle() {
    let regular = EnvString::from("some$value".to_string());
    assert_eq!(regular.as_ref(), "some$value");
}
