use std::fmt::{self, Write};

use crate::ast::ObjectProperty;

use super::{ErrorHandler, Expr, Program, Stmt};

const INDENT: &str = "  ";

fn indent(level: usize) -> String {
    INDENT.repeat(level)
}

fn write_error_handler(f: &mut String, handler: &ErrorHandler<'_>, level: usize) {
    writeln!(f, "{}Error Handler:", indent(level)).unwrap();
    if handler.propagate {
        writeln!(f, "{}Propagate(?)", indent(level + 1)).unwrap();
    } else {
        writeln!(
            f,
            "{}Variable: {}",
            indent(level + 1),
            handler.error_var.lexeme
        )
        .unwrap();
        writeln!(f, "{}Body:", indent(level + 1)).unwrap();
        for stmt in &handler.handler_body {
            stmt.fmt_with_indent(f, level + 2);
        }
    }
}

impl<'gc> Program<'gc> {
    pub fn pretty_print(&self) -> String {
        let mut output = String::from("AST:\n");
        self.fmt_with_indent(&mut output, 0);
        output
    }

    fn fmt_with_indent(&self, f: &mut String, level: usize) {
        writeln!(f, "{}Program", indent(level)).unwrap();
        for stmt in &self.statements {
            stmt.fmt_with_indent(f, level + 1);
        }
    }
}

impl<'gc> Stmt<'gc> {
    fn fmt_with_indent(&self, f: &mut String, level: usize) {
        let ind = indent(level);
        match self {
            Self::Use { path, .. } => writeln!(f, "{ind}Use({})", path.lexeme).unwrap(),
            Self::Expression { expression, .. } => {
                writeln!(f, "{ind}Expr").unwrap();
                expression.fmt_with_indent(f, level + 1);
            }
            Self::Let(var) => {
                writeln!(f, "{ind}Let {}", var.name.lexeme).unwrap();
                writeln!(f, "{}Expr", indent(level + 1)).unwrap();
                if let Some(init) = &var.initializer {
                    init.fmt_with_indent(f, level + 2);
                }
            }
            Self::Block { statements, .. } => {
                writeln!(f, "{ind}Block").unwrap();
                for stmt in statements {
                    stmt.fmt_with_indent(f, level + 1);
                }
            }
            Self::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                writeln!(f, "{ind}If").unwrap();
                writeln!(f, "{}Condition:", indent(level + 1)).unwrap();
                condition.fmt_with_indent(f, level + 2);
                writeln!(f, "{}Then:", indent(level + 1)).unwrap();
                then_branch.fmt_with_indent(f, level + 2);
                if let Some(else_branch) = else_branch {
                    writeln!(f, "{}Else:", indent(level + 1)).unwrap();
                    else_branch.fmt_with_indent(f, level + 2);
                }
            }
            Self::Function(func) => {
                writeln!(f, "{ind}Function {}", func.name.lexeme).unwrap();
                writeln!(f, "{}Parameters:", indent(level + 1)).unwrap();
                for param in func.params.keys() {
                    writeln!(f, "{}{}", indent(level + 2), param.lexeme).unwrap();
                }
                if let Some(ret_type) = &func.return_type {
                    writeln!(f, "{}Return Type: {}", indent(level + 1), ret_type.lexeme).unwrap();
                }
                if !func.error_types.is_empty() {
                    writeln!(f, "{}Error Types:", indent(level + 1)).unwrap();
                    for err_type in &func.error_types {
                        writeln!(f, "{}{}", indent(level + 2), err_type.lexeme).unwrap();
                    }
                }
                writeln!(f, "{}Body:", indent(level + 1)).unwrap();
                for stmt in &func.body {
                    stmt.fmt_with_indent(f, level + 2);
                }
            }
            Self::Return { value, .. } => {
                writeln!(f, "{ind}Return").unwrap();
                if let Some(val) = value {
                    val.fmt_with_indent(f, level + 1);
                }
            }
            Self::BlockReturn { value, .. } => {
                writeln!(f, "{ind}BlockReturn").unwrap();
                value.fmt_with_indent(f, level + 1);
            }
            Self::Loop {
                condition,
                body,
                increment,
                ..
            } => {
                writeln!(f, "{ind}Loop").unwrap();
                writeln!(f, "{}Condition:", indent(level + 1)).unwrap();
                condition.fmt_with_indent(f, level + 2);
                if let Some(inc) = increment {
                    writeln!(f, "{}Increment:", indent(level + 1)).unwrap();
                    inc.fmt_with_indent(f, level + 2);
                }
                writeln!(f, "{}Body:", indent(level + 1)).unwrap();
                body.fmt_with_indent(f, level + 2);
            }
            Self::Break { .. } => writeln!(f, "{ind}Break").unwrap(),
            Self::Continue { .. } => writeln!(f, "{ind}Continue").unwrap(),
            Self::Class(class) => {
                writeln!(f, "{ind}Class {}", class.name.lexeme).unwrap();
                if let Some(superclass) = &class.superclass {
                    writeln!(f, "{}Superclass:", indent(level + 1)).unwrap();
                    superclass.fmt_with_indent(f, level + 2);
                }
                writeln!(f, "{}Methods:", indent(level + 1)).unwrap();
                for method in &class.methods {
                    method.fmt_with_indent(f, level + 2);
                }
            }
            Self::Enum(e) => {
                writeln!(f, "{ind}Enum {}", e.name.lexeme).unwrap();
                writeln!(f, "{}Variants:", indent(level + 1)).unwrap();
                for v in &e.variants {
                    writeln!(f, "{}{} = {}", indent(level + 2), v.name.lexeme, v.value).unwrap();
                }
            }
            Self::Const {
                name, initializer, ..
            } => {
                writeln!(f, "{ind}Const {}", name.lexeme).unwrap();
                initializer.fmt_with_indent(f, level + 1);
            }
            Self::Agent(agent) => {
                writeln!(f, "{ind}Agent {}", agent.name.lexeme).unwrap();
                writeln!(f, "{}Tools:", indent(level + 1)).unwrap();
                for tool in &agent.tools {
                    tool.fmt_with_indent(f, level + 2);
                }
            }
            Self::Raise { error, .. } => {
                writeln!(f, "{ind}Raise").unwrap();
                error.fmt_with_indent(f, level + 1);
            }
        }
    }
}

