use serde_json::Value;

use crate::{Directive, DirectiveParams, FromDirective};

#[derive(Debug, Clone, Default)]
pub struct Docs {
    pub deprecated: bool,
    pub hidden: bool,
    pub tag: Option<String>,
}

impl FromDirective for Docs {
    fn from_directive(directive: Directive) -> Result<Self, String> {
        if directive.name != "docs" {
            return Err(format!(
                "Expected 'docs' directive, got '{}'",
                directive.name
            ));
        }

        let mut docs = Docs::default();

        if let DirectiveParams::KeyValue(params) = &directive.params {
            // Parse deprecated flag
            if let Some(Value::Bool(value)) = params.get("deprecated") {
                docs.deprecated = *value;
            } else if params.contains_key("deprecated") {
                return Err("'deprecated' parameter must be a boolean".to_string());
            }

            // Parse hidden flag
            if let Some(Value::Bool(value)) = params.get("hidden") {
                docs.hidden = *value;
            } else if params.contains_key("hidden") {
                return Err("'hidden' parameter must be a boolean".to_string());
            }

            // Parse tag
            if let Some(Value::String(value)) = params.get("tag") {
                docs.tag = Some(value.clone());
            } else if params.contains_key("tag") {
                return Err("'tag' parameter must be a string".to_string());
            }

            // Check for unknown parameters
            for key in params.keys() {
                match key.as_str() {
                    "deprecated" | "hidden" | "tag" => {}
                    _ => return Err(format!("Unknown parameter '{}' for @docs directive", key)),
                }
            }
        } else {
            return Err("@docs directive requires key-value parameters".to_string());
        }

        Ok(docs)
    }
}
