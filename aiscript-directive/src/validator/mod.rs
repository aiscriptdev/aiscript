use std::any::Any;

use date::DateValidator;
use regex::RegexValidator;
use serde_json::Value;

use crate::{Directive, DirectiveParams, FromDirective};

mod array;
mod date;
mod format;
mod regex;

pub trait Validator: Send + Sync + Any {
    fn name(&self) -> &'static str;
    fn validate(&self, value: &Value) -> Result<(), String>;
    fn as_any(&self) -> &dyn Any;
    fn downcast_ref<U: Any>(&self) -> Option<&U>
    where
        Self: Sized,
    {
        self.as_any().downcast_ref::<U>()
    }
}

// Nighly feature gate required
// impl dyn Validator {
//     pub fn as_any(&self) -> &dyn Any where Self: 'static {
//         self
//     }
// }

impl Validator for Box<dyn Validator> {
    fn name(&self) -> &'static str {
        self.as_ref().name()
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        self.as_ref().validate(value)
    }

    fn as_any(&self) -> &dyn Any {
        self.as_ref().as_any()
    }
}

impl<T: Validator> Validator for Box<T> {
    fn name(&self) -> &'static str {
        self.as_ref().name()
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        self.as_ref().validate(value)
    }

    fn as_any(&self) -> &dyn Any {
        self.as_ref().as_any()
    }
}

pub struct AnyValidator<V>(pub Box<[V]>);

pub struct NotValidator<V>(pub V);

#[derive(Default)]
pub struct StringValidator {
    pub min_len: Option<u32>,
    pub max_len: Option<u32>,
    pub exact_len: Option<u32>,
    // regex: Option<String>,
    pub start_with: Option<String>,
    pub end_with: Option<String>,
}

pub struct NumberValidator {
    min: Option<f64>,
    max: Option<f64>,
    equal: Option<f64>,
    strict_int: Option<bool>,
    strict_float: Option<bool>,
}

pub struct InValidator(pub Vec<Value>);

