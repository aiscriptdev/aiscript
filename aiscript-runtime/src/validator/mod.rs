use serde_json::Value;

use crate::Directive;

pub trait Validator: Send + Sync + 'static {
    fn validate(&self, value: &Value) -> Result<(), String>;
}

impl Validator for Box<dyn Validator> {
    fn validate(&self, value: &Value) -> Result<(), String> {
        self.as_ref().validate(value)
    }
}

pub struct AnyValidator<V>(Box<[V]>);

pub struct NotValidator<V>(V);

pub struct StringValidator {
    min_len: Option<u32>,
    max_len: Option<u32>,
    exact_len: Option<u32>,
    regex: Option<String>,
    start_with: Option<String>,
    end_with: Option<String>,
}

pub struct NumberValidator {
    min: Option<f64>,
    max: Option<f64>,
    equal: Option<f64>,
}

pub struct InValidator(Vec<Value>);

impl<V: Validator> Validator for AnyValidator<V> {
    fn validate(&self, value: &Value) -> Result<(), String> {
        for validator in &self.0 {
            validator.validate(value)?
        }
        Ok(())
    }
}

impl<V: Validator> Validator for NotValidator<V> {
    fn validate(&self, value: &Value) -> Result<(), String> {
        let validator = &self.0;
        if validator.validate(value).is_ok() {
            return Err("Value does not meet the validation criteria".into());
        }
        Ok(())
    }
}

impl Validator for StringValidator {
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
}

impl Validator for NumberValidator {
    fn validate(&self, value: &Value) -> Result<(), String> {
        let value = value.as_f64().unwrap();
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
}

impl Validator for InValidator {
    fn validate(&self, value: &Value) -> Result<(), String> {
        if self.0.contains(value) {
            Ok(())
        } else {
            Err("Value is not in the list of allowed values".into())
        }
    }
}

pub fn convert_from_directive(directive: Directive) -> Box<dyn Validator> {
    match directive {
        Directive::Simple { name, params } => match &*name {
            // TODO: validate params
            "string" => Box::new(StringValidator {
                min_len: params
                    .get("min_len")
                    .and_then(|v| v.as_u64().map(|v| v as u32)),
                max_len: params
                    .get("max_len")
                    .and_then(|v| v.as_u64().map(|v| v as u32)),
                exact_len: params
                    .get("exact_len")
                    .and_then(|v| v.as_u64().map(|v| v as u32)),
                regex: params
                    .get("regex")
                    .and_then(|v| v.as_str().map(|v| v.to_string())),
                start_with: params
                    .get("start_with")
                    .and_then(|v| v.as_str().map(|v| v.to_string())),
                end_with: params
                    .get("end_with")
                    .and_then(|v| v.as_str().map(|v| v.to_string())),
            }),
            _ => {
                panic!("Unsupported directive: @{}", name)
            }
        },
        Directive::Any(directives) => Box::new(AnyValidator(
            directives
                .into_iter()
                .map(|directive| convert_from_directive(directive))
                .collect::<Vec<Box<dyn Validator>>>()
                .into_boxed_slice(),
        )),
        Directive::Not(directive) => Box::new(NotValidator(convert_from_directive(*directive))),
        Directive::In(values) => Box::new(InValidator(values)),
    }
}
