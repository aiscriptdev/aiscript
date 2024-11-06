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
    pub tools: Vec<usize>,
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
    pub fn new(ctx: &Context<'gc>, name: InternedString<'gc>) -> Agent<'gc> {
        let instructions = InternedString::from_static(ctx, "test");
        let model = InternedString::from_static(ctx, "gpt-4");
        let tools = vec![];

        Agent {
            name,
            instructions,
            model,
            tools,
            tool_choice: ToolChoice::Auto,
        }
    }

    pub fn parse_instructions(mut self, fields: &HashMap<&'gc str, Expr<'gc>>) -> Self {
        if let Some(Expr::Literal {
            value: LiteralValue::String(value),
            ..
        }) = fields.get("instructions")
        {
            self.instructions = *value;
        }
        self
    }

    pub fn parse_model(mut self, fields: &HashMap<&'gc str, Expr<'gc>>) -> Self {
        if let Some(Expr::Literal {
            value: LiteralValue::String(value),
            ..
        }) = fields.get("model")
        {
            self.model = *value;
        }
        self
    }

    pub fn parse_tools<F>(mut self, fields: &HashMap<&'gc str, Expr<'gc>>, mut f: F) -> Self
    where
        F: FnMut(&'gc str) -> usize,
    {
        if let Some(Expr::Array { elements, .. }) = dbg!(fields.get("tools")) {
            for element in elements {
                match element {
                    Expr::Variable { name, .. } => {
                        self.tools.push(f(name.lexeme));
                    }
                    _ => panic!("Expected string literal"),
                }
            }
        }
        self
    }
}

pub fn run_agent<'gc>(agent: Gc<'gc, Agent<'gc>>, input: InternedString<'gc>) -> String {
    format!(
        "input: {},instructions: {}, model: {}, tools: {:?}",
        input, agent.instructions, agent.model, agent.tools
    )
}
