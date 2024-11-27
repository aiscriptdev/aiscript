use aiscript_vm::{ReturnValue, Vm, VmError};
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{CompletionType, Config, EditMode, Editor};
use std::path::PathBuf;

pub struct Repl {
    vm: Vm,
    /// Buffer to store multi-line input
    buffer: String,
    /// Track brace depth for multi-line input
    brace_depth: i32,
    /// Command history editor
    editor: Editor<(), FileHistory>,
    /// Path to history file
    history_path: PathBuf,
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}

impl Repl {
    pub fn new() -> Self {
        let config = Config::builder()
            .history_ignore_space(true)
            .completion_type(CompletionType::List)
            .edit_mode(EditMode::Emacs)
            .build();

        // Create editor instance
        let mut editor = Editor::with_config(config).unwrap();

        // Setup history file path
        let history_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ai_script_history");

        // Load existing history
        if editor.load_history(&history_path).is_err() {
            println!("No previous history.");
        }

        Self {
            vm: Vm::new(),
            buffer: String::new(),
            brace_depth: 0,
            editor,
            history_path,
        }
    }

    pub fn run(&mut self) -> Result<(), VmError> {
        println!("AI Script REPL v0.1.0");
        println!("Type '.help' for more information.");

        loop {
            let prompt = if self.buffer.is_empty() { "> " } else { "... " };

            // Read line with history support
            match self.editor.readline(prompt) {
                Ok(line) => {
                    let input = line.trim();

                    // Add valid commands to history
                    if !input.is_empty() && self.buffer.is_empty() {
                        self.editor.add_history_entry(line.as_str()).unwrap();
                        self.editor
                            .save_history(&self.history_path)
                            .unwrap_or_else(|e| {
                                eprintln!("Error saving history: {}", e);
                            });
                    }

                    if input.is_empty() && !self.buffer.is_empty() {
                        // Empty line in multi-line mode - execute the buffered code
                        let code = self.buffer.clone();
                        self.buffer.clear();
                        self.brace_depth = 0;
                        self.execute_code(&code)?;
                        continue;
                    }

                    // Handle special commands
                    match input {
                        ".help" => {
                            println!("Available commands:");
                            println!("  .help    Show this help message");
                            println!("  .clear   Clear the current buffer");
                            println!("  .exit    Exit the REPL");
                            println!("\nUse arrow keys ↑/↓ to navigate history");
                            continue;
                        }
                        ".clear" => {
                            self.buffer.clear();
                            self.brace_depth = 0;
                            continue;
                        }
                        ".exit" => break,
                        _ => {}
                    }

                    // Track brace depth for multi-line input
                    for c in input.chars() {
                        match c {
                            '{' => self.brace_depth += 1,
                            '}' => self.brace_depth -= 1,
                            _ => {}
                        }
                    }

                    // Add input to buffer
                    if !self.buffer.is_empty() {
                        self.buffer.push('\n');
                    }
                    self.buffer.push_str(input);

                    // Execute if complete
                    if self.brace_depth == 0 && !input.ends_with('\\') {
                        let code = self.buffer.clone();
                        self.buffer.clear();
                        self.execute_code(&code)?;
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("CTRL-C");
                    self.buffer.clear();
                    self.brace_depth = 0;
                }
                Err(ReadlineError::Eof) => {
                    println!("CTRL-D");
                    break;
                }
                Err(err) => {
                    println!("Error: {}", err);
                    break;
                }
            }
        }

        Ok(())
    }

    fn execute_code(&mut self, code: &str) -> Result<(), VmError> {
        // Wrap the code in a function if it's an expression
        let code = if !code.contains(';') && !code.contains("fn ") && !code.contains("class ") {
            format!("return {};", code)
        } else {
            code.to_string()
        };

        match self.vm.compile(Box::leak(code.into_boxed_str())) {
            Ok(()) => {
                match self.vm.interpret() {
                    Ok(value) => {
                        // Only print non-nil values
                        if !matches!(value, ReturnValue::Nil) {
                            println!("{}", value);
                        }
                    }
                    Err(e) => eprintln!("Runtime error: {}", e),
                }
            }
            Err(e) => eprintln!("Compile error: {}", e),
        }

        Ok(())
    }
}