impl<'gc> Expr<'gc> {
    fn fmt_with_indent(&self, f: &mut String, level: usize) {
        let ind = indent(level);
        match self {
            Self::Literal { value, .. } => writeln!(f, "{ind}Literal({value})").unwrap(),
            Self::Binary {
                left,
                operator,
                right,
                ..
            } => {
                writeln!(f, "{ind}Binary {}", operator.lexeme).unwrap();
                left.fmt_with_indent(f, level + 1);
                right.fmt_with_indent(f, level + 1);
            }
            Self::Unary {
                operator, right, ..
            } => {
                writeln!(f, "{ind}Unary {}", operator.lexeme).unwrap();
                right.fmt_with_indent(f, level + 1);
            }
            Self::Variable { name, .. } => writeln!(f, "{ind}Variable({})", name.lexeme).unwrap(),
            Self::Assign { name, value, .. } => {
                writeln!(f, "{ind}Assign {}", name.lexeme).unwrap();
                value.fmt_with_indent(f, level + 1);
            }
            Self::Call {
                callee,
                arguments,
                keyword_args,
                error_handler,
                ..
            } => {
                writeln!(f, "{ind}Call").unwrap();
                writeln!(f, "{}Callee:", indent(level + 1)).unwrap();
                callee.fmt_with_indent(f, level + 2);
                if !arguments.is_empty() {
                    writeln!(f, "{}Arguments:", indent(level + 1)).unwrap();
                    for arg in arguments {
                        arg.fmt_with_indent(f, level + 2);
                    }
                }
                if !keyword_args.is_empty() {
                    writeln!(f, "{}Keyword Arguments:", indent(level + 1)).unwrap();
                    for (name, value) in keyword_args {
                        writeln!(f, "{}{}:", indent(level + 2), name).unwrap();
                        value.fmt_with_indent(f, level + 3);
                    }
                }
                if let Some(handler) = error_handler {
                    write_error_handler(f, handler, level + 1);
                }
            }
            Self::Invoke {
                object,
                method,
                arguments,
                keyword_args,
                error_handler,
                ..
            } => {
                writeln!(f, "{ind}Invoke").unwrap();
                writeln!(f, "{}Object:", indent(level + 1)).unwrap();
                writeln!(f, "{}Method({})", indent(level + 1), method.lexeme).unwrap();
                object.fmt_with_indent(f, level + 2);
                if !arguments.is_empty() {
                    writeln!(f, "{}Arguments:", indent(level + 1)).unwrap();
                    for arg in arguments {
                        arg.fmt_with_indent(f, level + 2);
                    }
                }
                if !keyword_args.is_empty() {
                    writeln!(f, "{}Keyword Arguments:", indent(level + 1)).unwrap();
                    for (name, value) in keyword_args {
                        writeln!(f, "{}{}:", indent(level + 2), name).unwrap();
                        value.fmt_with_indent(f, level + 3);
                    }
                }
                if let Some(handler) = error_handler {
                    write_error_handler(f, handler, level + 1);
                }
            }
            Self::Get { object, name, .. } => {
                writeln!(f, "{ind}Get {}", name.lexeme).unwrap();
                object.fmt_with_indent(f, level + 1);
            }
            Self::Set {
                object,
                name,
                value,
                ..
            } => {
                writeln!(f, "{ind}Set {}", name.lexeme).unwrap();
                writeln!(f, "{}Object:", indent(level + 1)).unwrap();
                object.fmt_with_indent(f, level + 2);
                writeln!(f, "{}Value:", indent(level + 1)).unwrap();
                value.fmt_with_indent(f, level + 2);
            }
            Self::Self_ { .. } => writeln!(f, "{ind}Self").unwrap(),
            Self::Super { method, .. } => writeln!(f, "{ind}Super {}", method.lexeme).unwrap(),
            Self::Array { elements, .. } => {
                writeln!(f, "{ind}Array").unwrap();
                for elem in elements {
                    elem.fmt_with_indent(f, level + 1);
                }
            }
            Self::Object { properties, .. } => {
                writeln!(f, "{ind}Object").unwrap();
                for prop in properties {
                    match prop {
                        ObjectProperty::Literal { key, value } => {
                            writeln!(f, "{}Property {}", indent(level + 1), key.lexeme).unwrap();
                            value.fmt_with_indent(f, level + 2);
                        }
                        ObjectProperty::Computed { key_expr, value } => {
                            writeln!(f, "{}Computed Property:", indent(level + 1)).unwrap();
                            key_expr.fmt_with_indent(f, level + 2);
                            writeln!(f, "{}Value:", indent(level + 1)).unwrap();
                            value.fmt_with_indent(f, level + 2);
                        }
                    }
                }
            }
            Self::Lambda { params, body, .. } => {
                writeln!(f, "{ind}Lambda").unwrap();
                writeln!(f, "{}Parameters:", indent(level + 1)).unwrap();
                for param in params {
                    writeln!(f, "{}{}", indent(level + 2), param.lexeme).unwrap();
                }
                writeln!(f, "{}Body:", indent(level + 1)).unwrap();
                body.fmt_with_indent(f, level + 2);
            }
            Self::Block { statements, .. } => {
                writeln!(f, "{ind}Block").unwrap();
                for stmt in statements {
                    stmt.fmt_with_indent(f, level + 1);
                }
            }
            Self::EnumVariant {
                enum_name, variant, ..
            } => writeln!(
                f,
                "{ind}EnumVariant {}::{}",
                enum_name.lexeme, variant.lexeme
            )
            .unwrap(),
            Self::EvaluateVariant { expr, .. } => {
                writeln!(f, "{ind}EvaluateVariant").unwrap();
                expr.fmt_with_indent(f, level + 1);
            }
            Self::Index {
                object, key, value, ..
            } => {
                if value.is_some() {
                    writeln!(f, "{ind}SetIndex").unwrap();
                } else {
                    writeln!(f, "{ind}GetIndex").unwrap();
                }
                writeln!(f, "{}Object:", indent(level + 1)).unwrap();
                object.fmt_with_indent(f, level + 2);
                writeln!(f, "{}Key:", indent(level + 1)).unwrap();
                key.fmt_with_indent(f, level + 2);
                if let Some(val) = value {
                    writeln!(f, "{}Value:", indent(level + 1)).unwrap();
                    val.fmt_with_indent(f, level + 2);
                }
            }
            Self::InlineIf {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                writeln!(f, "{ind}InlineIf").unwrap();
                writeln!(f, "{}Condition:", indent(level + 1)).unwrap();
                condition.fmt_with_indent(f, level + 2);
                writeln!(f, "{}Then:", indent(level + 1)).unwrap();
                then_branch.fmt_with_indent(f, level + 2);
                writeln!(f, "{}Else:", indent(level + 1)).unwrap();
                else_branch.fmt_with_indent(f, level + 2);
            }
            Self::Prompt { expression, .. } => {
                writeln!(f, "{ind}Prompt").unwrap();
                expression.fmt_with_indent(f, level + 1);
            }
            Self::And { left, right, .. } => {
                writeln!(f, "{ind}And").unwrap();
                left.fmt_with_indent(f, level + 1);
                right.fmt_with_indent(f, level + 1);
            }
            Self::Or { left, right, .. } => {
                writeln!(f, "{ind}Or").unwrap();
                left.fmt_with_indent(f, level + 1);
                right.fmt_with_indent(f, level + 1);
            }
            _ => writeln!(f, "{ind}...").unwrap(),
        }
    }
}

// For easier printing in debug scenarios
impl<'gc> fmt::Display for Program<'gc> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pretty_print())
    }
}
