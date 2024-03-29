use super::token::Token;
use crate::bulk_print;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Binary {
        left: Box<Expr>,
        right: Box<Expr>,
        op: Token,
    },
    Group {
        expr: Box<Expr>,
    },
    Unary {
        right: Box<Expr>,
        op: Token,
    },
    StringLiteral {
        token: Token,
        value: String,
    },
    FloatLiteral {
        token: Token,
        value: f64,
    },
    IntegerLiteral {
        token: Token,
        value: i64,
    },
    BoolLiteral {
        token: Token,
        payload: bool,
    },
    AtomLiteral {
        token: Token,
        value: String,
    },
    Underscore {
        token: Token,
    },
    Null {
        token: Token,
    },
    ListLiteral {
        token: Token,
        values: Vec<Box<Expr>>,
    },
    ObjectLiteral {
        token: Token,
        keys: Vec<Token>,
        values: Vec<Box<Expr>>,
    },
    Logical {
        left: Box<Expr>,
        right: Box<Expr>,
        op: Token,
    },
    Variable {
        name: Token,
    },
    Assign {
        init: bool,
        public: bool,
        mutable: bool,
        token: Token,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
        token: Token,
    },
    Get {
        instance: Box<Expr>,
        value: Box<Expr>,
        token: Token,
    },
    Set {
        instance: Box<Expr>,
        token: Token,
        value: Box<Expr>,
    },
    Func {
        public: bool,
        name: Option<Token>,
        params: Vec<Token>,
        rest: Option<Token>,
        body: Box<Expr>,
    },
    Match {
        token: Token,
        condition: Box<Expr>,
        branches: Vec<MatchBranch>,
    },
    Block {
        token: Token,
        exprs: Vec<Box<Expr>>,
    },
    Unsafe {
        token: Token,
        expr: Box<Expr>,
    },
    Shell {
        token: Token,
        expr: Box<Expr>,
    },
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchBranch {
    pub target: Box<Expr>,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallArg {
    Positional(Box<Expr>),
    Unpacking(Box<Expr>),
}

impl Expr {
    pub fn pretty_print(exprs: &Vec<Expr>) -> String {
        exprs
            .into_iter()
            .map(|expr| expr.print())
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn print(&self) -> String {
        match self {
            Expr::Binary { left, right, op } => {
                format!("({} {} {})", op.print(), left.print(), right.print())
            }
            Expr::Group { expr } => {
                format!("({})", expr.print())
            }
            Expr::Unary { right, op } => {
                format!("({} {})", op.print(), right.print())
            }
            Expr::StringLiteral { token: _, value } => {
                format!("\"{}\"", value)
            }
            Expr::IntegerLiteral { token: _, value } => {
                format!("{}", value)
            }
            Expr::FloatLiteral { token: _, value } => {
                format!("{}", value)
            }
            Expr::BoolLiteral { token: _, payload } => {
                format!("{}", payload)
            }
            Expr::AtomLiteral { token: _, value } => {
                format!(":{}", value)
            }
            Expr::Underscore { token: _ } => String::from(":_:"),
            Expr::Null { token: _ } => String::from("null"),
            Expr::ListLiteral { token: _, values } => {
                if values.len() > 0 {
                    format!("(list {})", bulk_print!(values, " "))
                } else {
                    String::from("(list)")
                }
            }
            Expr::ObjectLiteral {
                token: _,
                keys,
                values,
            } => {
                if keys.len() > 0 {
                    format!(
                        "(object {})",
                        keys.into_iter()
                            .zip(values.into_iter())
                            .map(|(k, v)| format!("{}:{}", k.print(), v.print()))
                            .collect::<Vec<String>>()
                            .join(" ")
                    )
                } else {
                    String::from("(object)")
                }
            }
            Expr::Logical { left, right, op } => {
                format!("({} {} {})", op.print(), left.print(), right.print())
            }
            Expr::Variable { name } => {
                format!("{}", name.print())
            }
            Expr::Assign {
                token: _,
                left,
                right,
                init,
                public,
                mutable,
            } => {
                format!(
                    "(assign{}{}{} {} {})",
                    if *public { "P" } else { "" },
                    if *init { "I" } else { "" },
                    if *mutable { "M" } else { "" },
                    left.print(),
                    right.print()
                )
            }
            Expr::Call {
                callee,
                args,
                token: _,
            } => {
                let mut builder = format!("({}", callee.print());
                if args.len() > 0 {
                    builder += &format!(
                        " {})",
                        args.into_iter()
                            .map(|arg| match arg {
                                CallArg::Positional(expr) => expr.print(),
                                CallArg::Unpacking(expr) => format!("...{}", expr.print()),
                            })
                            .collect::<Vec<String>>()
                            .join(" "),
                    );
                } else {
                    builder += ")";
                }
                builder
            }
            Expr::Get {
                instance,
                value,
                token: _,
            } => {
                format!("{}.{}", instance.print(), value.print())
            }
            Expr::Set {
                instance,
                token,
                value,
            } => {
                format!(
                    "(set {}.{} {})",
                    instance.print(),
                    token.print(),
                    value.print()
                )
            }
            Expr::Func {
                public,
                name,
                params,
                rest,
                body,
            } => {
                if let Some(name) = name {
                    format!(
                        "(func{} {} ({}{}) {})",
                        if *public {
                            " [public]".to_string()
                        } else {
                            "".to_string()
                        },
                        name.print(),
                        bulk_print!(params, " "),
                        match rest {
                            Some(rest) =>
                                if params.len() > 0 {
                                    format!(" {}+", rest.print())
                                } else {
                                    format!("{}+", rest.print())
                                },
                            None => "".to_string(),
                        },
                        body.print()
                    )
                } else {
                    format!("(lambda ({}) {})", bulk_print!(params, " "), body.print(),)
                }
            }
            Expr::Match {
                token: _,
                condition,
                branches,
            } => {
                let mut builder = format!("(match {}", condition.print());
                if branches.len() > 0 {
                    builder += " ";
                    builder += &branches
                        .into_iter()
                        .map(|x| format!("{} -> {}", x.target.print(), x.body.print()))
                        .collect::<Vec<String>>()
                        .join(" ");
                    builder += ")";
                } else {
                    builder += ")";
                }
                builder
            }
            Expr::Block { token: _, exprs } => {
                format!("(block{})", {
                    let expr = bulk_print!(exprs, " ");
                    if expr == "" {
                        String::new()
                    } else {
                        String::from(" ") + &expr
                    }
                })
            }
            Expr::Unsafe { token: _, expr } => {
                format!("(unsafe {})", expr.print())
            }
            Expr::Shell { token: _, expr } => {
                format!("(shell_op {})", expr.print())
            }
            Expr::End => "".to_string(),
        }
    }
}
