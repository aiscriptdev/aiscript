use regex::Regex;
use serde_json::Value;
use std::any::Any;

use super::Validator;
use crate::{Directive, DirectiveParams, FromDirective};

pub struct RegexValidator {
    pattern: Regex,
    raw_pattern: String,
}

impl Validator for RegexValidator {
    fn name(&self) -> &'static str {
        "@regex"
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        let value_str = match value.as_str() {
            Some(s) => s,
            None => return Err("Value must be a string".into()),
        };

        if self.pattern.is_match(value_str) {
            Ok(())
        } else {
            Err(format!(
                "Value does not match the regex pattern: {}",
                self.raw_pattern
            ))
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl FromDirective for RegexValidator {
    fn from_directive(directive: Directive) -> Result<Self, String> {
        // Only support KeyValue format with "pattern" parameter
        match &directive.params {
            DirectiveParams::KeyValue(params) => {
                // Get the pattern parameter
                let pattern_str = params
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "@regex directive requires a 'pattern' parameter".to_string())?;

                // Compile the regex
                let regex = match Regex::new(pattern_str) {
                    Ok(re) => re,
                    Err(e) => return Err(format!("Invalid regex pattern: {}", e)),
                };

                Ok(Self {
                    pattern: regex,
                    raw_pattern: pattern_str.to_string(),
                })
            }
            _ => {
                Err("Invalid format for @regex directive. Use @regex(pattern=\"...\")".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Directive, DirectiveParams};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_directive(pattern: &str) -> Directive {
        let mut params = HashMap::new();
        params.insert("pattern".to_string(), json!(pattern));

        Directive {
            name: "regex".into(),
            params: DirectiveParams::KeyValue(params),
            line: 1,
        }
    }

    #[test]
    fn test_regex_validator_basic() {
        let directive = create_directive("^[a-z]+$");
        let validator = RegexValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("abc")).is_ok());
        assert!(validator.validate(&json!("123")).is_err());
        assert!(validator.validate(&json!("abc123")).is_err());
        assert!(validator.validate(&json!("ABC")).is_err());
    }

    #[test]
    fn test_regex_validator_for_ssn() {
        let directive = create_directive("^\\d{3}-\\d{2}-\\d{4}$");
        let validator = RegexValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("123-45-6789")).is_ok());
        assert!(validator.validate(&json!("abc-12-3456")).is_err());
        assert!(validator.validate(&json!("12-34-5678")).is_err());
        assert!(validator.validate(&json!("1234-56-7890")).is_err());
    }

    #[test]
    fn test_regex_validator_with_non_string_value() {
        let directive = create_directive("^[a-z]+$");
        let validator = RegexValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!(123)).is_err());
        assert!(validator.validate(&json!(true)).is_err());
        assert!(validator.validate(&json!(null)).is_err());
    }

    #[test]
    fn test_invalid_regex_pattern() {
        let directive = create_directive("*invalid*");
        assert!(RegexValidator::from_directive(directive).is_err());
    }

    #[test]
    fn test_missing_pattern() {
        // Empty HashMap for parameters
        let params = HashMap::new();
        let directive = Directive {
            name: "regex".into(),
            params: DirectiveParams::KeyValue(params),
            line: 1,
        };
        assert!(RegexValidator::from_directive(directive).is_err());
    }

    #[test]
    fn test_incorrect_params_type() {
        // Test with Array instead of KeyValue params
        let directive = Directive {
            name: "regex".into(),
            params: DirectiveParams::Array(vec![json!("^[a-z]+$")]),
            line: 1,
        };
        assert!(RegexValidator::from_directive(directive).is_err());
    }

    #[test]
    fn test_complex_regex() {
        let directive = create_directive("^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$");
        let validator = RegexValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("user@example.com")).is_ok());
        assert!(
            validator
                .validate(&json!("user.name+tag@example.co.uk"))
                .is_ok()
        );
        assert!(validator.validate(&json!("invalid-email")).is_err());
        assert!(validator.validate(&json!("missing@domain")).is_err());
    }
}
