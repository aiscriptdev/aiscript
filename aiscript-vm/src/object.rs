use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    hash::BuildHasherDefault,
    iter,
    ops::{Deref, DerefMut},
};

use ahash::AHasher;
use aiscript_arena::{
    Collect, Gc, Mutation,
    lock::{GcRefLock, RefLock},
};
use aiscript_directive::Validator;

use crate::{Chunk, Value, string::InternedString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Collect)]
#[collect(require_static)]
pub enum ListKind {
    Array,
    Tuple,
}

#[derive(Collect)]
#[collect(no_drop)]
pub struct List<'gc> {
    pub kind: ListKind,
    pub data: Vec<Value<'gc>>,
}

impl<'gc> List<'gc> {
    pub fn array(data: Vec<Value<'gc>>) -> Self {
        Self {
            kind: ListKind::Array,
            data,
        }
    }

    pub fn tuple(data: Vec<Value<'gc>>) -> Self {
        Self {
            kind: ListKind::Tuple,
            data,
        }
    }

    pub fn with_capacity(kind: ListKind, capacity: usize) -> Self {
        Self {
            kind,
            data: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn equals(&self, other: &Self) -> bool {
        self.kind == other.kind && self.data == other.data
    }

    pub fn push(&mut self, value: Value<'gc>) {
        if self.kind == ListKind::Array {
            self.data.push(value);
        }
    }

    pub fn get(&self, index: usize) -> Option<Value<'gc>> {
        self.data.get(index).copied()
    }

    pub fn set(&mut self, index: usize, value: Value<'gc>) -> Result<(), &'static str> {
        match self.kind {
            ListKind::Array => {
                if index < self.data.len() {
                    self.data[index] = value;
                    Ok(())
                } else {
                    Err("Index out of bounds")
                }
            }
            ListKind::Tuple => Err("Cannot modify tuple - tuples are immutable"),
        }
    }
}

#[derive(Collect)]
#[collect[no_drop]]
pub struct UpvalueObj<'gc> {
    pub location: usize,
    pub closed: Option<Value<'gc>>,
    pub next: Option<GcRefLock<'gc, UpvalueObj<'gc>>>,
}

impl Default for UpvalueObj<'_> {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Collect)]
#[collect[no_drop]]
pub struct Closure<'gc> {
    pub function: Gc<'gc, Function<'gc>>,
    pub upvalues: Box<[GcRefLock<'gc, UpvalueObj<'gc>>]>,
}

#[derive(Collect, Default)]
#[collect[no_drop]]
pub struct Function<'gc> {
    pub arity: u8,
    pub max_arity: u8,
    // <name, parameter>
    pub params: HashMap<InternedString<'gc>, Parameter<'gc>>,
    pub chunk: Chunk<'gc>,
    pub name: Option<InternedString<'gc>>,
    pub upvalues: Vec<Upvalue>,
}

#[derive(Collect, Default)]
#[collect[no_drop]]
pub struct Parameter<'gc> {
    // parameter order index
    pub position: u8,
    pub default_value: Value<'gc>,
    #[collect(require_static)]
    pub validators: Vec<Box<dyn Validator>>,
}

impl<'gc> Parameter<'gc> {
    pub fn new(position: u8, default_value: Value<'gc>) -> Self {
        Parameter {
            position,
            default_value,
            validators: Vec::new(),
        }
    }

    pub fn validators(mut self, validators: Vec<Box<dyn Validator>>) -> Self {
        self.validators = validators;
        self
    }
}

#[derive(Debug, Collect)]
#[collect(require_static)]
pub struct Upvalue {
    pub index: usize,
    // that flag controls whether the closure captures a local
    // variable or an upvalue from the surrounding function.
    pub is_local: bool,
}

