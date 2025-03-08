use std::fmt::Display;

use aiscript_arena::{lock::GcRefLock, Collect, Gc, Mutation, RefLock};

use crate::{
    ai::Agent,
    object::{BoundMethod, Class, Closure, Enum, EnumVariant, Instance, List, ListKind, Object},
    string::{InternedString, StringValue},
    vm::{Context, VmError},
    NativeFn,
};

#[derive(Copy, Clone, Default, Collect)]
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
    // Array(GcRefLock<'gc, Vec<Value<'gc>>>),
    List(GcRefLock<'gc, List<'gc>>),
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

impl PartialEq for Value<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl Display for Value<'_> {
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
            Value::List(list) => {
                let list = list.borrow();
                match list.kind {
                    ListKind::Array => {
                        write!(f, "[")?;
                        for (i, value) in list.data.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", value)?;
                        }
                        write!(f, "]")
                    }
                    ListKind::Tuple => {
                        write!(f, "(")?;
                        for (i, value) in list.data.iter().enumerate() {
                            if i > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{}", value)?;
                        }
                        // Add trailing comma for 1-element tuples
                        if list.data.len() == 1 {
                            write!(f, ",")?;
                        }
                        write!(f, ")")
                    }
                }
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
    pub fn array(mc: &Mutation<'gc>, data: Vec<Value<'gc>>) -> Self {
        Value::List(Gc::new(mc, RefLock::new(List::array(data))))
    }

    #[inline]
    pub fn equals(&self, other: &Value<'gc>) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::String(a), Value::String(b)) => a.equals(b),
            (Value::IoString(a), Value::IoString(b)) => *a == *b,
            (Value::String(a), Value::IoString(b)) => a.as_bytes() == b.as_bytes(),
            (Value::IoString(a), Value::String(b)) => a.as_bytes() == b.as_bytes(),
            (Value::List(a), Value::List(b)) => a.borrow().equals(&b.borrow()),
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
                "cannot convert to number: {}",
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

    // Helper method to convert any string value to a common format
    pub fn as_string_value(&self) -> Result<StringValue<'gc>, VmError> {
        match self {
            Value::String(s) => Ok(StringValue::Interned(*s)),
            Value::IoString(s) => Ok(StringValue::Dynamic(*s)),
            _ => Err(VmError::RuntimeError("Not a string value".into())),
        }
    }

    // Helper to create a new string value
    pub fn new_string(ctx: Context<'gc>, s: &str, should_intern: bool) -> Value<'gc> {
        if should_intern {
            Value::String(ctx.intern(s.as_bytes()))
        } else {
            Value::IoString(Gc::new(&ctx, s.to_string()))
        }
    }

    pub fn as_closure(self) -> Result<Gc<'gc, Closure<'gc>>, VmError> {
        match self {
            Value::Closure(closure) => Ok(closure),
            v => Err(VmError::RuntimeError(format!(
                "cannot convert to closure, the value is {v}"
            ))),
        }
    }

    pub fn as_array(self) -> Result<GcRefLock<'gc, List<'gc>>, VmError> {
        match self {
            Value::List(list) => Ok(list),
            v => Err(VmError::RuntimeError(format!(
                "cannot convert to array, the value is {v}"
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

    pub fn is_error(&self) -> bool {
        match self {
            // Check instance of error class (NetworkError!)
            Value::Instance(instance) => instance.borrow().class.borrow().is_error_type(),
            // Check enum variant from error enum (IOError!::ReadError)
            Value::EnumVariant(variant) => variant.enum_.borrow().is_error_type(),
            _ => false,
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

    pub fn is_native_function(&self) -> bool {
        matches!(self, Value::NativeFunction(_))
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
            serde_json::Value::Array(array) => {
                let data = array
                    .iter()
                    .map(|value| Value::from_serde_value(ctx, value))
                    .collect();
                Value::array(&ctx, data)
            }
            serde_json::Value::Null => Value::Nil,
        }
    }

    pub fn to_serde_value(&self) -> serde_json::Value {
        match self {
            Value::Number(n) => (*n).into(),
            Value::Boolean(b) => (*b).into(),
            Value::String(str) => str.to_string().into(),
            Value::IoString(str) => str.to_string().into(),
            Value::List(list) => serde_json::Value::Array(
                list.borrow()
                    .data
                    .iter()
                    .map(|v| v.to_serde_value())
                    .collect(),
            ),
            Value::Object(obj) => serde_json::Value::Object(
                obj.borrow()
                    .fields
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_serde_value()))
                    .collect(),
            ),
            Value::Instance(instance) => instance
                .borrow()
                .fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_serde_value()))
                .collect(),
            Value::EnumVariant(variant) => variant.value.to_serde_value(),
            _ => serde_json::Value::Null,
        }
    }
}

// Implementations for Enum and EnumVariant
impl Value<'_> {
    pub fn is_enum(&self) -> bool {
        matches!(self, Value::Enum(_))
    }

    pub fn is_enum_variant(&self) -> bool {
        matches!(self, Value::EnumVariant { .. })
    }
}

impl From<u64> for Value<'_> {
    fn from(value: u64) -> Self {
        Value::Number(value as f64)
    }
}

impl From<f64> for Value<'_> {
    fn from(value: f64) -> Self {
        Value::Number(value)
    }
}

impl From<bool> for Value<'_> {
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
