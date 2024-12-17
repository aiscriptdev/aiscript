use std::collections::HashMap;

use gc_arena::{Collect, Gc};
use openai_api_rs::v1::types::JSONSchemaType;
#[cfg(not(feature = "ai_test"))]
use openai_api_rs::v1::{
    chat_completion::{
        ChatCompletionMessage, ChatCompletionMessageForResponse, ChatCompletionRequest, Content,
        MessageRole, Tool, ToolCall, ToolChoiceType, ToolType,
    },
    common::GPT3_5_TURBO,
    types::{self, FunctionParameters, JSONSchemaDefine},
};
use tokio::runtime::Handle;

use crate::{
    ast::{Expr, FnDef, Literal},
    lexer::Token,
    object::{Function, Object, Parameter},
    string::InternedString,
    ty::PrimitiveType,
    vm::{Context, State},
    Chunk, Value,
};

#[derive(Collect)]
#[collect(no_drop)]
pub struct Agent<'gc> {
    pub name: InternedString<'gc>,
    pub instructions: InternedString<'gc>,
    pub model: InternedString<'gc>,
    pub tools: HashMap<String, FnDef>,
    pub tool_choice: ToolChoice,
    pub methods: HashMap<InternedString<'gc>, Gc<'gc, Function<'gc>>>,
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

#[cfg(not(feature = "ai_test"))]
#[derive(Default)]
struct Response<'gc> {
    agent: Option<Gc<'gc, Agent<'gc>>>,
    messages: Vec<ChatCompletionMessage>,
}

fn make_response_object<'gc>(
    state: &mut State<'gc>,
    agent: Gc<'gc, Agent<'gc>>,
    message: String,
) -> Value<'gc> {
    let fields = [
        (state.intern_static("agent"), Value::Agent(agent)),
        (
            state.intern_static("message"),
            Value::String(state.intern(message.as_bytes())),
        ),
    ]
    .into_iter()
    .collect();
    Value::Object(state.gc_ref(Object { fields }))
}

fn agent_methods<'gc>(ctx: &Context<'gc>) -> HashMap<InternedString<'gc>, Gc<'gc, Function<'gc>>> {
    [(
        InternedString::from_static(ctx, "run"),
        Gc::new(
            ctx,
            Function {
                arity: 1,
                max_arity: 2,
                params: [("input", Value::Nil), ("debug", Value::Boolean(false))]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, default))| {
                        (
                            InternedString::from_static(ctx, name),
                            Parameter::new(i as u8, default),
                        )
                    })
                    .collect(),
                chunk: Chunk::new(),
                name: None,
                upvalues: Vec::new(),
            },
        ),
    )]
    .into_iter()
    .collect()
}

impl<'gc> Agent<'gc> {
    pub fn new(ctx: &Context<'gc>, name: InternedString<'gc>) -> Self {
        Agent {
            name,
            instructions: InternedString::from_static(ctx, ""),
            model: InternedString::from_static(ctx, "gpt-4"),
            tools: HashMap::new(),
            tool_choice: ToolChoice::Auto,
            methods: agent_methods(ctx),
        }
    }

    pub fn parse_instructions(mut self, fields: &HashMap<&'gc str, Expr<'gc>>) -> Self {
        if let Some(Expr::Literal {
            value: Literal::String(value),
            ..
        }) = fields.get("instructions")
        {
            self.instructions = *value;
        }
        self
    }

    pub fn parse_model(mut self, fields: &HashMap<&'gc str, Expr<'gc>>) -> Self {
        if let Some(Expr::Literal {
            value: Literal::String(value),
            ..
        }) = fields.get("model")
        {
            self.model = *value;
        }
        self
    }

