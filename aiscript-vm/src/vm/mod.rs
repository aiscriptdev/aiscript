use std::{fmt::Display, fs, ops, path::PathBuf};

use aiscript_arena::{Arena, Mutation, Rootable, arena::CollectionPhase};
use sqlx::{PgPool, SqlitePool};
pub use state::State;

use crate::{
    ReturnValue, Value,
    ai::AiConfig,
    ast::ChunkId,
    builtins, stdlib,
    string::{InternedString, InternedStringSet},
};
use fuel::Fuel;

mod extra;
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

impl Default for Vm {
    fn default() -> Self {
        Self::new(None, None, None, None)
    }
}

pub struct Vm {
    arena: Arena<Rootable![State<'_>]>,
}

impl Vm {
    pub fn new(
        pg_connection: Option<PgPool>,
        sqlite_connection: Option<SqlitePool>,
        redis_connection: Option<redis::aio::MultiplexedConnection>,
        ai_config: Option<AiConfig>,
    ) -> Self {
        let mut vm = Vm {
            arena: Arena::<Rootable![State<'_>]>::new(|mc| {
                let mut state = State::new(mc);
                state.pg_connection = pg_connection;
                state.sqlite_connection = sqlite_connection;
                state.redis_connection = redis_connection;
                state.ai_config = ai_config;
                state
            }),
        };
        vm.init_stdlib();
        vm
    }

    pub fn run_file(&mut self, path: PathBuf) {
        match fs::read_to_string(&path) {
            Ok(source) => {
                let source: &'static str = Box::leak(source.into_boxed_str());
                if let Err(VmError::CompileError) = self.compile(source) {
                    std::process::exit(65);
                }
                if let Err(VmError::RuntimeError(err)) = self.interpret() {
                    eprintln!("{err}");
                    std::process::exit(70);
                }
            }
            Err(err) => {
                eprintln!("Failed to read file '{}': {}", path.display(), err);
                std::process::exit(1);
            }
        }
    }

    fn init_stdlib(&mut self) {
        self.arena.mutate_root(|_mc, state| {
            let ctx = state.get_context();

            state.builtin_methods.init(ctx);
            state.globals.insert(
                ctx.intern(b"ValidationError!"),
                Value::Class(builtins::create_validation_error(ctx)),
            );

            // Initialize standard library modules
            state.module_manager.register_native_module(
                ctx.intern(b"std.auth.jwt"),
                stdlib::create_jwt_module(ctx),
            );
            state
                .module_manager
                .register_native_module(ctx.intern(b"std.env"), stdlib::create_env_module(ctx));
            state
                .module_manager
                .register_native_module(ctx.intern(b"std.math"), stdlib::create_math_module(ctx));
            state
                .module_manager
                .register_native_module(ctx.intern(b"std.http"), stdlib::create_http_module(ctx));
            state
                .module_manager
                .register_native_module(ctx.intern(b"std.io"), stdlib::create_io_module(ctx));
            state
                .module_manager
                .register_native_module(ctx.intern(b"std.time"), stdlib::create_time_module(ctx));
            state.module_manager.register_native_module(
                ctx.intern(b"std.random"),
                stdlib::create_random_module(ctx),
            );
            state
                .module_manager
                .register_native_module(ctx.intern(b"std.serde"), stdlib::create_serde_module(ctx));
            state
                .module_manager
                .register_native_module(ctx.intern(b"std.db.pg"), stdlib::create_pg_module(ctx));
            state.module_manager.register_native_module(
                ctx.intern(b"std.db.sqlite"),
                stdlib::create_sqlite_module(ctx),
            );
            state.module_manager.register_native_module(
                ctx.intern(b"std.db.redis"),
                stdlib::create_redis_module(ctx),
            );
        });
    }

    pub fn compile(&mut self, source: &'static str) -> Result<(), VmError> {
        self.arena.mutate_root(|_mc, state| {
            let context = state.get_context();
            state.chunks = crate::compiler::compile(context, source)?;
            builtins::define_builtin_functions(state);
            // The script function's chunk id is always the highest chunk id.
            let script_chunk_id = state.chunks.keys().max().copied().unwrap();
            let function = state.get_chunk(script_chunk_id)?;
            state.call_function(function, &[])
        })
    }

    pub fn eval_function(
        &mut self,
        chunk_id: ChunkId,
        params: &[serde_json::Value],
    ) -> Result<ReturnValue, VmError> {
        self.arena.mutate_root(|_mc, state| {
            let ctx = state.get_context();
            let return_value = state.eval_function_with_id(
                chunk_id,
                &params
                    .iter()
                    .map(|v| Value::from_serde_value(ctx, v))
                    .collect::<Vec<_>>(),
            )?;
            Ok(ReturnValue::from(return_value))
        })
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
