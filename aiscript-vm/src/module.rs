use ahash::AHasher;
use gc_arena::Collect;

use crate::{string::InternedString, vm::VmError, Value};
use std::{collections::HashMap, hash::BuildHasherDefault, path::PathBuf};

#[derive(Debug, Collect)]
#[collect(no_drop)]
pub struct Module<'gc> {
    pub name: InternedString<'gc>,
    pub exports: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
    pub globals: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
    pub path: PathBuf,
}

pub enum ModuleSource {
    Cached,
    New { source: String, path: PathBuf },
}

pub struct ModuleManager<'gc> {
    pub modules: HashMap<InternedString<'gc>, Module<'gc>>,
    search_paths: Vec<PathBuf>,
}

impl<'gc> Module<'gc> {
    pub fn new(name: InternedString<'gc>, path: PathBuf) -> Self {
        Module {
            name,
            exports: HashMap::default(),
            globals: HashMap::default(),
            path,
        }
    }

    #[cfg(feature = "debug")]
    pub fn debug_info(&self) -> String {
        format!("Module '{}' from {:?}", self.name, self.path)
    }
}

impl<'gc> ModuleManager<'gc> {
    pub fn new() -> Self {
        ModuleManager {
            modules: HashMap::new(),
            search_paths: vec![PathBuf::from(".")], // Current directory by default
        }
    }

    // pub fn add_search_path(&mut self, path: PathBuf) {
    //     self.search_paths.push(path);
    // }

    pub fn get_or_load_module(
        &mut self,
        name: InternedString<'gc>,
    ) -> Result<ModuleSource, VmError> {
        // Return early if module is already loaded
        if self.modules.contains_key(&name) {
            return Ok(ModuleSource::Cached);
        }

        // Find and read module file
        let module_path = self.find_module_file(&name)?;
        let source = std::fs::read_to_string(&module_path)
            .map_err(|e| VmError::RuntimeError(format!("Failed to read module: {}", e)))?;

        Ok(ModuleSource::New {
            source,
            path: module_path,
        })
    }

    fn find_module_file(&self, name: &InternedString) -> Result<PathBuf, VmError> {
        let module_name = name.to_str().unwrap();
        let file_name = format!("{}.ai", module_name);

        for search_path in &self.search_paths {
            let full_path = search_path.join(&file_name);
            if full_path.exists() {
                return Ok(full_path);
            }
        }

        Err(VmError::RuntimeError(format!(
            "Could not find module '{}'.",
            module_name
        )))
    }

    pub fn register_module(&mut self, name: InternedString<'gc>, module: Module<'gc>) {
        #[cfg(feature = "debug")]
        println!("Registering {}", module.debug_info());
        self.modules.insert(name, module);
    }

    pub fn get_export(
        &self,
        module_name: InternedString<'gc>,
        export_name: InternedString<'gc>,
    ) -> Option<Value<'gc>> {
        self.modules.get(&module_name).and_then(|m| {
            let value = m.exports.get(&export_name).copied();
            #[cfg(feature = "debug")]
            if value.is_none() {
                println!("Export '{}' not found in {}", export_name, m.debug_info());
            }
            value
        })
    }
}
