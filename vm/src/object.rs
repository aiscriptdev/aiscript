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

use crate::{Chunk, Value};

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

#[derive(Debug, Clone, Collect)]
#[collect[no_drop]]
pub struct Function<'gc> {
    pub arity: u8,
    pub chunk: Chunk<'gc>,
    pub name: Gc<'gc, String>,
    pub upvalue_count: u16,
}

impl<'gc> Display for Function<'gc> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.name.is_empty() {
            write!(f, "<script>")
        } else {
            write!(f, "<fn {}>", self.name)
        }
    }
}

pub type NativeFn<'gc> = fn(Vec<Value<'gc>>) -> Value<'gc>;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FunctionType {
    Function,
    Method,
    Initializer,
    Script,
}

#[derive(Debug, Collect)]
#[collect(no_drop)]
pub struct Class<'gc> {
    pub name: Gc<'gc, String>,
    pub methods: HashMap<Gc<'gc, String>, Value<'gc>, BuildHasherDefault<AHasher>>,
}

#[derive(Debug, Collect)]
#[collect(no_drop)]
pub struct Instance<'gc> {
    pub class: GcRefLock<'gc, Class<'gc>>,
    pub fields: HashMap<Gc<'gc, String>, Value<'gc>, BuildHasherDefault<AHasher>>,
}

#[derive(Debug, Collect)]
#[collect(no_drop)]
pub struct BoundMethod<'gc> {
    pub receiver: Value<'gc>,
    pub method: Gc<'gc, Closure<'gc>>,
}

impl<'gc> Class<'gc> {
    pub fn new(name: Gc<'gc, String>) -> Self {
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
            .take(function.upvalue_count as usize)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { function, upvalues }
    }
}

impl<'gc> Function<'gc> {
    pub fn new(mc: &'gc Mutation<'gc>, name: &str, arity: u8) -> Self {
        Self {
            arity,
            chunk: Chunk::new(),
            name: Gc::new(mc, name.to_owned()),
            upvalue_count: 0,
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
