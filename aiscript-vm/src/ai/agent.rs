use std::{collections::HashMap, env};

use gc_arena::{Collect, Gc};
use openai_api_rs::v1::{
    api::OpenAIClient,
    chat_completion::{self, ChatCompletionRequest, Tool, ToolCall},
    common::GPT3_5_TURBO,
    types::{self, FunctionParameters, JSONSchemaType},
};
use tokio::runtime::Handle;

use crate::{
    ast::{Expr, LiteralValue},
    string::InternedString,
    vm::{Context, State},
};

#[derive(Debug, Collect)]
#[collect(no_drop)]
pub struct Agent<'gc> {
    pub name: InternedString<'gc>,
    pub instructions: InternedString<'gc>,
    pub model: InternedString<'gc>,
    pub tools: HashMap<String, usize>,
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

struct Response<'gc> {
    agent: Option<Agent<'gc>>,
    messages: Vec<chat_completion::ChatCompletionMessage>,
}

impl<'gc> Agent<'gc> {
    pub fn new(ctx: &Context<'gc>, name: InternedString<'gc>) -> Self {
        Agent {
            name,
            instructions: InternedString::from_static(ctx, ""),
            model: InternedString::from_static(ctx, "gpt-4"),
            tools: HashMap::new(),
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
        if let Some(Expr::Array { elements, .. }) = fields.get("tools") {
            for element in elements {
                match element {
                    Expr::Variable { name, .. } => {
                        // self.tools.push(FnDef {
                        //     name: name.lexeme.to_owned(),
                        //     chunk_id: f(name.lexeme),
                        // });
                        self.tools.insert(name.lexeme.to_owned(), f(name.lexeme));
                    }
                    _ => panic!("Expected string literal"),
                }
            }
        }
        self
    }

    fn get_tools(&self) -> Vec<Tool> {
        let mut tool_calls = Vec::new();
        for (name, chunk_id) in &self.tools {
            let tool_call = Tool {
                r#type: chat_completion::ToolType::Function,
                function: types::Function {
                    name: name.clone(),
                    description: None,
                    parameters: FunctionParameters {
                        schema_type: JSONSchemaType::Object,
                        properties: None,
                        required: None,
                    },
                },
            };
            tool_calls.push(tool_call);
        }
        tool_calls
    }

    #[cfg(not(feature = "ai_test"))]
    fn handle_tool_call(
        &self,
        state: &mut State<'gc>,
        tool_calls: &Option<Vec<ToolCall>>,
    ) -> Response<'gc> {
        let mut response = Response {
            agent: None,
            messages: vec![],
        };
        // let mut messages = vec![];
        for tool_call in tool_calls.as_ref().unwrap() {
            // TODO: call tool function
            let name = tool_call.function.name.as_ref().unwrap();
            if let Some(chunk_id) = self.tools.get(name) {
                let result = state.eval_function(*chunk_id).unwrap();
                println!("{:?}", result);
                response
                    .messages
                    .push(chat_completion::ChatCompletionMessage {
                        role: chat_completion::MessageRole::tool,
                        content: chat_completion::Content::Text(String::new()),
                        name: tool_call.function.name.clone(),
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
            }
        }

        response
    }
}

#[cfg(feature = "ai_test")]
pub async fn _run_agent<'gc>(
    state: &mut State<'gc>,
    agent: Gc<'gc, Agent<'gc>>,
    message: InternedString<'gc>,
) -> String {
    format!(
        "input: {},instructions: {}, model: {}, tools: {:?}",
        message, agent.instructions, agent.model, agent.tools
    )
}

#[cfg(not(feature = "ai_test"))]
pub async fn _run_agent<'gc>(
    state: &mut State<'gc>,
    agent: Gc<'gc, Agent<'gc>>,
    message: InternedString<'gc>,
) -> String {
    // let client = OpenAIClient::new(env::var("OPENAI_API_KEY").unwrap().to_string());
    // loop {
    //     let req = ChatCompletionRequest::new(
    //         GPT3_5_TURBO.to_string(),
    //         vec![chat_completion::ChatCompletionMessage {
    //             role: chat_completion::MessageRole::user,
    //             content: chat_completion::Content::Text(message.to_string()),
    //             name: None,
    //             tool_calls: None,
    //             tool_call_id: None,
    //         }],
    //     )
    //     .tools(agent.get_tools())
    //     .tool_choice(chat_completion::ToolChoiceType::Auto)
    //     .parallel_tool_calls(true);
    //     let result = client.chat_completion(req).await.unwrap();
    //     let response = &result.choices[0].message;

    //     if response.tool_calls.is_none() {
    //         break;
    //     }

    //     let response = agent.handle_tool_call(state, &response.tool_calls);
    // }
    let chunk_id = dbg!(agent.tools.get("apply_discount").unwrap());
    let result = state.eval_function(*chunk_id).unwrap();
    format!(
        "input: {}, instructions: {}, model: {}, tools: {:?}, result: {:?}",
        message, agent.instructions, agent.model, agent.tools, result
    )
}

pub fn run_agent<'gc>(
    state: &mut State<'gc>,
    agent: Gc<'gc, Agent<'gc>>,
    message: InternedString<'gc>,
) -> String {
    if Handle::try_current().is_ok() {
        // We're in an async context, use await
        Handle::current().block_on(async { _run_agent(state, agent, message).await })
    } else {
        // We're in a sync context, create a new runtime
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async { _run_agent(state, agent, message).await })
    }
}
