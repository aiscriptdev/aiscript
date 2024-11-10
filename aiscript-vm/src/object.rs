use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    hash::BuildHasherDefault,
    iter,
    ops::{Deref, DerefMut},
};

use ahash::AHasher;
use gc_arena::{
    lock::{GcRefLock, RefLock},
    Collect, Gc, Mutation,
};

use crate::{string::InternedString, Chunk, Value};

#[derive(Debug, Copy, Clone, Collect)]
#[collect[no_drop]]
pub struct UpvalueObj<'gc> {
    pub location: usize,
    pub closed: Option<Value<'gc>>,
    pub next: Option<GcRefLock<'gc, UpvalueObj<'gc>>>,
}

impl<'gc> Default for UpvalueObj<'gc> {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, Collect)]
#[collect[no_drop]]
pub struct Closure<'gc> {
    pub function: Gc<'gc, Function<'gc>>,
    pub upvalues: Box<[GcRefLock<'gc, UpvalueObj<'gc>>]>,
}

#[derive(Debug, Clone, Collect, Default)]
#[collect[no_drop]]
pub struct Function<'gc> {
    pub arity: u8,
    pub max_arity: u8,
    pub param_names: Vec<String>,
    // <param_name, constant_index>
    pub default_values: HashMap<String, usize>, 
    pub chunk: Chunk<'gc>,
    pub name: Option<InternedString<'gc>>,
    pub upvalues: Vec<Upvalue>,
}

#[derive(Debug, Clone, Copy, Collect)]
#[collect(require_static)]
pub struct Upvalue {
    pub index: usize,
    // that flag controls whether the closure captures a local
    // variable or an upvalue from the surrounding function.
    pub is_local: bool,
}

impl<'gc> Display for Function<'gc> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = self.name {
            write!(f, "<fn {}>", name)
        } else {
            write!(f, "<script>")
        }
    }
}

pub type NativeFn<'gc> = fn(Vec<Value<'gc>>) -> Value<'gc>;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FunctionType {
    AiFunction,
    Function,
    Tool,
    Method,
    Initializer,
    Script,
}

#[derive(Debug, Collect)]
#[collect(no_drop)]
pub struct Class<'gc> {
    pub name: InternedString<'gc>,
    pub methods: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
}

#[derive(Debug, Collect)]
#[collect(no_drop)]
pub struct Instance<'gc> {
    pub class: GcRefLock<'gc, Class<'gc>>,
    pub fields: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
}

#[derive(Debug, Collect)]
#[collect(no_drop)]
pub struct BoundMethod<'gc> {
    pub receiver: Value<'gc>,
    pub method: Gc<'gc, Closure<'gc>>,
}

impl<'gc> Class<'gc> {
    pub fn new(name: InternedString<'gc>) -> Self {
        Self {
            name,
            methods: HashMap::default(),
        }
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

impl<'gc> UpvalueObj<'gc> {
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
            param_names: Vec::new(),
            default_values: HashMap::new(),
            chunk: Chunk::new(),
            name: Some(name),
            upvalues: Vec::new(),
        }
    }
}

impl<'gc> Deref for Function<'gc> {
    type Target = Chunk<'gc>;
    fn deref(&self) -> &Self::Target {
        &self.chunk
    }
}

impl<'gc> DerefMut for Function<'gc> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.chunk
    }
}
