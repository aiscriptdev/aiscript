use std::{path::PathBuf, process};

use aiscript_runtime::Config;
use aiscript_vm::Vm;

use clap::{Parser, Subcommand};
use repr::Repl;
use tokio::task;

mod project;
mod repr;

use project::ProjectGenerator;

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
        file: Option<PathBuf>,
        /// The web server listening port.
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
        /// Reload the file on change
        #[arg(short, long, default_value_t = false)]
        reload: bool,
    },
    /// Create a new AIScript project with a standard directory structure.
    New {
        /// The name of the new project
        #[arg(value_name = "PROJECT_NAME")]
        name: String,
    },
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    Config::load("project.toml");

    let cli = AIScriptCli::parse();
    match cli.command {
        Some(Commands::Serve { file, port, reload }) => {
            println!("Server listening on port http://localhost:{}", port);
            aiscript_runtime::run(file, port, reload).await;
        }
        Some(Commands::New { name }) => {
            let generator = ProjectGenerator::new(&name);
            if let Err(e) = generator.generate() {
                eprintln!("{}", e);
                process::exit(1);
            }
        }
        None => {
            if let Some(path) = cli.file {
                let pg_connection = aiscript_runtime::get_pg_connection().await;
                let sqlite_connection = aiscript_runtime::get_sqlite_connection().await;
                let redis_connection = aiscript_runtime::get_redis_connection().await;
                task::spawn_blocking(move || {
                    let mut vm = Vm::new(pg_connection, sqlite_connection, redis_connection);
                    vm.run_file(path);
                })
                .await // must use await to wait for the thread to finish
                .unwrap();
            } else {
                // Run the repl
                let mut repl = Repl::new();
                if let Err(e) = repl.run() {
                    eprintln!("Error: {}", e);
                    process::exit(1);
                }
            }
        }
    }
}
