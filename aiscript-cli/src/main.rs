use std::{env::args, fs, process::exit};

use aiscript_vm::{Vm, VmError};

fn main() {
    let mut args = args();
    if args.len() == 1 {
        // repl
    } else if args.len() == 2 {
        let path = args.nth(1).unwrap();
        run_file(&path);
    } else {
        println!("Usage: aiscript [path]");
        exit(64);
    }
}

fn run_file(path: &str) {
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
