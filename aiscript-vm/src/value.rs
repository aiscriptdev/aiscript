use std::{collections::HashMap, fmt::Display};

use gc_arena::{lock::GcRefLock, Collect, Gc, RefLock};

use crate::{
    ai::Agent,
    object::{BoundMethod, Class, Closure, Enum, EnumVariant, Instance, Object},
    string::InternedString,
    vm::{Context, VmError},
    NativeFn, ReturnValue,
};

#[derive(Debug, Copy, Clone, Default, Collect)]
#[collect(no_drop)]
pub enum Value<'gc> {
    Number(f64),
    Boolean(bool),
    // For identifiers, module names, etc.
    String(InternedString<'gc>),
    // For file contents, user input, etc. Not interned.
    IoString(Gc<'gc, String>),
    Closure(Gc<'gc, Closure<'gc>>),
    NativeFunction(NativeFn<'gc>),
    Array(GcRefLock<'gc, Vec<Value<'gc>>>),
    Object(GcRefLock<'gc, Object<'gc>>),
    Enum(GcRefLock<'gc, Enum<'gc>>),
    EnumVariant(Gc<'gc, EnumVariant<'gc>>),
    Class(GcRefLock<'gc, Class<'gc>>),
    Instance(GcRefLock<'gc, Instance<'gc>>),
    BoundMethod(Gc<'gc, BoundMethod<'gc>>),
    Module(InternedString<'gc>),
    Agent(Gc<'gc, Agent<'gc>>),
    #[default]
    Nil,
}

impl<'gc> PartialEq for Value<'gc> {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl<'gc> Display for Value<'gc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Number(v) => write!(f, "{}", v),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::String(s) => write!(f, "{}", s),
            Value::IoString(s) => write!(f, "{}", s),
            Value::Closure(closure) => {
                if let Some(name) = closure.function.name {
                    write!(f, "<fn {}>", name)
                } else {
                    write!(f, "<script>")
                }
            }
            Value::NativeFunction(_) => write!(f, "<native fn>"),
            Value::Array(array) => {
                let array = array.borrow();
                write!(f, "[")?;
                for (i, value) in array.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", value)?;
                }
                write!(f, "]")
            }
            Value::Object(obj) => {
                write!(f, "{{")?;
                let mut first = true;
                for (key, value) in &obj.borrow().fields {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", key, value)?;
                    first = false;
                }
                write!(f, "}}")
            }
            Value::Enum(enum_) => write!(f, "enum {}", enum_.borrow().name),
            Value::EnumVariant(variant) => {
                write!(f, "{}::{}", variant.enum_.borrow().name, variant.name)?;
                if !variant.value.is_nil() {
                    write!(f, "({})", variant.value)
                } else {
                    Ok(())
                }
            }
            Value::Class(class) => write!(f, "{}", class.borrow().name),
            Value::Instance(instance) => {
                let mut s = format!("{} {{", instance.borrow().class.borrow().name);
                for (i, (key, value)) in instance.borrow().fields.iter().enumerate() {
                    s.push_str(&format!("{}: {}", key, value));
                    if i != instance.borrow().fields.len() - 1 {
                        s.push_str(", ");
                    }
                }
                s.push('}');
                write!(f, "{}", s)
            }
            Value::BoundMethod(bm) => write!(f, "{}", bm.method.function),
            Value::Agent(agent) => write!(f, "agent {}", agent.name),
            Value::Module(module) => write!(f, "module {}", module),
            Value::Nil => write!(f, "nil"),
        }
    }
}

