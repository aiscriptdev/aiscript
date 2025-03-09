use std::any::Any;

use chrono::NaiveDate;
use serde_json::Value;

use crate::{Directive, DirectiveParams, FromDirective};

use super::Validator;

pub struct DateValidator {
    pub format: Option<String>,
    pub min: Option<String>,
    pub max: Option<String>,
}

impl Validator for DateValidator {
    fn name(&self) -> &'static str {
        "@date"
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        let value_str = match value.as_str() {
            Some(s) => s,
            None => return Err("Value must be a string".into()),
        };

        // Get the format and convert it to chrono format
        let user_format = self.format.as_deref().unwrap_or("YYYY-MM-DD");
        let chrono_format = convert_format(user_format);

        // Parse the date
        let date = match NaiveDate::parse_from_str(value_str, &chrono_format) {
            Ok(date) => date,
            Err(_) => return Err(format!("Invalid date format, expected {}", user_format)),
        };

        // Check minimum date constraint
        if let Some(min_str) = &self.min {
            match NaiveDate::parse_from_str(min_str, &chrono_format) {
                Ok(min_date) => {
                    if date < min_date {
                        return Err(format!("Date must be on or after {}", min_str));
                    }
                }
                Err(_) => return Err(format!("Invalid minimum date: {}", min_str)),
            }
        }

        // Check maximum date constraint
        if let Some(max_str) = &self.max {
            match NaiveDate::parse_from_str(max_str, &chrono_format) {
                Ok(max_date) => {
                    if date > max_date {
                        return Err(format!("Date must be on or before {}", max_str));
                    }
                }
                Err(_) => return Err(format!("Invalid maximum date: {}", max_str)),
            }
        }

        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl FromDirective for DateValidator {
    fn from_directive(directive: Directive) -> Result<Self, String> {
        match directive.params {
            DirectiveParams::KeyValue(params) => {
                let format = params
                    .get("format")
                    .and_then(|v| v.as_str().map(ToString::to_string));
                let min = params
                    .get("min")
                    .and_then(|v| v.as_str().map(ToString::to_string));
                let max = params
                    .get("max")
                    .and_then(|v| v.as_str().map(ToString::to_string));

                // Validate that min and max follow the format if provided
                if let (Some(format_str), Some(min_str)) = (&format, &min) {
                    let chrono_format = convert_format(format_str);
                    if NaiveDate::parse_from_str(min_str, &chrono_format).is_err() {
                        return Err(format!(
                            "min date '{}' doesn't match format '{}'",
                            min_str, format_str
                        ));
                    }
                }

                if let (Some(format_str), Some(max_str)) = (&format, &max) {
                    let chrono_format = convert_format(format_str);
                    if NaiveDate::parse_from_str(max_str, &chrono_format).is_err() {
                        return Err(format!(
                            "max date '{}' doesn't match format '{}'",
                            max_str, format_str
                        ));
                    }
                }

                Ok(Self { format, min, max })
            }
            _ => Err("Invalid params for @date directive".into()),
        }
    }
}

fn convert_format(user_format: &str) -> String {
    user_format
        .replace("YYYY", "%Y")
        .replace("YY", "%y")
        .replace("MM", "%m")
        .replace("DD", "%d")
        .replace("M", "%m")
        .replace("D", "%d")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Directive, DirectiveParams};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_directive(params: HashMap<String, Value>) -> Directive {
        Directive {
            name: "date".into(),
            params: DirectiveParams::KeyValue(params),
            line: 1,
        }
    }

    #[test]
    fn test_date_validator_basic() {
        let params = HashMap::new();
        let directive = create_directive(params);
        let validator = DateValidator::from_directive(directive).unwrap();

        // Default format is %Y-%m-%d
        assert!(validator.validate(&json!("2023-05-15")).is_ok());
        assert!(validator.validate(&json!("not-a-date")).is_err());
        assert!(validator.validate(&json!("20230515")).is_err());
        assert!(validator.validate(&json!(123)).is_err());
    }

