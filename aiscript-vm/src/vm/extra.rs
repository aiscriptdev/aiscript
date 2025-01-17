use std::collections::HashMap;

use gc_arena::{Gc, RefLock};

use crate::{object::Instance, Value};

use super::Vm;

impl Vm {
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
}