impl<V: Validator> Validator for AnyValidator<V> {
    fn name(&self) -> &'static str {
        "@any"
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        for validator in &self.0 {
            validator.validate(value)?
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<V: Validator> Validator for NotValidator<V> {
    fn name(&self) -> &'static str {
        "@not"
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        let validator = &self.0;
        if validator.validate(value).is_ok() {
            return Err("Value does not meet the validation criteria".into());
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Validator for StringValidator {
    fn name(&self) -> &'static str {
        "@string"
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        let value = value.as_str().unwrap();
        if let Some(min_len) = self.min_len {
            if value.len() < min_len as usize {
                return Err(format!(
                    "String length is less than the minimum length of {}",
                    min_len
                ));
            }
        }
        if let Some(max_len) = self.max_len {
            if value.len() > max_len as usize {
                return Err(format!(
                    "String length is greater than the maximum length of {}",
                    max_len
                ));
            }
        }

        if let Some(exact_len) = self.exact_len {
            if value.len() != exact_len as usize {
                return Err(format!(
                    "String length is not equal to the exact length of {}",
                    exact_len
                ));
            }
        }

        // if let Some(regex) = &self.regex {
        //     let regex = regex::Regex::new(regex).unwrap();
        //     if !regex.is_match(value) {
        //         return Err(format!(
        //             "String does not match the required regex pattern: {}",
        //             regex
        //         ));
        //     }
        // }

        if let Some(start_with) = &self.start_with {
            if !value.starts_with(start_with) {
                return Err(format!(
                    "String does not start with the required string: {}",
                    start_with
                ));
            }
        }

        if let Some(end_with) = &self.end_with {
            if !value.ends_with(end_with) {
                return Err(format!(
                    "String does not end with the required string: {}",
                    end_with
                ));
            }
        }

        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Validator for NumberValidator {
    fn name(&self) -> &'static str {
        "@number"
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        let num = value.as_number().unwrap();
        let value = num.as_f64().unwrap();
        if let (Some(true), Some(true)) = (self.strict_int, self.strict_float) {
            return Err("Cannot set both strict_int and strict_float to true".into());
        }
        if let Some(true) = self.strict_int {
            if !num.is_i64() {
                return Err("Value must be an integer".into());
            }
        }
        if let Some(true) = self.strict_float {
            if num.is_i64() {
                return Err("Value must be a float".into());
            }
        }
        if let Some(min) = self.min {
            if value < min {
                return Err(format!("Number is less than the minimum value of {}", min));
            }
        }
        if let Some(max) = self.max {
            if value > max {
                return Err(format!(
                    "Number is greater than the maximum value of {}",
                    max
                ));
            }
        }
        if let Some(equal) = self.equal {
            if value != equal {
                return Err(format!(
                    "Number is not equal to the required value of {}",
                    equal
                ));
            }
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Validator for InValidator {
    fn name(&self) -> &'static str {
        "@in"
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        if self.0.contains(value) {
            Ok(())
        } else {
            Err("Value is not in the list of allowed values".into())
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl FromDirective for Box<dyn Validator> {
    fn from_directive(directive: Directive) -> Result<Self, String>
    where
        Self: Sized,
    {
        match directive.name.as_str() {
            "string" => Ok(Box::new(StringValidator::from_directive(directive)?)),
            "number" => Ok(Box::new(NumberValidator::from_directive(directive)?)),
            "in" => Ok(Box::new(InValidator::from_directive(directive)?)),
            "any" => Ok(Box::new(AnyValidator::from_directive(directive)?)),
            "not" => Ok(Box::new(NotValidator::from_directive(directive)?)),
            "date" => Ok(Box::new(DateValidator::from_directive(directive)?)),
            "array" => Ok(Box::new(AnyValidator::from_directive(directive)?)), // Add this line
            "regex" => Ok(Box::new(RegexValidator::from_directive(directive)?)), // Add support for regex directive
            v => Err(format!("Invalid validators: @{}", v)),
        }
    }
}

impl FromDirective for StringValidator {
    fn from_directive(Directive { params, .. }: Directive) -> Result<Self, String> {
        match params {
            DirectiveParams::KeyValue(params) => {
                Ok(Self {
                    min_len: params
                        .get("min_len")
                        .and_then(|v| v.as_u64().map(|v| v as u32)),
                    max_len: params
                        .get("max_len")
                        .and_then(|v| v.as_u64().map(|v| v as u32)),
                    exact_len: params
                        .get("exact_len")
                        .and_then(|v| v.as_u64().map(|v| v as u32)),
                    // regex: params
                    //     .get("regex")
                    //     .and_then(|v| v.as_str().map(|v| v.to_string())),
                    start_with: params
                        .get("start_with")
                        .and_then(|v| v.as_str().map(|v| v.to_string())),
                    end_with: params
                        .get("end_with")
                        .and_then(|v| v.as_str().map(|v| v.to_string())),
                })
            }
            _ => Err("Invalid params for @string directive".into()),
        }
    }
}

impl FromDirective for InValidator {
    fn from_directive(Directive { params, .. }: Directive) -> Result<Self, String> {
        match params {
            DirectiveParams::Array(values) => Ok(Self(values)),
            _ => Err("Invalid params for @in directive".into()),
        }
    }
}

impl FromDirective for AnyValidator<Box<dyn Validator>> {
    fn from_directive(Directive { params, .. }: Directive) -> Result<Self, String> {
        match params {
            DirectiveParams::Directives(directives) => {
                let mut validators = Vec::with_capacity(directives.len());
                for directive in directives {
                    validators.push(FromDirective::from_directive(directive)?);
                }
                Ok(Self(validators.into_boxed_slice()))
            }
            _ => Err("Invalid params for @any directive".into()),
        }
    }
}

impl FromDirective for NotValidator<Box<dyn Validator>> {
    fn from_directive(Directive { params, .. }: Directive) -> Result<Self, String> {
        match params {
            DirectiveParams::Directives(mut directives) => {
                if let Some(directive) = directives.pop() {
                    let validator = FromDirective::from_directive(directive)?;
                    if !directives.is_empty() {
                        return Err("@not directive only support one directive".into());
                    }

                    Ok(Self(validator))
                } else {
                    Err("@not directive requires one directive".into())
                }
            }
            _ => Err("Invalid params for @not directive, expect a directive".into()),
        }
    }
}

impl FromDirective for NumberValidator {
    fn from_directive(Directive { params, .. }: Directive) -> Result<Self, String> {
        match params {
            DirectiveParams::KeyValue(params) => Ok(Self {
                min: params.get("min").and_then(|v| v.as_f64()),
                max: params.get("max").and_then(|v| v.as_f64()),
                equal: params.get("equal").and_then(|v| v.as_f64()),
                strict_int: params.get("strict_int").and_then(|v| v.as_bool()),
                strict_float: params.get("strict_float").and_then(|v| v.as_bool()),
            }),
            _ => Err("Invalid params for @number directive".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Simple validator implementations for testing
    #[derive(Debug)]
    struct RangeValidator {
        min: i64,
        max: i64,
    }

    impl RangeValidator {
        fn new(min: i64, max: i64) -> Self {
            Self { min, max }
        }
    }

    impl Validator for RangeValidator {
        fn name(&self) -> &'static str {
            "range"
        }

        fn validate(&self, value: &Value) -> Result<(), String> {
            if let Some(num) = value.as_i64() {
                if num >= self.min && num <= self.max {
                    Ok(())
                } else {
                    Err(format!(
                        "Value must be between {} and {}",
                        self.min, self.max
                    ))
                }
            } else {
                Err("Value must be an integer".to_string())
            }
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[test]
    fn test_direct_downcast() {
        let range_validator = RangeValidator::new(1, 10);

        // Downcast using concrete type
        let downcast_result = range_validator.downcast_ref::<RangeValidator>();
        assert!(downcast_result.is_some());

        let range = downcast_result.unwrap();
        assert_eq!(range.min, 1);
        assert_eq!(range.max, 10);

        let range_validator = RangeValidator::new(1, 10);
        let validator: Box<dyn Validator> = Box::new(range_validator);
        let v = validator.downcast_ref::<RangeValidator>().unwrap();
        assert_eq!(v.min, 1);
        assert_eq!(v.max, 10);
    }

    #[test]
    fn test_nested_downcast() {
        let inner_validator = Box::new(RangeValidator::new(1, 10));
        let not_validator = NotValidator(inner_validator);

        // Test successful downcast
        let downcast_result = not_validator.downcast_ref::<NotValidator<Box<RangeValidator>>>();
        assert!(downcast_result.is_some());
    }

    #[test]
    fn test_any_validator_downcast() {
        let validators = vec![
            Box::new(RangeValidator::new(1, 10)),
            Box::new(RangeValidator::new(0, 5)),
        ];
        let any_validator = AnyValidator(validators.into_boxed_slice());

        // Test successful downcast
        let downcast_result = any_validator.downcast_ref::<AnyValidator<Box<RangeValidator>>>();
        assert!(downcast_result.is_some());

        // Verify the inner validators
        let any = downcast_result.unwrap();
        assert_eq!(any.0.len(), 2);
    }

    #[test]
    fn test_wrong_downcast() {
        let range_validator = RangeValidator::new(1, 10);

        // Try to downcast to wrong types
        let not_result = range_validator.downcast_ref::<NotValidator<Box<dyn Validator>>>();
        assert!(not_result.is_none());

        let any_result = range_validator.downcast_ref::<AnyValidator<Box<dyn Validator>>>();
        assert!(any_result.is_none());
    }

    #[test]
    fn test_downcast_and_validate() {
        let range_validator = RangeValidator::new(1, 10);

        // Downcast and validate
        if let Some(range) = range_validator.downcast_ref::<RangeValidator>() {
            assert!(range.validate(&json!(5)).is_ok());
            assert!(range.validate(&json!(0)).is_err());
            assert!(range.validate(&json!(11)).is_err());
            assert!(range.validate(&json!("not a number")).is_err());
        } else {
            panic!("Downcast failed");
        }
    }

    #[test]
    fn test_nested_validator_chain() {
        let range = Box::new(RangeValidator::new(1, 10));
        let not = NotValidator(range);
        let any = AnyValidator(vec![not].into_boxed_slice());

        // Test validation behavior of the chain
        assert!(any.validate(&json!(0)).is_ok()); // Outside range, so NotValidator makes it valid
        assert!(any.validate(&json!(5)).is_err()); // Inside range, so NotValidator makes it invalid

        // Test downcasting of each layer
        let any_downcast = any
            .downcast_ref::<AnyValidator<NotValidator<Box<RangeValidator>>>>()
            .expect("Should downcast to AnyValidator");
        assert_eq!(any_downcast.0.len(), 1);
    }
}