impl Display for Function<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = self.name {
            write!(f, "<fn {}>", name)
        } else {
            write!(f, "<script>")
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum FunctionType {
    Lambda,
    // Regular functions
    Function {
        is_ai: bool,
    },
    // Class methods
    Method {
        is_ai: bool,
        is_static: bool,
    },
    // Class constructor
    Constructor,
    // Agent tool function
    Tool,
    // Root script function
    #[default]
    Script,
}

impl FunctionType {
    pub fn is_ai_function(&self) -> bool {
        matches!(self, Self::Function { is_ai }  | Self::Method { is_ai, .. } if *is_ai)
    }

    pub fn is_method(&self) -> bool {
        matches!(self, Self::Method { .. })
    }

    pub fn is_lambda(&self) -> bool {
        matches!(self, Self::Lambda { .. })
    }

    pub fn is_static_method(&self) -> bool {
        matches!(self, Self::Method { is_static, .. } if *is_static)
    }

    pub fn is_constructor(&self) -> bool {
        matches!(self, Self::Constructor)
    }
}

#[derive(Collect)]
#[collect(no_drop)]
pub struct Class<'gc> {
    pub name: InternedString<'gc>,
    pub methods: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
    pub static_methods: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
}

#[derive(Collect)]
#[collect(no_drop)]
pub struct Instance<'gc> {
    pub class: GcRefLock<'gc, Class<'gc>>,
    pub fields: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
}

#[derive(Collect)]
#[collect(no_drop)]
pub struct BoundMethod<'gc> {
    pub receiver: Value<'gc>,
    pub method: Gc<'gc, Closure<'gc>>,
}

#[derive(Collect, Default)]
#[collect(no_drop)]
pub struct Object<'gc> {
    pub fields: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
}

#[derive(Collect)]
#[collect(no_drop)]
pub struct Enum<'gc> {
    pub name: InternedString<'gc>,
    // Variant name -> value mapping, default value is Value::Nil
    pub variants: HashMap<InternedString<'gc>, Value<'gc>>,
    // Method name -> function mapping
    pub methods: HashMap<InternedString<'gc>, Value<'gc>>,
    pub static_methods: HashMap<InternedString<'gc>, Value<'gc>>,
}

#[derive(Collect)]
#[collect(no_drop)]
pub struct EnumVariant<'gc> {
    // Reference to enum definition
    pub enum_: GcRefLock<'gc, Enum<'gc>>,
    // Variant name
    pub name: InternedString<'gc>,
    // Variant value, default is Value::Nil
    pub value: Value<'gc>,
}

impl<'gc> Enum<'gc> {
    pub fn is_error_type(&self) -> bool {
        self.name.to_str().unwrap().ends_with('!')
    }

    pub fn get_variant_value(&self, variant_name: InternedString<'gc>) -> Option<Value<'gc>> {
        self.variants.get(&variant_name).copied()
    }
}

impl<'gc> Class<'gc> {
    pub fn new(name: InternedString<'gc>) -> Self {
        Self {
            name,
            methods: HashMap::default(),
            static_methods: HashMap::default(),
        }
    }

    pub fn is_error_type(&self) -> bool {
        self.name.to_str().unwrap().ends_with('!')
    }
}

impl<'gc> Instance<'gc> {
    pub fn new(class: GcRefLock<'gc, Class<'gc>>) -> Self {
        Self {
            class,
            fields: HashMap::default(),
        }
    }
}

impl<'gc> BoundMethod<'gc> {
    pub fn new(receiver: Value<'gc>, method: Gc<'gc, Closure<'gc>>) -> Self {
        Self { receiver, method }
    }
}

impl UpvalueObj<'_> {
    pub fn new(location: usize) -> Self {
        Self {
            location,
            closed: None,
            next: None,
        }
    }
}

impl<'gc> Closure<'gc> {
    pub fn new(mc: &'gc Mutation<'gc>, function: Gc<'gc, Function<'gc>>) -> Self {
        let upvalues = iter::repeat_with(|| Gc::new(mc, RefLock::new(UpvalueObj::default())))
            .take(function.upvalues.len())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { function, upvalues }
    }
}

impl<'gc> Function<'gc> {
    pub fn new(name: InternedString<'gc>, arity: u8) -> Self {
        Self {
            arity,
            max_arity: arity,
            params: HashMap::new(),
            chunk: Chunk::new(),
            name: Some(name),
            upvalues: Vec::new(),
        }
    }

    pub fn shrink_to_fit(&mut self) {
        self.params.shrink_to_fit();
        self.chunk.shrink_to_fit();
        self.upvalues.shrink_to_fit();
    }
}

impl<'gc> Deref for Function<'gc> {
    type Target = Chunk<'gc>;
    fn deref(&self) -> &Self::Target {
        &self.chunk
    }
}

impl DerefMut for Function<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.chunk
    }
}
