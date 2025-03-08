use crate::{Config, config::EnvString};
use std::env;

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

#[test]
fn test_config_with_env_vars() {
    unsafe {
        env::set_var("TEST_JWT_SECRET", "secret123");

        let config_str = r#"
            [auth.jwt]
            secret = "$TEST_JWT_SECRET"
            expiration = 3600
        "#;

        let config: Config = toml::from_str(config_str).unwrap();
        assert_eq!(config.auth.jwt.secret.as_ref(), "secret123");
        env::remove_var("TEST_JWT_SECRET");
    };
}

#[test]
fn test_database_config_with_env_vars() {
    unsafe {
        env::set_var("DB_PASSWORD", "pass123");
        env::set_var("DB_URL", "postgres://localhost:5432/db");

        let config_str = r#"
            [auth.jwt]
            secret = "secret"
            expiration = 3600

            [database.postgresql]
            url = "$DB_URL"
            password = "$DB_PASSWORD"
        "#;

        let config: Config = toml::from_str(config_str).unwrap();

        assert_eq!(
            config
                .database
                .postgresql
                .as_ref()
                .unwrap()
                .url
                .as_ref()
                .unwrap()
                .as_ref(),
            "postgres://localhost:5432/db"
        );
        assert_eq!(
            config
                .database
                .postgresql
                .as_ref()
                .unwrap()
                .password
                .as_ref()
                .unwrap()
                .as_ref(),
            "pass123"
        );

        env::remove_var("DB_PASSWORD");
        env::remove_var("DB_URL");
    };
}
