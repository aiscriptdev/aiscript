use std::{fs, path::PathBuf, process::exit};

use aiscript_vm::{Vm, VmError};

use clap::{Parser, Subcommand};
use tokio::runtime::Runtime;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct AIScriptCli {
    /// Sets a custom config file
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
    /// Subcommands
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the web server.
    Serve {
        /// The file to run.
        #[arg(value_name = "FILE")]
        file: PathBuf,
        /// The web server listening port.
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
}

// #[tokio::main]
fn main() {
    dotenv::dotenv().ok();
    let cli = AIScriptCli::parse();
    match cli.command {
        Some(Commands::Serve { file, port }) => {
            println!("Server listening on port {}", port);
            Runtime::new().unwrap().block_on(async {
                aiscript_runtime::run(file, port).await;
            });
        }
        None => {
            if let Some(path) = cli.file {
                run_file(path);
            } else {
                // Run the repl
                println!("Welcome to the AIScript REPL!");
            }
        }
    }
}

fn run_file(path: PathBuf) {
    let source = fs::read_to_string(path).unwrap();
    let source: &'static str = Box::leak(source.into_boxed_str());
    let mut vm = Vm::new();
    if let Err(err) = vm.interpret(source) {
        match err {
            VmError::CompileError => exit(65),
            VmError::RuntimeError(err) => {
                eprintln!("{err}");
                exit(70)
            }
        }
    }
}
