use serde_json::Value;
use std::any::Any;
use std::collections::HashSet;

use crate::{Directive, DirectiveParams, FromDirective};

use super::Validator;

pub struct ArrayValidator {
    pub min_len: Option<usize>,
    pub max_len: Option<usize>,
    pub unique: bool,
}

impl Validator for ArrayValidator {
    fn name(&self) -> &'static str {
        "@array"
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        let array = match value.as_array() {
            Some(a) => a,
            None => return Err("Value must be an array".into()),
        };

        // Check minimum length
        if let Some(min_len) = self.min_len {
            if array.len() < min_len {
                return Err(format!(
                    "Array length {} is less than the minimum length of {}",
                    array.len(),
                    min_len
                ));
            }
        }

        // Check maximum length
        if let Some(max_len) = self.max_len {
            if array.len() > max_len {
                return Err(format!(
                    "Array length {} is greater than the maximum length of {}",
                    array.len(),
                    max_len
                ));
            }
        }

        // Check uniqueness if required
        if self.unique && array.len() > 1 {
            let mut seen = HashSet::new();
            for item in array {
                // Convert to string for comparison since not all Value types implement Hash
                let item_str = item.to_string();
                if !seen.insert(item_str) {
                    return Err("Array contains duplicate values".into());
                }
            }
        }

        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl FromDirective for ArrayValidator {
    fn from_directive(directive: Directive) -> Result<Self, String> {
        match directive.params {
            DirectiveParams::KeyValue(params) => {
                let min_len = params
                    .get("min_len")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);

                let max_len = params
                    .get("max_len")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);

                let unique = params
                    .get("unique")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // Validate that min_len <= max_len if both are provided
                if let (Some(min), Some(max)) = (min_len, max_len) {
                    if min > max {
                        return Err(format!(
                            "min_len ({}) cannot be greater than max_len ({})",
                            min, max
                        ));
                    }
                }

                Ok(Self {
                    min_len,
                    max_len,
                    unique,
                })
            }
            _ => Err("Invalid params for @array directive".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Directive, DirectiveParams};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_directive(params: HashMap<String, Value>) -> Directive {
        Directive {
            name: "array".into(),
            params: DirectiveParams::KeyValue(params),
            line: 1,
        }
    }

    #[test]
    fn test_array_validator_basic() {
        let params = HashMap::new();
        let directive = create_directive(params);
        let validator = ArrayValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!([])).is_ok());
        assert!(validator.validate(&json!([1, 2, 3])).is_ok());
        assert!(validator.validate(&json!("not-an-array")).is_err());
    }

    #[test]
    fn test_array_validator_min_length() {
        let mut params = HashMap::new();
        params.insert("min_len".into(), json!(2));
        let directive = create_directive(params);
        let validator = ArrayValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!([1, 2])).is_ok());
        assert!(validator.validate(&json!([1, 2, 3])).is_ok());
        assert!(validator.validate(&json!([1])).is_err());
        assert!(validator.validate(&json!([])).is_err());
    }

    #[test]
    fn test_array_validator_max_length() {
        let mut params = HashMap::new();
        params.insert("max_len".into(), json!(3));
        let directive = create_directive(params);
        let validator = ArrayValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!([])).is_ok());
        assert!(validator.validate(&json!([1, 2, 3])).is_ok());
        assert!(validator.validate(&json!([1, 2, 3, 4])).is_err());
    }

    #[test]
    fn test_array_validator_min_max_length() {
        let mut params = HashMap::new();
        params.insert("min_len".into(), json!(2));
        params.insert("max_len".into(), json!(4));
        let directive = create_directive(params);
        let validator = ArrayValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!([1, 2])).is_ok());
        assert!(validator.validate(&json!([1, 2, 3, 4])).is_ok());
        assert!(validator.validate(&json!([1])).is_err());
        assert!(validator.validate(&json!([1, 2, 3, 4, 5])).is_err());
    }

    #[test]
    fn test_array_validator_uniqueness() {
        let mut params = HashMap::new();
        params.insert("unique".into(), json!(true));
        let directive = create_directive(params);
        let validator = ArrayValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!([1, 2, 3])).is_ok());
        assert!(validator.validate(&json!(["a", "b", "c"])).is_ok());
        assert!(validator.validate(&json!([1, 2, 1])).is_err());
        assert!(validator.validate(&json!(["a", "b", "a"])).is_err());

        // Different types should be considered unique
        assert!(validator.validate(&json!([1, "1", true])).is_ok());

        // Empty array and single element array are always unique
        assert!(validator.validate(&json!([])).is_ok());
        assert!(validator.validate(&json!([1])).is_ok());
    }

    #[test]
    fn test_array_validator_all_constraints() {
        let mut params = HashMap::new();
        params.insert("min_len".into(), json!(2));
        params.insert("max_len".into(), json!(4));
        params.insert("unique".into(), json!(true));
        let directive = create_directive(params);
        let validator = ArrayValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!([1, 2])).is_ok());
        assert!(validator.validate(&json!([1, 2, 3, 4])).is_ok());
        assert!(validator.validate(&json!([1])).is_err()); // Too short
        assert!(validator.validate(&json!([1, 2, 3, 4, 5])).is_err()); // Too long
        assert!(validator.validate(&json!([1, 2, 1])).is_err()); // Not unique
    }

    #[test]
    fn test_invalid_directive_params() {
        // Test with invalid min_len > max_len
        let mut params = HashMap::new();
        params.insert("min_len".into(), json!(5));
        params.insert("max_len".into(), json!(2));
        let directive = create_directive(params);
        assert!(ArrayValidator::from_directive(directive).is_err());
    }

    #[test]
    fn test_validator_name() {
        let params = HashMap::new();
        let directive = create_directive(params);
        let validator = ArrayValidator::from_directive(directive).unwrap();

        assert_eq!(validator.name(), "@array");
    }

    #[test]
    fn test_downcast_ref() {
        let params = HashMap::new();
        let directive = create_directive(params);
        let validator: Box<dyn Validator> =
            Box::new(ArrayValidator::from_directive(directive).unwrap());

        let downcast = validator.downcast_ref::<ArrayValidator>();
        assert!(downcast.is_some());
    }
}
