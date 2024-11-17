use std::{collections::HashMap, fmt::Display, ops};

use gc_arena::{arena::CollectionPhase, lock::RefLock, Arena, Gc, Mutation, Rootable};
pub use state::State;

use crate::{
    object::{Class, Instance},
    stdlib,
    string::{InternedString, InternedStringSet},
    ReturnValue, Value,
};
use fuel::Fuel;

mod fuel;
mod state;

#[derive(Debug)]
pub enum VmError {
    CompileError,
    RuntimeError(std::string::String),
}

impl std::error::Error for VmError {}

impl Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CompileError => write!(f, "CompileError"),
            Self::RuntimeError(s) => write!(f, "RuntimeError: {s}"),
        }
    }
}

pub struct Vm {
    arena: Arena<Rootable![State<'_>]>,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    pub fn new() -> Self {
        let mut vm = Vm {
            arena: Arena::<Rootable![State<'_>]>::new(|mc| State::new(mc)),
        };
        vm.init_stdlib();
        vm
    }

    fn init_stdlib(&mut self) {
        self.arena.mutate_root(|mc, state| {
            let ctx = Context {
                mutation: mc,
                strings: state.strings,
            };

            // Initialize standard library modules
            state
                .module_manager
                .register_native_module(ctx.intern(b"std.math"), stdlib::create_math_module(ctx));
            state
                .module_manager
                .register_native_module(ctx.intern(b"std.io"), stdlib::create_io_module(ctx));
        });
    }

    #[cfg(feature = "v1")]
    pub fn compile(&mut self, source: &'static str) -> Result<(), VmError> {
        self.arena.mutate_root(|mc, state| {
            let context = Context {
                mutation: mc,
                strings: state.strings,
            };
            let function = crate::v1::compile(context, source)?;
            #[cfg(feature = "debug")]
            function.disassemble("script");
            state.define_builtins();
            let closure = Gc::new(mc, Closure::new(mc, Gc::new(mc, function)));
            state.push_stack(Value::from(closure));
            state.call(closure, 0)
        })?;
        Ok(())
    }

    #[cfg(not(feature = "v1"))]
    pub fn compile(&mut self, source: &'static str) -> Result<(), VmError> {
        self.arena.mutate_root(|mc, state| {
            let context = Context {
                mutation: mc,
                strings: state.strings,
            };
            state.chunks = crate::compiler::compile(context, source)?;
            state.define_builtins();
            // The script function's chunk id is always the highest chunk id.
            let script_chunk_id = state.chunks.keys().max().copied().unwrap();
            state.call_function(script_chunk_id, vec![])
        })?;
        Ok(())
    }

    pub fn interpret(&mut self) -> Result<ReturnValue, VmError> {
        loop {
            const FUEL_PER_GC: i32 = 1024 * 10;
            let mut fuel = Fuel::new(FUEL_PER_GC);
            // periodically exit the arena in order to collect garbage concurrently with running the VM.
            let result = self.arena.mutate_root(|_, state| state.step(&mut fuel));

            const COLLECTOR_GRANULARITY: f64 = 10240.0;
            if self.arena.metrics().allocation_debt() > COLLECTOR_GRANULARITY {
                // Do garbage collection.
                #[cfg(feature = "debug")]
                println!("Collecting...");
                if self.arena.collection_phase() == CollectionPhase::Sweeping {
                    self.arena.collect_debt();
                } else {
                    // Immediately transition to `CollectionPhase::Sweeping`.
                    self.arena.mark_all().unwrap().start_sweeping();
                }
            }

            match result {
                Ok(result) => {
                    if let Some(value) = result {
                        return Ok(value);
                    }
                }
                Err(err) => return Err(err),
            }
        }
    }
}

impl Vm {
    pub fn inject_variables(&mut self, variables: HashMap<String, serde_json::Value>) {
        self.arena.mutate_root(|_mc, state| {
            for (key, value) in variables {
                let name = state.intern(key.as_bytes());
                let v = match value {
                    serde_json::Value::Bool(b) => Value::Boolean(b),
                    serde_json::Value::Number(number) => Value::Number(number.as_f64().unwrap()),
                    serde_json::Value::String(str) => {
                        let s = state.intern(str.as_bytes());
                        Value::String(s)
                    }
                    serde_json::Value::Null => Value::Nil,
                    _ => continue,
                };
                state.globals.insert(name, v);
            }
        });
    }

    pub fn inject_instance(
        &mut self,
        name: &'static str,
        fields: HashMap<&'static str, serde_json::Value>,
    ) {
        self.arena.mutate_root(|mc, state| {
            let name = state.intern_static(name);
            let class = Gc::new(mc, RefLock::new(Class::new(name)));
            let mut instance = Instance::new(class);
            for (key, value) in fields {
                let v = match value {
                    serde_json::Value::Bool(b) => Value::Boolean(b),
                    serde_json::Value::Number(number) => Value::Number(number.as_f64().unwrap()),
                    serde_json::Value::String(str) => {
                        let s = state.intern(str.as_bytes());
                        Value::from(s)
                    }
                    serde_json::Value::Null => Value::Nil,
                    _ => continue,
                };
                instance.fields.insert(state.intern_static(key), v);
            }
            state
                .globals
                .insert(name, Gc::new(mc, RefLock::new(instance)).into());
        });
    }
}

#[derive(Copy, Clone)]
pub struct Context<'gc> {
    pub mutation: &'gc Mutation<'gc>,
    pub strings: InternedStringSet<'gc>,
}

impl<'gc> Context<'gc> {
    pub fn intern(self, s: &[u8]) -> InternedString<'gc> {
        self.strings.intern(&self, s)
    }

    #[allow(unused)]
    pub fn intern_static(self, s: &'static str) -> InternedString<'gc> {
        self.strings.intern_static(&self, s.as_bytes())
    }
}

impl<'gc> ops::Deref for Context<'gc> {
    type Target = Mutation<'gc>;

    fn deref(&self) -> &Self::Target {
        self.mutation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_variables() {
        let mut vm = Vm::new();
        vm.inject_variables({
            let mut map = HashMap::new();
            map.insert("test".into(), "abc".into());
            map.insert("test2".into(), 123.into());
            map.insert("test3".into(), true.into());
            map
        });
        vm.compile("return test;").unwrap();
        let result = vm.interpret().unwrap();
        assert_eq!(result, ReturnValue::String("abc".into()));
        vm.compile("return test2;").unwrap();
        let result = vm.interpret().unwrap();
        assert_eq!(result, ReturnValue::Number(123.0));
        vm.compile("return test3;").unwrap();
        let result = vm.interpret().unwrap();
        assert_eq!(result, ReturnValue::Boolean(true));
    }

    #[test]
    fn test_inject_instance() {
        let mut vm = Vm::new();
        vm.inject_instance("request", {
            let mut map = HashMap::new();
            map.insert("method", "get".into());
            map.insert("code", 200.0.into());
            map.insert("test", true.into());
            map
        });
        vm.compile("return request;").unwrap();
        let result = vm.interpret().unwrap();
        let request = result.as_object().unwrap();
        assert_eq!(request.get("method").unwrap(), "get");
        assert_eq!(request.get("code").unwrap(), 200.0);
        assert_eq!(request.get("test").unwrap(), true);
        assert!(request.get("abc").is_none());
    }
}
