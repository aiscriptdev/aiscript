use std::collections::HashMap;

use gc_arena::{Gc, RefLock};

use crate::{
    builtins::response,
    object::{Instance, Object},
    NativeFn, ReturnValue, Value,
};

use super::Vm;

impl Vm {
    pub fn get_global(&mut self, name: &'static str) -> Option<ReturnValue> {
        self.arena.mutate_root(|_mc, state| {
            let name = state.intern_static(name);
            state.globals.get(&name).copied().map(ReturnValue::from)
        })
    }

    pub fn register_extra_native_functions(&mut self) {
        self.arena.mutate_root(|_mc, state| {
            state.define_native_function("response", NativeFn(response::response));
            state.define_native_function(
                "temporary_redirect",
                NativeFn(response::temporary_redirect),
            );
            state.define_native_function(
                "permanent_redirect",
                NativeFn(response::permanent_redirect),
            );
        });
    }

    pub fn inject_sso_instance<K>(&mut self, fields: HashMap<K, serde_json::Value>)
    where
        K: AsRef<str> + Eq,
    {
        self.arena.mutate_root(|mc, state| {
            let ctx = state.get_context();
            let name = state.intern_static("sso");
            let class = crate::builtins::sso::create_sso_provider_class(ctx);
            let mut instance = Instance::new(class);
            for (key, value) in fields {
                instance.fields.insert(
                    state.intern(key.as_ref().as_bytes()),
                    Value::from_serde_value(ctx, &value),
                );
            }
            state
                .globals
                .insert(name, Gc::new(mc, RefLock::new(instance)).into());
        });
    }

    pub fn inject_variables(&mut self, variables: HashMap<String, serde_json::Value>) {
        self.arena.mutate_root(|_mc, state| {
            let ctx = state.get_context();
            for (key, value) in variables {
                let name = state.intern(key.as_bytes());
                state
                    .globals
                    .insert(name, Value::from_serde_value(ctx, &value));
            }
        });
    }

    pub fn inject_object<K>(&mut self, name: &'static str, fields: HashMap<K, serde_json::Value>)
    where
        K: AsRef<str> + Eq,
    {
        self.arena.mutate_root(|mc, state| {
            let ctx = state.get_context();
            let name = state.intern_static(name);
            let mut obj = Object::default();
            for (key, value) in fields {
                obj.fields.insert(
                    state.intern(key.as_ref().as_bytes()),
                    Value::from_serde_value(ctx, &value),
                );
            }
            state
                .globals
                .insert(name, Value::Object(Gc::new(mc, RefLock::new(obj))));
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::ReturnValue;

    use super::*;

    #[test]
    fn test_inject_variables() {
        let mut vm = Vm::default();
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
        let mut vm = Vm::default();
        vm.inject_object("request", {
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
