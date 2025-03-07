use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::process::Command;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run with debug feature enabled
    Debug { file: String },
    /// Run with optimizer feature enabled
    Optimizer { file: String },
    /// Run with all features enabled
    All { file: String },
    /// Run test suite
    Test,
}

fn run_interpreter(features: &[&str], file: &str) -> Result<()> {
    let features_arg = features.join(",");
    let status = Command::new("cargo")
        .args(["run", "--features", &features_arg, "--", file])
        .status()
        .context("Failed to execute cargo run")?;

    if !status.success() {
        anyhow::bail!("Interpreter execution failed");
    }
    Ok(())
}

fn run_tests() -> Result<()> {
    let status = Command::new("cargo")
        .args([
            "build",
            "--features",
            "ai_test",
            "--bin",
            "aiscript-test",
        ])
        .status()
        .context("Failed to build with ai_test feature")?;

    if !status.success() {
        anyhow::bail!("Build failed");
    }

    let status = Command::new("cargo")
        .args(["test"])
        .status()
        .context("Failed to run tests")?;

    if !status.success() {
        anyhow::bail!("Tests failed");
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Debug { file } => {
            run_interpreter(&["debug"], &file)?;
        }
        Commands::Optimizer { file } => {
            run_interpreter(&["optimizer"], &file)?;
        }
        Commands::All { file } => {
            run_interpreter(&["all"], &file)?;
        }
        Commands::Test => {
            run_tests()?;
        }
    }

    Ok(())
}