    #[test]
    fn test_date_validator_with_format() {
        let mut params = HashMap::new();
        params.insert("format".into(), json!("MM/DD/YYYY"));
        let directive = create_directive(params);
        let validator = DateValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("05/15/2023")).is_ok());
        assert!(validator.validate(&json!("2023-05-15")).is_err());
    }

    #[test]
    fn test_date_validator_min_constraint() {
        let mut params = HashMap::new();
        params.insert("min".into(), json!("2023-01-01"));
        let directive = create_directive(params);
        let validator = DateValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("2023-01-01")).is_ok()); // Equal to min
        assert!(validator.validate(&json!("2023-02-15")).is_ok()); // After min
        assert!(validator.validate(&json!("2022-12-31")).is_err()); // Before min
    }

    #[test]
    fn test_date_validator_max_constraint() {
        let mut params = HashMap::new();
        params.insert("max".into(), json!("2023-12-31"));
        let directive = create_directive(params);
        let validator = DateValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("2023-12-31")).is_ok()); // Equal to max
        assert!(validator.validate(&json!("2023-06-15")).is_ok()); // Before max
        assert!(validator.validate(&json!("2024-01-01")).is_err()); // After max
    }

    #[test]
    fn test_date_validator_min_max_constraints() {
        let mut params = HashMap::new();
        params.insert("min".into(), json!("2023-01-01"));
        params.insert("max".into(), json!("2023-12-31"));
        let directive = create_directive(params);
        let validator = DateValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("2023-01-01")).is_ok()); // Equal to min
        assert!(validator.validate(&json!("2023-12-31")).is_ok()); // Equal to max
        assert!(validator.validate(&json!("2023-06-15")).is_ok()); // Between min and max
        assert!(validator.validate(&json!("2022-12-31")).is_err()); // Before min
        assert!(validator.validate(&json!("2024-01-01")).is_err()); // After max
    }

    #[test]
    fn test_date_validator_format_with_constraints() {
        let mut params = HashMap::new();
        params.insert("format".into(), json!("YYYY-MM-DD"));
        params.insert("min".into(), json!("2023-01-01"));
        params.insert("max".into(), json!("2023-12-31"));
        let directive = create_directive(params);
        let validator = DateValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("2023-01-01")).is_ok());
        assert!(validator.validate(&json!("2023-12-31")).is_ok());
        assert!(validator.validate(&json!("2023-06-15")).is_ok());
        assert!(validator.validate(&json!("2022-12-31")).is_err());
        assert!(validator.validate(&json!("2024-01-01")).is_err());
    }

    #[test]
    fn test_date_validator_different_formats() {
        // Test YY-M-D format
        let mut params = HashMap::new();
        params.insert("format".into(), json!("YY-M-D"));
        let directive = create_directive(params);
        let validator = DateValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("23-5-15")).is_ok());
        assert!(validator.validate(&json!("23-05-15")).is_ok());
        assert!(validator.validate(&json!("2023-5-15")).is_err());

        // Test DD.MM.YYYY format
        let mut params = HashMap::new();
        params.insert("format".into(), json!("DD.MM.YYYY"));
        let directive = create_directive(params);
        let validator = DateValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("15.05.2023")).is_ok());
        assert!(validator.validate(&json!("05.15.2023")).is_err()); // Invalid month
    }

    #[test]
    fn test_invalid_directive_params() {
        // Test with invalid min date
        let mut params = HashMap::new();
        params.insert("format".into(), json!("YYYY-MM-DD"));
        params.insert("min".into(), json!("invalid-date"));
        let directive = create_directive(params);
        assert!(DateValidator::from_directive(directive).is_err());

        // Test with invalid max date
        let mut params = HashMap::new();
        params.insert("format".into(), json!("YYYY-MM-DD"));
        params.insert("max".into(), json!("invalid-date"));
        let directive = create_directive(params);
        assert!(DateValidator::from_directive(directive).is_err());
    }

    #[test]
    fn test_validator_name() {
        let params = HashMap::new();
        let directive = create_directive(params);
        let validator = DateValidator::from_directive(directive).unwrap();

        assert_eq!(validator.name(), "@date");
    }

    #[test]
    fn test_downcast_ref() {
        let params = HashMap::new();
        let directive = create_directive(params);
        let validator: Box<dyn Validator> =
            Box::new(DateValidator::from_directive(directive).unwrap());

        let downcast = validator.downcast_ref::<DateValidator>();
        assert!(downcast.is_some());
    }
}
