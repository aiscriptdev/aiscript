// Maybe integrate with pydantic-core?
// https://github.com/pydantic/pydantic-core

use serde_json::Value;

#[derive(Clone, Debug)]
pub enum ValidatorKind {
    Length(Length),
    Format(Format),
}

#[derive(Debug, Clone)]
pub struct Length {
    pub min: Option<usize>,
    pub max: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum Format {
    HttpUrl,
    Email,
}

impl ValidatorKind {
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        match self {
            ValidatorKind::Length(length) => length.validate(value),
            ValidatorKind::Format(format) => format.validate(value),
        }
    }
}

impl Length {
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        let value = value.as_str().unwrap();
        if let Some(min) = self.min {
            if value.len() < min {
                return Err(format!(
                    "Field does not meet minimum length requirement: {}",
                    min
                ));
            }
        }
        if let Some(max) = self.max {
            if value.len() > max {
                return Err(format!(
                    "Field does not meet maximum length requirement: {}",
                    max
                ));
            }
        }
        Ok(())
    }
}

impl Format {
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        let value = value.as_str().unwrap();
        match self {
            Format::HttpUrl => {
                // TODO: Implement HTTP URL validation
                Ok(())
            }
            Format::Email => {
                // TODO: Implement email validation
                Ok(())
            }
        }
    }
}
