use crate::{Value, VmError, string::InternedString};
use ahash::AHasher;
use aiscript_arena::Collect;
use std::hash::BuildHasherDefault;
use std::{collections::HashMap, path::PathBuf};

pub enum ModuleSource {
    Cached,
    New { source: String, path: PathBuf },
}

#[derive(Collect)]
#[collect(no_drop)]
pub enum ModuleKind<'gc> {
    Script {
        name: InternedString<'gc>,
        exports: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
        globals: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
        path: PathBuf,
    },
    Native {
        name: InternedString<'gc>,
        exports: HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>,
    },
}

impl<'gc> ModuleKind<'gc> {
    pub fn name(&self) -> InternedString<'gc> {
        match self {
            ModuleKind::Script { name, .. } => *name,
            ModuleKind::Native { name, .. } => *name,
        }
    }

    pub fn get_export(&self, name: InternedString<'gc>) -> Option<Value<'gc>> {
        match self {
            ModuleKind::Script { exports, .. } => exports.get(&name).copied(),
            ModuleKind::Native { exports, .. } => exports.get(&name).copied(),
        }
    }

    pub fn add_export(&mut self, name: InternedString<'gc>, value: Value<'gc>) {
        match self {
            ModuleKind::Script { exports, .. } => {
                exports.insert(name, value);
            }
            ModuleKind::Native { exports, .. } => {
                exports.insert(name, value);
            }
        }
    }

    #[cfg(feature = "debug")]
    pub fn debug_info(&self) -> String {
        match self {
            ModuleKind::Script { name, path, .. } => {
                format!("Script Module '{}' from {:?}", name, path)
            }
            ModuleKind::Native { name, .. } => {
                format!("Native Module '{}'", name)
            }
        }
    }
}

#[derive(Collect)]
#[collect(no_drop)]
pub struct ModuleManager<'gc> {
    pub modules: HashMap<InternedString<'gc>, ModuleKind<'gc>>,
    search_paths: Vec<PathBuf>,
}

impl Default for ModuleManager<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'gc> ModuleManager<'gc> {
    pub fn new() -> Self {
        ModuleManager {
            modules: HashMap::new(),
            search_paths: vec![PathBuf::from(".")], // Current directory by default
        }
    }

    #[allow(unused)]
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    pub fn get_or_load_module(
        &mut self,
        name: InternedString<'gc>,
    ) -> Result<ModuleSource, VmError> {
        // Return early if module is already loaded
        if self.modules.contains_key(&name) {
            return Ok(ModuleSource::Cached);
        }

        let module_name = name.to_str().unwrap();

        if module_name.starts_with("std.") {
            // This is a standard library module, it should be registered during VM initialization
            // If we reach here, it means the module wasn't registered
            return Err(VmError::RuntimeError(format!(
                "Standard library module '{}' not found.",
                module_name
            )));
        }

        // For user script modules, find and load the source
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

    pub fn register_native_module(&mut self, name: InternedString<'gc>, module: ModuleKind<'gc>) {
        if !name.to_str().unwrap().starts_with("std.") {
            panic!("Native modules must have names starting with 'std.'");
        }
        #[cfg(feature = "debug")]
        println!("Registering native module {}", module.debug_info());
        self.modules.insert(name, module);
    }

    pub fn register_script_module(&mut self, name: InternedString<'gc>, module: ModuleKind<'gc>) {
        if name.to_str().unwrap().starts_with("std.") {
            panic!("Script modules cannot have names starting with 'std.'");
        }
        #[cfg(feature = "debug")]
        println!("Registering script module {}", module.debug_info());
        self.modules.insert(name, module);
    }

    pub fn get_module(&self, name: InternedString<'gc>) -> Option<&ModuleKind<'gc>> {
        self.modules.get(&name)
    }

    pub fn get_export(
        &self,
        module_name: InternedString<'gc>,
        export_name: InternedString<'gc>,
    ) -> Option<Value<'gc>> {
        self.modules.get(&module_name).and_then(|m| {
            let value = m.get_export(export_name);
            #[cfg(feature = "debug")]
            if value.is_none() {
                println!("Export '{}' not found in {}", export_name, m.debug_info());
            }
            value
        })
    }
}