impl<'gc> Value<'gc> {
    #[inline]
    pub fn equals(&self, other: &Value<'gc>) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::String(a), Value::String(b)) => a.equals(b),
            (Value::IoString(a), Value::IoString(b)) => *a == *b,
            (Value::String(a), Value::IoString(b)) => a.as_bytes() == b.as_bytes(),
            (Value::IoString(a), Value::String(b)) => a.as_bytes() == b.as_bytes(),
            (Value::Array(a), Value::Array(b)) => Gc::ptr_eq(*a, *b),
            (Value::Object(a), Value::Object(b)) => Gc::ptr_eq(*a, *b),
            (Value::Enum(a), Value::Enum(b)) => Gc::ptr_eq(*a, *b),
            (Value::EnumVariant(a), Value::EnumVariant(b)) => {
                // We only need to compare the enum type name and variant name, not the underlying value.
                // The value is just for initialization and access, but doesn't affect the variant's identity.
                Gc::ptr_eq(a.enum_, b.enum_) && a.name == b.name
            }
            (Value::Class(a), Value::Class(b)) => Gc::ptr_eq(*a, *b),
            (Value::Closure(a), Value::Closure(b)) => Gc::ptr_eq(*a, *b),
            (Value::Instance(a), Value::Instance(b)) => Gc::ptr_eq(*a, *b),
            (Value::BoundMethod(a), Value::BoundMethod(b)) => Gc::ptr_eq(*a, *b),
            (Value::Agent(a), Value::Agent(b)) => Gc::ptr_eq(*a, *b),
            (Value::Nil, Value::Nil) => true,
            // _ => core::mem::discriminant(self) == core::mem::discriminant(other),
            _ => false,
        }
    }

    pub fn as_number(self) -> Result<f64, VmError> {
        match self {
            Value::Number(value) => Ok(value),
            a => Err(VmError::RuntimeError(format!(
                "cannot convert to number: {:?}",
                a
            ))),
        }
    }

    pub fn as_boolean(&self) -> bool {
        match self {
            Value::Boolean(value) => *value,
            Value::Number(value) => *value != 0.0,
            Value::String(s) => !s.is_empty(),
            _ => false,
        }
    }

    pub fn as_string(self) -> Result<InternedString<'gc>, VmError> {
        match self {
            Value::String(value) => Ok(value),
            v => Err(VmError::RuntimeError(format!(
                "cannot convert to string, the value is {v}"
            ))),
        }
    }

    pub fn as_closure(self) -> Result<Gc<'gc, Closure<'gc>>, VmError> {
        match self {
            Value::Closure(closure) => Ok(closure),
            v => Err(VmError::RuntimeError(format!(
                "cannot convert to closure, the value is {v:?}"
            ))),
        }
    }

    pub fn as_agent(self) -> Result<Gc<'gc, Agent<'gc>>, VmError> {
        match self {
            Value::Agent(agent) => Ok(agent),
            _ => Err(VmError::RuntimeError("cannot convert to agent.".into())),
        }
    }

    pub fn as_class(self) -> Result<GcRefLock<'gc, Class<'gc>>, VmError> {
        match self {
            Value::Class(class) => Ok(class),
            v => Err(VmError::RuntimeError(format!(
                "cannot convert to class, the value is {v}"
            ))),
        }
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }

    pub fn is_bound_method(&self) -> bool {
        matches!(self, Value::BoundMethod(_))
    }

    pub fn is_class(&self) -> bool {
        matches!(self, Value::Class(_))
    }

    pub fn is_instance(&self) -> bool {
        matches!(self, Value::Instance(_))
    }

    pub fn is_closure(&self) -> bool {
        matches!(self, Value::Closure(_))
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Value::Number(_))
    }

    pub fn is_boolean(&self) -> bool {
        matches!(self, Value::Boolean(_) | Value::Nil)
    }

    pub fn is_true(&self) -> bool {
        !self.is_falsy()
    }

    pub fn is_falsy(&self) -> bool {
        self.is_nil() || (self.is_boolean() && !self.as_boolean())
    }

    pub fn from_serde_value(ctx: Context<'gc>, value: &serde_json::Value) -> Value<'gc> {
        match value {
            serde_json::Value::Bool(b) => Value::Boolean(*b),
            serde_json::Value::Number(number) => Value::Number(number.as_f64().unwrap()),
            serde_json::Value::String(str) => {
                let s = ctx.intern(str.as_bytes());
                Value::from(s)
            }
            serde_json::Value::Object(obj) => {
                let fields = obj
                    .into_iter()
                    .map(|(key, value)| {
                        (
                            ctx.intern(key.as_bytes()),
                            Value::from_serde_value(ctx, value),
                        )
                    })
                    .collect();
                Value::Object(Gc::new(&ctx, RefLock::new(Object { fields })))
            }
            _ => Value::Nil,
        }
    }
}

// Implementations for Enum and EnumVariant
impl<'gc> Value<'gc> {
    pub fn is_enum(&self) -> bool {
        matches!(self, Value::Enum(_))
    }

    pub fn is_enum_variant(&self) -> bool {
        matches!(self, Value::EnumVariant { .. })
    }
}

impl<'gc> From<u64> for Value<'gc> {
    fn from(value: u64) -> Self {
        Value::Number(value as f64)
    }
}

impl<'gc> From<f64> for Value<'gc> {
    fn from(value: f64) -> Self {
        Value::Number(value)
    }
}

impl<'gc> From<bool> for Value<'gc> {
    fn from(value: bool) -> Self {
        Value::Boolean(value)
    }
}

impl<'gc> From<InternedString<'gc>> for Value<'gc> {
    fn from(value: InternedString<'gc>) -> Self {
        Value::String(value)
    }
}

impl<'gc> From<Gc<'gc, String>> for Value<'gc> {
    fn from(value: Gc<'gc, String>) -> Self {
        Value::IoString(value)
    }
}

impl<'gc> From<Gc<'gc, Closure<'gc>>> for Value<'gc> {
    fn from(value: Gc<'gc, Closure<'gc>>) -> Self {
        Value::Closure(value)
    }
}

impl<'gc> From<GcRefLock<'gc, Class<'gc>>> for Value<'gc> {
    fn from(value: GcRefLock<'gc, Class<'gc>>) -> Self {
        Value::Class(value)
    }
}

impl<'gc> From<GcRefLock<'gc, Instance<'gc>>> for Value<'gc> {
    fn from(value: GcRefLock<'gc, Instance<'gc>>) -> Self {
        Value::Instance(value)
    }
}

impl<'gc> From<Gc<'gc, BoundMethod<'gc>>> for Value<'gc> {
    fn from(value: Gc<'gc, BoundMethod<'gc>>) -> Self {
        Value::BoundMethod(value)
    }
}

impl<'gc> From<Gc<'gc, Agent<'gc>>> for Value<'gc> {
    fn from(value: Gc<'gc, Agent<'gc>>) -> Self {
        Value::Agent(value)
    }
}

impl<'gc> From<Value<'gc>> for ReturnValue {
    fn from(value: Value<'gc>) -> Self {
        match value {
            Value::Number(value) => ReturnValue::Number(value),
            Value::Boolean(value) => ReturnValue::Boolean(value),
            Value::String(value) => ReturnValue::String(value.to_string()),
            Value::Instance(instance) => {
                let mut map = HashMap::new();
                for (key, value) in &instance.borrow().fields {
                    let v = match value {
                        Value::Number(n) => (*n).into(),
                        Value::Boolean(b) => (*b).into(),
                        Value::String(str) => str.to_string().into(),
                        Value::Nil => serde_json::Value::Null,
                        _ => continue,
                    };
                    map.insert(key.to_string(), v);
                }
                ReturnValue::Object(map)
            }
            Value::Agent(agent) => ReturnValue::Agent(agent.name.to_string()),
            _ => ReturnValue::Nil,
        }
    }
}