    pub fn parse_tools<F>(mut self, fields: &HashMap<&'gc str, Expr<'gc>>, mut f: F) -> Self
    where
        F: FnMut(&Token<'gc>) -> Option<FnDef>,
    {
        if let Some(Expr::Array { elements, .. }) = fields.get("tools") {
            for element in elements {
                match element {
                    Expr::Variable { name, .. } => {
                        if let Some(fn_def) = f(name) {
                            self.tools.insert(name.lexeme.to_owned(), fn_def);
                        }
                    }
                    _ => panic!("Expected string literal"),
                }
            }
        }
        self
    }
}

#[cfg(not(feature = "ai_test"))]
impl<'gc> Agent<'gc> {
    fn get_instruction_message(&self) -> ChatCompletionMessage {
        ChatCompletionMessage {
            role: MessageRole::system,
            content: Content::Text(self.instructions.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn get_tools(&self) -> Vec<Tool> {
        let mut tool_calls = Vec::new();
        for (name, fn_def) in &self.tools {
            let properties = fn_def
                .params
                .iter()
                .map(|(name, ty)| {
                    (
                        name.to_owned(),
                        Box::new(JSONSchemaDefine {
                            schema_type: Some(JSONSchemaType::from(*ty)),
                            ..Default::default()
                        }),
                    )
                })
                .collect();
            let tool_call = Tool {
                r#type: ToolType::Function,
                function: types::Function {
                    name: name.clone(),
                    description: Some(fn_def.doc.clone()),
                    parameters: FunctionParameters {
                        schema_type: JSONSchemaType::Object,
                        properties: Some(properties),
                        required: Some(fn_def.params.keys().cloned().collect()),
                    },
                },
            };
            tool_calls.push(tool_call);
        }
        tool_calls
    }

    fn handle_tool_call(
        &self,
        state: &mut State<'gc>,
        tool_calls: &Option<Vec<ToolCall>>,
    ) -> Result<Response<'gc>, String> {
        let mut response = Response::default();
        for tool_call in tool_calls.as_ref().unwrap() {
            let name = tool_call.function.name.as_ref().unwrap();
            if let Some(tool_def) = self.tools.get(name) {
                let arguments = serde_json::from_str::<serde_json::Value>(
                    tool_call.function.arguments.as_ref().unwrap(),
                )
                .unwrap();
                // Pass params as positional arguments
                let params = tool_def
                    .params
                    .keys()
                    .filter_map(|key| {
                        arguments
                            .get(key)
                            .map(|v| Value::String(state.intern(v.as_str().unwrap().as_bytes())))
                    })
                    .collect::<Vec<_>>();
                let result = state
                    .eval_function_with_id(tool_def.chunk_id, &params)
                    .unwrap();
                // println!(
                //     "call tool {name} params: {:?}, result: {}",
                //     params.iter().map(|i| format!("{i}")).collect::<Vec<_>>(),
                //     result
                // );
                let content = if let Value::Agent(agent) = result {
                    let agent_name = agent.name;
                    response.agent = state.get_global(agent_name).map(|v| v.as_agent().unwrap());
                    format!("{{\"assistant\": {}}}", agent_name)
                } else {
                    result.to_string()
                };
                response.messages.push(ChatCompletionMessage {
                    role: MessageRole::tool,
                    content: Content::Text(content),
                    name: tool_call.function.name.clone(),
                    tool_calls: None,
                    tool_call_id: Some(tool_call.id.clone()),
                });
            } else {
                return Err(format!("Warning: unknow tool function: {name}"));
            }
        }

        Ok(response)
    }
}

#[cfg(feature = "ai_test")]
pub async fn _run_agent<'gc>(
    state: &mut State<'gc>,
    agent: Gc<'gc, Agent<'gc>>,
    args: Vec<Value<'gc>>,
) -> Value<'gc> {
    let message = args[0];
    let debug = args[1].as_boolean();
    println!("debug: {debug}");
    make_response_object(
        state,
        agent,
        format!(
            "input: {},instructions: {}, model: {}, tools: {:?}",
            message, agent.instructions, agent.model, agent.tools
        ),
    )
}

#[cfg(not(feature = "ai_test"))]
pub async fn _run_agent<'gc>(
    state: &mut State<'gc>,
    mut agent: Gc<'gc, Agent<'gc>>,
    args: Vec<Value<'gc>>,
) -> Value<'gc> {
    let message = args[0];
    let debug = args[1].as_boolean();
    println!("debug: {debug}");
    let mut history = Vec::new();
    history.push(ChatCompletionMessage {
        role: MessageRole::user,
        content: Content::Text(message.to_string()),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    });
    let client = super::openai_client();
    loop {
        let mut messages = vec![agent.get_instruction_message()];
        messages.extend(history.clone());
        let mut req = ChatCompletionRequest::new(GPT3_5_TURBO.to_string(), messages);
        let tools = agent.get_tools();
        if !tools.is_empty() {
            req = req
                .tools(agent.get_tools())
                .tool_choice(ToolChoiceType::Auto)
                .parallel_tool_calls(true);
        }
        if debug {
            println!("Request: {}", serde_json::to_string(&req).unwrap());
        }
        let result = client.chat_completion(req).await.unwrap();
        let response = &result.choices[0].message;
        if debug {
            println!("Response: {}", serde_json::to_string(&response).unwrap());
        }
        history.push(convert_chat_response_message(response.clone()));
        if response.tool_calls.is_none() {
            return make_response_object(
                state,
                agent,
                response.content.clone().unwrap_or_default(),
            );
        } else {
            match agent.handle_tool_call(state, &response.tool_calls) {
                Ok(response) => {
                    if let Some(handoff_agent) = response.agent {
                        agent = handoff_agent;
                    }
                    // if debug {
                    //     println!("tool function call response: {:?}", response);
                    // }
                    history.extend(response.messages);
                }
                Err(message) => return make_response_object(state, agent, message),
            }
        }
    }
}

pub fn run_agent<'gc>(
    state: &mut State<'gc>,
    agent: Gc<'gc, Agent<'gc>>,
    args: Vec<Value<'gc>>,
) -> Value<'gc> {
    if Handle::try_current().is_ok() {
        // We're in an async context, use await
        Handle::current().block_on(async { _run_agent(state, agent, args).await })
    } else {
        // We're in a sync context, create a new runtime
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async { _run_agent(state, agent, args).await })
    }
}

impl From<PrimitiveType> for JSONSchemaType {
    fn from(ty: PrimitiveType) -> Self {
        match ty {
            PrimitiveType::Int | PrimitiveType::Float => JSONSchemaType::Number,
            PrimitiveType::Bool => JSONSchemaType::Boolean,
            _ => JSONSchemaType::String,
        }
    }
}

#[cfg(not(feature = "ai_test"))]
fn convert_chat_response_message(m: ChatCompletionMessageForResponse) -> ChatCompletionMessage {
    ChatCompletionMessage {
        role: m.role,
        content: Content::Text(m.content.unwrap_or_default()),
        name: m.name,
        tool_calls: m.tool_calls,
        tool_call_id: None,
    }
}
