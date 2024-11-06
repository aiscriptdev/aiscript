use std::collections::HashMap;

use gc_arena::{Collect, Gc};

use crate::{
    ast::{Expr, LiteralValue},
    string::InternedString,
    vm::Context,
};

#[derive(Debug, Collect)]
#[collect(no_drop)]
pub struct Agent<'gc> {
    pub name: InternedString<'gc>,
    pub instructions: InternedString<'gc>,
    pub model: InternedString<'gc>,
    pub tools: Vec<InternedString<'gc>>,
    pub tool_choice: ToolChoice,
}

// Controls which (if any) tool is called by the model.
// none means the model will not call any tool and instead generates a message.
// auto means the model can pick between generating a message or calling one or more tools.
// required means the model must call one or more tools.
// Specifying a particular tool via {"type": "function", "function": {"name": "my_function"}}
// forces the model to call that tool.
#[derive(Debug, Collect)]
#[collect(no_drop)]
pub enum ToolChoice {
    None,
    Auto,
    Required,
}

impl<'gc> Agent<'gc> {
    pub fn new(
        ctx: &Context<'gc>,
        name: InternedString<'gc>,
        fields: &HashMap<InternedString<'gc>, Expr<'gc>>,
    ) -> Agent<'gc> {
        let mut instructions = InternedString::from_static(ctx, "test");
        let mut model = InternedString::from_static(ctx, "gpt-4");
        let mut tools = vec![];

        for (key, expr) in fields {
            match &*key.to_string() {
                "instructions" => {
                    instructions = match expr {
                        Expr::Literal {
                            value: LiteralValue::String(value),
                            ..
                        } => *value,
                        _ => panic!("Expected string literal"),
                    };
                }
                "model" => {
                    model = match expr {
                        Expr::Literal {
                            value: LiteralValue::String(value),
                            ..
                        } => *value,
                        _ => panic!("Expected string literal"),
                    };
                }
                "tools" => {
                    match expr {
                        Expr::Array { elements, .. } => {
                            for element in elements {
                                match element {
                                    Expr::Literal {
                                        value: LiteralValue::String(value),
                                        ..
                                    } => {
                                        tools.push(*value);
                                    }
                                    Expr::Variable { name, .. } => {
                                        let name = ctx.intern(name.lexeme.as_bytes());
                                        tools.push(name);
                                    }
                                    _ => panic!("Expected string literal"),
                                }
                            }
                        }
                        _ => panic!("Expected array"),
                    };
                }
                _ => {}
            }
        }

        Agent {
            name,
            instructions,
            model,
            tools,
            tool_choice: ToolChoice::Auto,
        }
    }
}

pub fn run_agent<'gc>(agent: Gc<'gc, Agent<'gc>>, input: InternedString<'gc>) -> String {
    format!(
        "input: {},instructions: {}, model: {}, tools: {}",
        input,
        agent.instructions,
        agent.model,
        agent
            .tools
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}
