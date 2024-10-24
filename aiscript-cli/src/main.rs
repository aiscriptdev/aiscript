use std::{fs, path::PathBuf, process::exit};

use aiscript_vm::{Vm, VmError};

use clap::{Parser, Subcommand};

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
        /// The web server listening port.
        #[arg(short, long, default_value_t = 8000)]
        port: u16,
    },
}

#[tokio::main]
async fn main() {
    let cli = AIScriptCli::parse();
    match cli.command {
        Some(Commands::Serve { port }) => {
            println!("Server listening on port {}", port);
            aiscript_runtime::run(port).await;
        }
        None => {
            if let Some(path) = cli.file {
                run_file(path);
                return;
            } else {
                // Run the repl
                println!("Welcome to the AIScript REPL!");
                return;
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
