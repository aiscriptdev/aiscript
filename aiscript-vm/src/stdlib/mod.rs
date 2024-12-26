mod db;
mod env;
mod io;
mod math;
mod random;
mod serde;
mod time;

pub use db::create_pg_module;
pub use db::create_redis_module;
pub use db::create_sqlite_module;
pub use env::create_env_module;
pub use io::create_io_module;
pub use math::create_math_module;
pub use random::create_random_module;
pub use serde::create_serde_module;
pub use time::create_time_module;

/// Macro to get and validate a float argument from a slice of Values
///
/// # Arguments
/// * `args` - The vector of arguments
/// * `index` - The index of the argument to get
/// * `fn_name` - The name of the function for error messages
#[macro_export]
#[doc(hidden)]
macro_rules! float_arg {
    ($args:expr, $index:expr, $fn_name:expr) => {
        match $args.get($index) {
            Some(value) => value.as_number().map_err(|_| {
                VmError::RuntimeError(format!(
                    "{}: argument {} must be a number",
                    $fn_name,
                    $index + 1
                ))
            }),
            None => Err(VmError::RuntimeError(format!(
                "{}: expected {} arguments, got {}",
                $fn_name,
                $index + 1,
                $args.len()
            ))),
        }
    };
}

/// Macro to get and validate a string argument from a slice of Values
#[macro_export]
#[doc(hidden)]
macro_rules! string_arg {
    ($args:expr, $index:expr, $fn_name:expr) => {
        match $args.get($index) {
            Some(value) => value.as_string().map_err(|_| {
                VmError::RuntimeError(format!(
                    "{}: argument {} must be a string",
                    $fn_name,
                    $index + 1
                ))
            }),
            None => Err(VmError::RuntimeError(format!(
                "{}: expected {} arguments, got {}",
                $fn_name,
                $index + 1,
                $args.len()
            ))),
        }
    };
}
