use std::{borrow::Cow, collections::HashMap};

use serde_json::Value;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Directive {
    Simple {
        name: String,
        params: HashMap<String, Value>,
    },
    Any(Vec<Directive>), // Must have 2 or more directives
    Not(Box<Directive>),
    In(Vec<Value>),
}

impl Directive {
    pub fn name(&self) -> Cow<'static, str> {
        match self {
            Directive::Simple { name, .. } => Cow::Owned(name.to_owned()),
            Directive::Any(_) => "any".into(),
            Directive::Not(_) => "not".into(),
            Directive::In(_) => "in".into(),
        }
    }
}
