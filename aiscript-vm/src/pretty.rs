use std::fmt;

use crate::ast::*;

struct PrettyPrint<'a, T> {
    value: &'a T,
    indent: usize,
}

impl<'gc> fmt::Display for Program<'gc> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Program")?;
        for stmt in &self.statements {
            write!(f, "{}", PrettyPrint::new(stmt, 1))?;
        }
        Ok(())
    }
}

impl<'gc> fmt::Display for Expr<'gc> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", PrettyPrint::new(self, 0))
    }
}

impl<'gc> fmt::Display for Stmt<'gc> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", PrettyPrint::new(self, 0))
    }
}

impl<'gc> fmt::Display for PrettyPrint<'_, Box<Expr<'gc>>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", PrettyPrint::new(self.value.as_ref(), self.indent))
    }
}

impl<'gc> fmt::Display for PrettyPrint<'_, Box<Stmt<'gc>>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", PrettyPrint::new(self.value.as_ref(), self.indent))
    }
}

impl<'a, T> PrettyPrint<'a, T> {
    fn new(value: &'a T, indent: usize) -> Self {
        Self { value, indent }
    }

    fn indent(&self) -> String {
        "  ".repeat(self.indent)
    }
}

impl<'gc> fmt::Display for PrettyPrint<'_, Expr<'gc>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indent = self.indent();
        match self.value {
            Expr::Binary {
                left,
                operator,
                right,
                line,
            } => {
                writeln!(f, "{indent}Binary [line {line}]")?;
                writeln!(
                    f,
                    "{}Operator: {:?}",
                    "  ".repeat(self.indent + 1),
                    operator
                )?;
                write!(f, "{}Left: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(left, self.indent + 2))?;
                write!(f, "{}Right: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(right, self.indent + 2))
            }
            Expr::Grouping { expression, line } => {
                writeln!(f, "{indent}Grouping [line {line}]")?;
                write!(f, "{}Expression: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(expression, self.indent + 2))
            }
            Expr::Literal { value, line } => {
                writeln!(f, "{indent}Literal [line {line}]")?;
                match value {
                    LiteralValue::Number(n) => {
                        writeln!(f, "{}Number: {}", "  ".repeat(self.indent + 1), n)
                    }
                    LiteralValue::String(s) => {
                        writeln!(f, "{}String: {:?}", "  ".repeat(self.indent + 1), s)
                    }
                    LiteralValue::Boolean(b) => {
                        writeln!(f, "{}Boolean: {}", "  ".repeat(self.indent + 1), b)
                    }
                    LiteralValue::Nil => writeln!(f, "{}Nil", "  ".repeat(self.indent + 1)),
                }
            }
            Expr::Unary {
                operator,
                right,
                line,
            } => {
                writeln!(f, "{indent}Unary [line {line}]")?;
                writeln!(
                    f,
                    "{}Operator: {:?}",
                    "  ".repeat(self.indent + 1),
                    operator
                )?;
                write!(f, "{}Right: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(right, self.indent + 2))
            }
            Expr::Variable { name, line } => {
                writeln!(f, "{indent}Variable [line {line}]")?;
                writeln!(f, "{}Name: {:?}", "  ".repeat(self.indent + 1), name)
            }
            Expr::Assign { name, value, line } => {
                writeln!(f, "{indent}Assign [line {line}]")?;
                writeln!(f, "{}Name: {:?}", "  ".repeat(self.indent + 1), name)?;
                write!(f, "{}Value: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(value, self.indent + 2))
            }
            Expr::And { left, right, line } => {
                writeln!(f, "{indent}And [line {line}]")?;
                write!(f, "{}Left: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(left, self.indent + 2))?;
                write!(f, "{}Right: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(right, self.indent + 2))
            }
            Expr::Or { left, right, line } => {
                writeln!(f, "{indent}Or [line {line}]")?;
                write!(f, "{}Left: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(left, self.indent + 2))?;
                write!(f, "{}Right: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(right, self.indent + 2))
            }
            Expr::Call {
                callee,
                arguments,
                line,
            } => {
                writeln!(f, "{indent}Call [line {line}]")?;
                write!(f, "{}Callee: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(callee, self.indent + 2))?;
                writeln!(f, "{}Arguments:", "  ".repeat(self.indent + 1))?;
                for arg in arguments {
                    write!(f, "{}", PrettyPrint::new(arg, self.indent + 2))?;
                }
                Ok(())
            }
            Expr::Invoke {
                object,
                method,
                arguments,
                line,
            } => {
                writeln!(f, "{indent}Invoke [line {line}]")?;
                write!(f, "{}Object: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(object, self.indent + 2))?;
                writeln!(f, "{}Method: {:?}", "  ".repeat(self.indent + 1), method)?;
                writeln!(f, "{}Arguments:", "  ".repeat(self.indent + 1))?;
                for arg in arguments {
                    write!(f, "{}", PrettyPrint::new(arg, self.indent + 2))?;
                }
                Ok(())
            }
            Expr::Get { object, name, line } => {
                writeln!(f, "{indent}Get [line {line}]")?;
                write!(f, "{}Object: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(object, self.indent + 2))?;
                writeln!(f, "{}Name: {:?}", "  ".repeat(self.indent + 1), name)
            }
            Expr::Set {
                object,
                name,
                value,
                line,
            } => {
                writeln!(f, "{indent}Set [line {line}]")?;
                write!(f, "{}Object: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(object, self.indent + 2))?;
                writeln!(f, "{}Name: {:?}", "  ".repeat(self.indent + 1), name)?;
                write!(f, "{}Value: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(value, self.indent + 2))
            }
            Expr::This { line } => {
                writeln!(f, "{indent}This [line {line}]")
            }
            Expr::Super { method, line } => {
                writeln!(f, "{indent}Super [line {line}]")?;
                writeln!(f, "{}Method: {:?}", "  ".repeat(self.indent + 1), method)
            }
            Expr::Prompt { expression, line } => {
                writeln!(f, "{indent}Prompt [line {line}]")?;
                write!(f, "{}Expression: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(expression, self.indent + 2))
            }
        }
    }
}

impl<'gc> fmt::Display for PrettyPrint<'_, Stmt<'gc>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indent = self.indent();
        match self.value {
            Stmt::Expression { expression, line } => {
                writeln!(f, "{indent}Expression Statement [line {line}]")?;
                write!(f, "{}", PrettyPrint::new(expression, self.indent + 1))
            }
            Stmt::Print { expression, line } => {
                writeln!(f, "{indent}Print Statement [line {line}]")?;
                write!(f, "{}", PrettyPrint::new(expression, self.indent + 1))
            }
            Stmt::Let {
                name,
                initializer,
                line,
            } => {
                writeln!(f, "{indent}Let Statement [line {line}]")?;
                writeln!(f, "{}Name: {:?}", "  ".repeat(self.indent + 1), name)?;
                if let Some(init) = initializer {
                    write!(f, "{}Initializer: ", "  ".repeat(self.indent + 1))?;
                    write!(f, "{}", PrettyPrint::new(init, self.indent + 2))?;
                }
                Ok(())
            }
            Stmt::Block { statements, line } => {
                writeln!(f, "{indent}Block Statement [line {line}]")?;
                for stmt in statements {
                    write!(f, "{}", PrettyPrint::new(stmt, self.indent + 1))?;
                }
                Ok(())
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                line,
            } => {
                writeln!(f, "{indent}If Statement [line {line}]")?;
                write!(f, "{}Condition: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(condition, self.indent + 2))?;
                write!(f, "{}Then: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(then_branch, self.indent + 2))?;
                if let Some(else_branch) = else_branch {
                    write!(f, "{}Else: ", "  ".repeat(self.indent + 1))?;
                    write!(f, "{}", PrettyPrint::new(else_branch, self.indent + 2))?;
                }
                Ok(())
            }
            Stmt::While {
                condition,
                body,
                line,
            } => {
                writeln!(f, "{indent}While Statement [line {line}]")?;
                write!(f, "{}Condition: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(condition, self.indent + 2))?;
                write!(f, "{}Body: ", "  ".repeat(self.indent + 1))?;
                write!(f, "{}", PrettyPrint::new(body, self.indent + 2))
            }
            Stmt::Function {
                name,
                params,
                body,
                is_ai,
                line,
            } => {
                writeln!(f, "{indent}Function Statement [line {line}]")?;
                writeln!(f, "{}Name: {:?}", "  ".repeat(self.indent + 1), name)?;
                writeln!(f, "{}Is AI: {}", "  ".repeat(self.indent + 1), is_ai)?;
                writeln!(f, "{}Parameters:", "  ".repeat(self.indent + 1))?;
                for param in params {
                    writeln!(f, "{}{:?}", "  ".repeat(self.indent + 2), param)?;
                }
                writeln!(f, "{}Body:", "  ".repeat(self.indent + 1))?;
                for stmt in body {
                    write!(f, "{}", PrettyPrint::new(stmt, self.indent + 2))?;
                }
                Ok(())
            }
            Stmt::Return { value, line } => {
                writeln!(f, "{indent}Return Statement [line {line}]")?;
                if let Some(value) = value {
                    write!(f, "{}Value: ", "  ".repeat(self.indent + 1))?;
                    write!(f, "{}", PrettyPrint::new(value, self.indent + 2))?;
                }
                Ok(())
            }
            Stmt::Class {
                name,
                superclass,
                methods,
                line,
            } => {
                writeln!(f, "{indent}Class Statement [line {line}]")?;
                writeln!(f, "{}Name: {:?}", "  ".repeat(self.indent + 1), name)?;
                if let Some(superclass) = superclass {
                    write!(f, "{}Superclass: ", "  ".repeat(self.indent + 1))?;
                    write!(f, "{}", PrettyPrint::new(superclass, self.indent + 2))?;
                }
                writeln!(f, "{}Methods:", "  ".repeat(self.indent + 1))?;
                for method in methods {
                    write!(f, "{}", PrettyPrint::new(method, self.indent + 2))?;
                }
                Ok(())
            }
        }
    }
}
