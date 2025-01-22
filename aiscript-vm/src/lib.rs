mod ai;
mod ast;
mod builtins;
mod chunk;
mod compiler;
mod module;
mod object;
mod parser;
mod stdlib;
mod string;
mod ty;
mod value;
mod vm;

use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Deref;

pub(crate) use aiscript_lexer as lexer;
pub(crate) use chunk::{Chunk, OpCode};
use gc_arena::Collect;
use gc_arena::Mutation;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::Serialize;
pub use value::Value;
use vm::State;
pub use vm::Vm;
pub use vm::VmError;

type NativeFnInner<'gc> = fn(&mut State<'gc>, Vec<Value<'gc>>) -> Result<Value<'gc>, VmError>;
type BuiltinMethodInner<'gc> = fn(
    &'gc Mutation<'gc>,
    // receiver
    Value<'gc>,
    // args
    Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError>;

#[derive(Debug, Clone, Copy)]
pub struct NativeFn<'gc>(NativeFnInner<'gc>);

#[derive(Debug, Clone, Copy)]
pub struct BuiltinMethod<'gc>(BuiltinMethodInner<'gc>);

impl<'gc> Deref for NativeFn<'gc> {
    type Target = NativeFnInner<'gc>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'gc> Deref for BuiltinMethod<'gc> {
    type Target = BuiltinMethodInner<'gc>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

unsafe impl Collect for NativeFn<'_> {
    fn needs_trace() -> bool
    where
        Self: Sized,
    {
        false
    }

    fn trace(&self, _cc: &gc_arena::Collection) {}
}

unsafe impl Collect for BuiltinMethod<'_> {
    fn needs_trace() -> bool
    where
        Self: Sized,
    {
        false
    }

    fn trace(&self, _cc: &gc_arena::Collection) {}
}

#[derive(Debug, PartialEq)]
pub enum ReturnValue {
    Number(f64),
    Boolean(bool),
    String(String),
    Array(Vec<serde_json::Value>),
    Object(HashMap<String, serde_json::Value>),
    Response(HashMap<String, serde_json::Value>),
    Agent(String), // agent name
    Nil,
}

impl ReturnValue {
    pub fn as_object(&self) -> Option<&HashMap<String, serde_json::Value>> {
        match self {
            Self::Object(obj) => Some(obj),
            _ => None,
        }
    }
}

impl Serialize for ReturnValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ReturnValue::Number(n) => serializer.serialize_f64(*n),
            ReturnValue::Boolean(b) => serializer.serialize_bool(*b),
            ReturnValue::String(s) => serializer.serialize_str(s),
            ReturnValue::Array(vec) => {
                let mut s = serializer.serialize_seq(Some(vec.len()))?;
                for item in vec {
                    s.serialize_element(item)?;
                }
                s.end()
            }
            ReturnValue::Object(obj) | ReturnValue::Response(obj) => {
                let mut s = serializer.serialize_map(Some(obj.len()))?;
                for (key, value) in obj {
                    s.serialize_entry(key, value)?;
                }
                s.end()
            }
            ReturnValue::Agent(name) => serializer.serialize_str(name),
            ReturnValue::Nil => serializer.serialize_none(),
        }
    }
}

impl Display for ReturnValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(s) => write!(f, "{s}"),
            Self::Number(n) => write!(f, "{n}"),
            Self::Array(array) => {
                write!(f, "[")?;
                for (i, value) in array.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", value)?;
                }
                write!(f, "]")
            }
            Self::Boolean(b) => write!(f, "{b}"),
            Self::Agent(name) => write!(f, "{name}"),
            Self::Object(obj) | Self::Response(obj) => {
                write!(f, "{}", serde_json::to_string(obj).unwrap())
            }
            Self::Nil => write!(f, ""),
        }
    }
}

impl<'gc> From<Value<'gc>> for ReturnValue {
    fn from(value: Value<'gc>) -> Self {
        match value {
            Value::Number(value) => ReturnValue::Number(value),
            Value::Boolean(value) => ReturnValue::Boolean(value),
            Value::String(value) => ReturnValue::String(value.to_string()),
            Value::IoString(value) => ReturnValue::String(value.to_string()),
            Value::List(value) => ReturnValue::Array(
                value
                    .borrow()
                    .data
                    .iter()
                    .map(|item| item.to_serde_value())
                    .collect::<Vec<_>>(),
            ),
            Value::Instance(instance) => {
                if instance.borrow().class.borrow().name.to_str().unwrap() == "Response" {
                    return ReturnValue::Response(
                        instance
                            .borrow()
                            .fields
                            .iter()
                            .map(|(key, value)| (key.to_string(), value.to_serde_value()))
                            .collect(),
                    );
                } else {
                    ReturnValue::Object(
                        instance
                            .borrow()
                            .fields
                            .iter()
                            .map(|(key, value)| (key.to_string(), value.to_serde_value()))
                            .collect(),
                    )
                }
            }
            Value::Object(obj) => ReturnValue::Object(
                obj.borrow()
                    .fields
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_serde_value()))
                    .collect(),
            ),
            Value::Agent(agent) => ReturnValue::Agent(agent.name.to_string()),
            _ => ReturnValue::Nil,
        }
    }
}

pub fn eval(source: &'static str) -> Result<ReturnValue, VmError> {
    let mut vm = Vm::default();
    vm.compile(source)?;
    vm.interpret()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_expression() {
        assert_eq!(eval("return 1 + 2 * 3;").unwrap(), ReturnValue::Number(7.0));
    }
}
