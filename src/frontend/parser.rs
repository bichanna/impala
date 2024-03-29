use std::collections::HashMap;
use std::process;

use crossbeam_channel::{Receiver, Sender};

use super::ast::{CallArg, Expr, MatchBranch};
use super::error::ParserError;
use super::token::{Token, TokenType};

macro_rules! expect {
    ($self: expr, $kind: expr, $msg: expr) => {
        if $self.check_current($kind) {
            $self.advance();
        } else {
            return Err($msg);
        }
    };
}

pub struct Parser<'a> {
    /// The current token being parsed, received from the channel
    current: Token,
    /// The previous token received from the channel
    previous: Token,
    /// The channel that receives tokens concurrently
    recv: &'a Receiver<Token>,
    /// The channel to send AST nodes concurrently
    sender: &'a Sender<Expr>,
    /// Errors encountered while parsing
    errors: Vec<ParserError>,
    /// Macros
    macros: HashMap<String, Expr>,
}

impl<'a> Parser<'a> {
    pub fn new(
        source: &'a String,
        filename: &'a str,
        token_recv: &'a Receiver<Token>,
        output_sender: &'a Sender<Expr>,
    ) {
        let current = token_recv.recv().unwrap();
        let mut parser = Parser {
            current: current.clone(),
            previous: current.clone(),
            recv: token_recv,
            sender: output_sender,
            errors: vec![],
            macros: HashMap::new(),
        };
        parser.parse();
        parser.report_errors(filename, source);
    }

    /// Reports errors if any
    fn report_errors(&self, filename: &str, source: &String) {
        if self.errors.len() > 0 {
            for err in &self.errors {
                eprintln!("{}", err.format(filename));
                eprintln!(
                    "{}",
                    source.split("\n").collect::<Vec<&str>>()[err.line - 1]
                );
            }
            process::exit(1);
        }
    }

    /// Parses tokens to AST
    fn parse(&mut self) {
        while !self.is_end() {
            let expr = self.expression();
            match expr {
                Ok(expr) => self.sender.send(expr).unwrap(),
                Err(msg) => {
                    self.add_error(msg);
                    self.synchronize();
                }
            }
        }
        self.sender.send(Expr::End).unwrap();
    }

    fn expression(&mut self) -> Result<Expr, &'static str> {
        return self.assignment();
    }

    fn assignment(&mut self) -> Result<Expr, &'static str> {
        let expr = self.or_expr()?;
        if self.check_current(TokenType::Equal)
            || self.check_current(TokenType::ColonEq)
            || self.check_current(TokenType::PipeEq)
            || self.check_current(TokenType::DollarEq)
        {
            let init = if self.current.kind == TokenType::ColonEq
                || self.current.kind == TokenType::PipeEq
            {
                true
            } else {
                false
            };
            let public = if self.current.kind == TokenType::PipeEq {
                true
            } else {
                false
            };
            let mutable = if self.current.kind == TokenType::DollarEq {
                true
            } else {
                false
            };

            self.advance();
            let value = Box::new(self.assignment()?);

            match *value {
                Expr::Assign {
                    init: _,
                    public: _,
                    mutable: _,
                    token: _,
                    left: _,
                    right: _,
                } => return Err("assignment value should not be assignment"),
                _ => {}
            }

            // check what kind of assignment it is
            match expr {
                Expr::ObjectLiteral {
                    ref token,
                    keys: _,
                    ref values,
                } => {
                    for v in values {
                        match **v {
                            Expr::Variable { name: _ }
                            | Expr::Get {
                                instance: _,
                                value: _,
                                token: _,
                            }
                            | Expr::Underscore { token: _ } => continue,
                            _ => return Err("expected variable names or _ for object values"),
                        }
                    }
                    return Ok(Expr::Assign {
                        token: token.clone(),
                        init,
                        public,
                        mutable,
                        left: Box::new(expr),
                        right: value,
                    });
                }
                Expr::ListLiteral {
                    ref token,
                    ref values,
                } => {
                    for v in values {
                        match **v {
                            Expr::Variable { name: _ }
                            | Expr::Get {
                                instance: _,
                                value: _,
                                token: _,
                            }
                            | Expr::Underscore { token: _ } => continue,
                            _ => return Err("expected variable names or _ for elements"),
                        }
                    }
                    return Ok(Expr::Assign {
                        init,
                        public,
                        mutable,
                        token: token.clone(),
                        left: Box::new(expr),
                        right: value,
                    });
                }
                Expr::Variable { ref name } => {
                    return Ok(Expr::Assign {
                        init,
                        public,
                        mutable,
                        token: name.clone(),
                        left: Box::new(expr),
                        right: value,
                    })
                }
                Expr::Get {
                    instance,
                    value,
                    token,
                } => {
                    return Ok(Expr::Set {
                        instance,
                        token,
                        value,
                    })
                }
                _ => {
                    return Err("invalid assignment target");
                }
            }
        } else if self.does_match(&[
            TokenType::PlusEq,
            TokenType::MinusEq,
            TokenType::MulEq,
            TokenType::DivEq,
            TokenType::ModEq,
        ]) {
            let op = self.previous();
            let value = self.assignment()?;
            match expr {
                Expr::Variable { name: ref token } => {
                    return Ok(Expr::Assign {
                        token: token.clone(),
                        init: false,
                        public: false,
                        mutable: false,
                        left: Box::new(expr.clone()),
                        right: Box::new(Expr::Binary {
                            left: Box::new(expr),
                            right: Box::new(value),
                            op,
                        }),
                    });
                }
                _ => return Err("expected a variable"),
            };
        } else if self.does_match(&[TokenType::DPlus, TokenType::DMinus]) {
            let mut op = self.previous();
            match expr {
                Expr::Variable { name: ref token } => {
                    op.kind = if op.kind == TokenType::DPlus {
                        TokenType::Plus
                    } else {
                        TokenType::Minus
                    };
                    return Ok(Expr::Assign {
                        token: token.clone(),
                        init: false,
                        public: false,
                        mutable: false,
                        left: Box::new(expr.clone()),
                        right: Box::new(Expr::Binary {
                            left: Box::new(expr),
                            right: Box::new(Expr::IntegerLiteral {
                                token: op.clone(),
                                value: 1,
                            }),
                            op,
                        }),
                    });
                }
                _ => return Err("expected a variable"),
            }
        }

        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, &'static str> {
        if self.does_match(&[TokenType::True, TokenType::False]) {
            // Boolean
            let token = self.previous();
            Ok(Expr::BoolLiteral {
                token: token.clone(),
                payload: if token.kind == TokenType::True {
                    true
                } else {
                    false
                },
            })
        } else if self.does_match(&[TokenType::Underscore]) {
            // Underscore
            let token = self.previous();
            Ok(Expr::Underscore { token })
        } else if self.does_match(&[TokenType::Null]) {
            // Null
            let token = self.previous();
            Ok(Expr::Null { token })
        } else if self.does_match(&[TokenType::Int, TokenType::Float]) {
            // Number
            let token = self.previous();
            if token.kind == TokenType::Int {
                let value = token.value.parse::<i64>();
                if let Ok(value) = value {
                    Ok(Expr::IntegerLiteral { token, value })
                } else {
                    Err("invalid number")
                }
            } else {
                let value = token.value.parse::<f64>();
                if let Ok(value) = value {
                    Ok(Expr::FloatLiteral { token, value })
                } else {
                    Err("invalid number")
                }
            }
        } else if self.does_match(&[TokenType::Str]) {
            // String
            let token = self.previous();
            Ok(Expr::StringLiteral {
                token: token.clone(),
                value: token.value,
            })
        } else if self.does_match(&[TokenType::Atom]) {
            // Atom
            let token = self.previous();
            Ok(Expr::AtomLiteral {
                token: token.clone(),
                value: token.value,
            })
        } else if self.does_match(&[TokenType::Id]) {
            // identifier
            let token = self.previous();
            Ok(Expr::Variable { name: token })
        } else if self.does_match(&[TokenType::LParen]) {
            // grouping
            let expr = Box::new(self.expression()?);
            expect!(self, TokenType::RParen, "expected ')'");
            Ok(Expr::Group { expr })
        } else if self.does_match(&[TokenType::LBracket]) {
            // list literal
            let token = self.previous();
            let mut values: Vec<Box<Expr>> = vec![];
            while !self.check_current(TokenType::RBracket) && !self.is_end() {
                values.push(Box::new(self.expression()?));

                if self.check_current(TokenType::RBracket) || !self.does_match(&[TokenType::Comma])
                {
                    break;
                }
            }
            expect!(self, TokenType::RBracket, "expected ']'");
            Ok(Expr::ListLiteral { token, values })
        } else if self.does_match(&[TokenType::LBrace]) {
            // object or block
            let token = self.previous();

            if self.check_current(TokenType::RBrace) {
                // empty {} is always considered an
                // object; an empty block is illegal
                self.advance();
                Ok(Expr::ObjectLiteral {
                    token,
                    keys: vec![],
                    values: vec![],
                })
            } else {
                let first_expr = self.expression()?;
                if self.is_end() {
                    return Err("unexpected end of input inside block or object");
                }
                if self.does_match(&[TokenType::Colon]) {
                    // it's an object!
                    let token: Token;
                    match first_expr {
                        Expr::Variable { name } => token = name,
                        _ => return Err("expected an identifier"),
                    }

                    let mut keys: Vec<Token> = vec![token.clone()];
                    let mut values: Vec<Box<Expr>> = vec![];

                    if self.previous().kind != TokenType::Colon {
                        return Err("expected ':'");
                    }

                    values.push(Box::new(self.expression()?));
                    if self.does_match(&[TokenType::Comma]) {
                        while !self.check_current(TokenType::RBrace) && !self.is_end() {
                            expect!(self, TokenType::Id, "expected an identifier");
                            keys.push(self.previous().clone());
                            expect!(self, TokenType::Colon, "expected ':'");
                            values.push(Box::new(self.expression()?));
                            if self.check_current(TokenType::RBrace)
                                || !self.does_match(&[TokenType::Comma])
                            {
                                break;
                            }
                        }
                    }

                    expect!(self, TokenType::RBrace, "expected '}'");
                    Ok(Expr::ObjectLiteral {
                        token,
                        keys,
                        values,
                    })
                } else {
                    // it's a block!
                    let mut exprs: Vec<Box<Expr>> = vec![];
                    exprs.push(Box::new(first_expr));
                    while !self.check_current(TokenType::RBrace) && !self.is_end() {
                        exprs.push(Box::new(self.expression()?));
                    }
                    expect!(self, TokenType::RBrace, "expected '}'");
                    Ok(Expr::Block {
                        token: self.previous(),
                        exprs,
                    })
                }
            }
        } else if self.does_match(&[TokenType::MinusGT]) {
            // short-hand anonymous function
            let (params, rest) = if self.check_current(TokenType::RParen) {
                self.parse_params()?
            } else {
                (vec![], None)
            };
            let body = Box::new(self.expression()?);
            Ok(Expr::Func {
                public: false,
                name: None,
                params,
                rest,
                body,
            })
        } else if self.check_current(TokenType::Func) {
            // private function
            self.function(false, vec![])
        } else if self.does_match(&[TokenType::Public]) {
            // public function
            self.function(true, vec![])
        } else if self.does_match(&[TokenType::At]) {
            // decorator-applied function
            expect!(self, TokenType::LBracket, "expected '['");

            // get decorators
            let mut decorators = vec![];
            while !self.check_current(TokenType::RBracket) {
                let mut dec_args = vec![];
                let decorator_name = Expr::Variable {
                    name: self.current.clone(),
                };
                self.advance();
                if self.does_match(&[TokenType::LParen]) {
                    if !self.check_current(TokenType::RParen) {
                        if self.does_match(&[TokenType::Ellipsis]) {
                            dec_args.push(CallArg::Unpacking(Box::new(self.expression()?)));
                        } else {
                            dec_args.push(CallArg::Positional(Box::new(self.expression()?)));
                        }
                        while self.does_match(&[TokenType::Comma]) {
                            if self.does_match(&[TokenType::Ellipsis]) {
                                dec_args.push(CallArg::Unpacking(Box::new(self.expression()?)));
                            } else {
                                dec_args.push(CallArg::Positional(Box::new(self.expression()?)));
                            }
                        }
                    }
                    expect!(self, TokenType::RParen, "expected ')'");
                }
                decorators.push((decorator_name, dec_args));
                if self.does_match(&[TokenType::Comma]) {
                    continue;
                } else {
                    break;
                }
            }
            expect!(self, TokenType::RBracket, "expected ']'");

            self.function(false, decorators)
        } else if self.does_match(&[TokenType::Match]) {
            // match expression
            let token = self.previous();
            let condition = self.expression()?;
            expect!(self, TokenType::LBrace, "expected '{'");

            let mut branches: Vec<MatchBranch> = vec![];
            while !self.check_current(TokenType::RBrace) {
                let expr = self.expression()?;
                expect!(self, TokenType::MinusGT, "expected '->'");

                let body = self.expression()?;
                branches.push(MatchBranch {
                    target: Box::new(expr),
                    body: Box::new(body),
                });

                if !self.does_match(&[TokenType::Comma]) {
                    break;
                }
            }
            expect!(self, TokenType::RBrace, "expected '}'");

            Ok(Expr::Match {
                token,
                condition: Box::new(condition),
                branches,
            })
        } else if self.does_match(&[TokenType::Try]) {
            // try expression
            let token = self.previous();
            let result = self.expression()?;
            let err_handling = self.expression()?;
            let mut success: Option<Expr> = None;
            if self.check_current(TokenType::Else) {
                expect!(self, TokenType::Else, "expected 'else'");
                success = Some(self.expression()?);
            }
            Ok(Expr::Match {
                token: token.clone(),
                condition: Box::new(Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: Token::new(
                            TokenType::Id,
                            "error?".to_string(),
                            token.position.0,
                            token.position.1,
                        ),
                    }),
                    args: vec![CallArg::Positional(Box::new(result))],
                    token: token.clone(),
                }),
                branches: vec![
                    MatchBranch {
                        target: Box::new(Expr::BoolLiteral {
                            token: token.clone(),
                            payload: true,
                        }),
                        body: Box::new(err_handling),
                    },
                    MatchBranch {
                        target: Box::new(Expr::BoolLiteral {
                            token: token.clone(),
                            payload: false,
                        }),
                        body: Box::new(match success {
                            Some(expr) => expr,
                            None => Expr::Null {
                                token: token.clone(),
                            },
                        }),
                    },
                ],
            })
        } else if self.does_match(&[TokenType::Unsafe]) {
            // unsafe expression
            let token = self.previous();
            let expr = self.expression()?;
            Ok(Expr::Unsafe {
                token,
                expr: Box::new(expr),
            })
        } else if self.does_match(&[TokenType::Dollar]) {
            // shell operator
            let token = self.previous();
            let expr = self.expression()?;
            Ok(Expr::Shell {
                token,
                expr: Box::new(expr),
            })
        } else if self.does_match(&[TokenType::Def]) {
            // macro definition
            expect!(self, TokenType::Id, "expected an identifier");
            let name = self.previous();
            let expr = self.expression()?;
            if self.macros.contains_key(&name.value) {
                Err("macro with the same name is already defined")
            } else {
                self.macros.insert(name.value.clone(), expr);
                Ok(Expr::Null { token: name })
            }
        } else if self.does_match(&[TokenType::Redef]) {
            // macro redefinition
            expect!(self, TokenType::Id, "expected an identifier");
            let name = self.previous();
            let expr = self.expression()?;
            if self.macros.contains_key(&name.value) {
                self.macros.insert(name.value.clone(), expr);
                Ok(Expr::Null { token: name })
            } else {
                Err("macro with the same name is not defined")
            }
        } else if self.does_match(&[TokenType::Hash]) {
            // macro usage
            expect!(self, TokenType::Id, "expected an identifier");
            let name = self.previous();
            match self.macros.get(&name.value) {
                Some(expr) => Ok(expr.clone()),
                None => Err("macro undefined"),
            }
        } else {
            // println!("{:#?}", self.current);
            Err("unexpected token")
        }
    }

    fn function(
        &mut self,
        mut public: bool,
        decorators: Vec<(Expr, Vec<CallArg>)>,
    ) -> Result<Expr, &'static str> {
        if !public {
            if self.does_match(&[TokenType::Public]) {
                public = true;
            }
        }
        expect!(self, TokenType::Func, "expected 'func'");
        let name: Option<Token> = if self.check_current(TokenType::Id) {
            self.advance();
            Some(self.previous())
        } else {
            None
        };
        let params = self.parse_params()?;
        let body = self.expression()?;

        // check decorators
        if decorators.is_empty() {
            Ok(Expr::Func {
                public,
                name,
                params: params.0,
                rest: params.1,
                body: Box::new(body),
            })
        } else {
            // apply the decorators
            let mut right: Expr = Expr::Func {
                public: false,
                name: None,
                params: params.0,
                rest: params.1,
                body: Box::new(body),
            };
            for d in decorators {
                let mut args = vec![CallArg::Positional(Box::new(right))];
                for i in d.1 {
                    args.push(i);
                }
                right = Expr::Call {
                    callee: Box::new(d.0),
                    args,
                    token: self.previous(),
                }
            }

            match name {
                Some(name) => Ok(Expr::Assign {
                    init: true,
                    public,
                    mutable: false,
                    token: name.clone(),
                    left: Box::new(Expr::Variable { name }),
                    right: Box::new(right),
                }),
                None => Err("expected function name"),
            }
        }
    }

    fn finish_call(&mut self, callee: Expr, arg: Option<Expr>) -> Result<Expr, &'static str> {
        let callee = Box::new(callee);
        let mut args: Vec<CallArg> = vec![];
        if match arg {
            // check for |>
            Some(_) => true,
            _ => false,
        } {
            args.push(CallArg::Positional(Box::new(arg.unwrap())));
        }

        if !self.check_current(TokenType::RParen) {
            if self.does_match(&[TokenType::Ellipsis]) {
                args.push(CallArg::Unpacking(Box::new(self.expression()?)));
            } else {
                args.push(CallArg::Positional(Box::new(self.expression()?)));
            }
            while self.does_match(&[TokenType::Comma]) {
                if self.does_match(&[TokenType::Ellipsis]) {
                    args.push(CallArg::Unpacking(Box::new(self.expression()?)));
                } else {
                    args.push(CallArg::Positional(Box::new(self.expression()?)));
                }
            }
        }
        expect!(self, TokenType::RParen, "expected ')'");
        let token = self.previous();

        // check for <|
        if self.does_match(&[TokenType::LPipe]) {
            args.push(CallArg::Positional(Box::new(self.expression()?)));
        }

        // check for <~
        if self.does_match(&[TokenType::LTilde]) {
            let (params, rest) = if self.check_current(TokenType::LParen) {
                self.parse_params()?
            } else {
                (vec![], None)
            };
            let body = Box::new(self.expression()?);
            let func = Expr::Func {
                public: false,
                name: None,
                params,
                rest,
                body,
            };
            args.push(CallArg::Positional(Box::new(func)));
        }

        Ok(Expr::Call {
            callee,
            args,
            token,
        })
    }

    fn call(&mut self, arg: &Option<Expr>) -> Result<Expr, &'static str> {
        let mut expr = self.primary()?;
        loop {
            if self.does_match(&[TokenType::LParen]) {
                // function call
                expr = self.finish_call(expr, arg.clone())?;
            } else if self.does_match(&[TokenType::Dot]) {
                // object access or list indexing
                let token = self.previous();
                let value = self.expression()?;
                expr = Expr::Get {
                    instance: Box::new(expr),
                    value: Box::new(value),
                    token,
                }
            } else if self.does_match(&[TokenType::RPipe]) {
                // pipe
                expr = self.call(&Some(expr))?;
                break;
            } else if self.does_match(&[TokenType::Question]) {
                // short-hand match
                let token = self.previous();
                let true_value = self.expression()?;
                expect!(self, TokenType::Colon, "expected ':'");
                let false_value = self.expression()?;

                let true_branch = MatchBranch {
                    target: Box::new(Expr::BoolLiteral {
                        token: token.clone(),
                        payload: true,
                    }),
                    body: Box::new(true_value),
                };
                let false_branch = MatchBranch {
                    target: Box::new(Expr::Underscore {
                        token: token.clone(),
                    }),
                    body: Box::new(false_value),
                };

                expr = Expr::Match {
                    token,
                    condition: Box::new(expr),
                    branches: vec![true_branch, false_branch],
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, &'static str> {
        if self.does_match(&[TokenType::Bang, TokenType::Not, TokenType::Minus]) {
            let op = self.previous();
            Ok(Expr::Unary {
                right: Box::new(self.unary()?),
                op,
            })
        } else {
            self.call(&None)
        }
    }

    fn factor(&mut self) -> Result<Expr, &'static str> {
        let mut expr = self.unary()?;
        while self.does_match(&[TokenType::Div, TokenType::Mul, TokenType::Mod]) {
            let op = self.previous();
            expr = Expr::Binary {
                left: Box::new(expr),
                right: Box::new(self.unary()?),
                op,
            };
        }
        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, &'static str> {
        let mut expr = self.factor()?;
        while self.does_match(&[TokenType::Minus, TokenType::Plus]) {
            let op = self.previous();
            expr = Expr::Binary {
                left: Box::new(expr),
                right: Box::new(self.factor()?),
                op,
            };
        }
        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, &'static str> {
        let mut expr = self.term()?;
        while self.does_match(&[
            TokenType::GT,
            TokenType::GTEq,
            TokenType::LT,
            TokenType::LTEq,
        ]) {
            let op = self.previous();
            expr = Expr::Binary {
                left: Box::new(expr),
                right: Box::new(self.term()?),
                op,
            }
        }
        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, &'static str> {
        let mut expr = self.comparison()?;
        while self.does_match(&[TokenType::BangEq, TokenType::DEq]) {
            let op = self.previous();
            expr = Expr::Binary {
                left: Box::new(expr),
                right: Box::new(self.comparison()?),
                op,
            };
        }
        Ok(expr)
    }

    fn and_expr(&mut self) -> Result<Expr, &'static str> {
        let mut expr = self.equality()?;
        while self.does_match(&[TokenType::DAmp, TokenType::And]) {
            let op = self.previous();
            expr = Expr::Logical {
                left: Box::new(expr),
                right: Box::new(self.equality()?),
                op,
            };
        }
        Ok(expr)
    }

    fn or_expr(&mut self) -> Result<Expr, &'static str> {
        let mut expr = self.and_expr()?;
        while self.does_match(&[TokenType::DPipe, TokenType::Or]) {
            let op = self.previous();
            expr = Expr::Logical {
                left: Box::new(expr),
                right: Box::new(self.and_expr()?),
                op,
            };
        }
        Ok(expr)
    }

    fn parse_params(&mut self) -> Result<(Vec<Token>, Option<Token>), &'static str> {
        expect!(self, TokenType::LParen, "expected '('");
        let mut params: Vec<Token> = vec![];
        let mut rest: Option<Token> = None;
        if !self.check_current(TokenType::RParen) {
            loop {
                if match rest {
                    Some(_) => true,
                    None => false,
                } {
                    return Err("required parameter cannot follow rest parameter");
                }

                if self.check_current(TokenType::Id) {
                    // identifier
                    expect!(self, TokenType::Id, "expected an identifier");
                    let param = self.previous();
                    if self.does_match(&[TokenType::Plus]) {
                        rest = Some(param);
                    } else {
                        params.push(param);
                    }
                } else if self.check_current(TokenType::Underscore) {
                    // empty parameter
                    expect!(self, TokenType::Underscore, "expected an underscore");
                    let param = self.previous();
                    params.push(param);
                }

                if !self.does_match(&[TokenType::Comma]) {
                    break;
                }
            }
        }
        expect!(self, TokenType::RParen, "expected ')'");
        Ok((params, rest))
    }

    /// Checks if the current token is in the given types
    fn does_match(&mut self, these: &[TokenType]) -> bool {
        for kind in these {
            if self.check_current(*kind) {
                self.advance();
                return true;
            }
        }
        false
    }

    /// Advances one token
    fn advance(&mut self) {
        if !self.is_end() {
            self.previous = self.current.clone();
            self.current = self.recv.recv().unwrap();
        }
    }

    /// Checks if the token type of the current token is the same as the expected token type
    fn check_current(&self, kind: TokenType) -> bool {
        if self.current.kind == kind {
            true
        } else {
            false
        }
    }

    /// Returns the previous token
    fn previous(&self) -> Token {
        self.previous.clone()
    }

    /// Checks if the end is reached
    fn is_end(&self) -> bool {
        match self.current.kind {
            TokenType::EOF => true,
            _ => false,
        }
    }

    /// Appends the error created with the given error message and the current line and column
    fn add_error(&mut self, message: &str) {
        let error = ParserError::new(message, self.current.position.0, self.current.position.1);
        self.errors.push(error);
    }

    /// Discards tokens until reaching one that can appear at that point in the rule
    fn synchronize(&mut self) {
        if !self.is_end() {
            self.advance();
        } else {
            return;
        }

        while !self.is_end() {
            self.advance();
            match self.current.kind {
                TokenType::Func
                | TokenType::Match
                | TokenType::Def
                | TokenType::Redef
                | TokenType::Try
                | TokenType::Unsafe
                | TokenType::Id => return,
                _ => {}
            }
        }
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::lexer::Lexer;
    use crate::parse;

    #[test]
    fn test_parser() {
        let source = r#"{printfln: println} := import("fmt")
{each: each} := import("std")

names := ["Nobu", "Sol", "Thomas", "Damian", "Ryan", "Zen", "Esfir"]
each(names) <| func(name) println("Hello, %{}!", name)


// fizzbuzz
std := import("std")

public func fizzbuzz(n) match ([n % 3, n % 5]) {
    [0, 0] -> "FizzBuzz",
    [0, _] -> "Fizz",
    [_, 0] -> "Buzz",
    _ -> string(n),
}

std.range(1, 101) |> std.each() <| func(n) {
    std.println(fizzbuzz(n))
}"#;
        let expected = r#"(assignI (object printfln:println) (import "fmt"))
(assignI (object each:each) (import "std"))
(assignI names (list "Nobu" "Sol" "Thomas" "Damian" "Ryan" "Zen" "Esfir"))
(each names (lambda (name) (println "Hello, %{}!" name)))
(assignI std (import "std"))
(func [public] fizzbuzz (n) (match ((list (Mod n 3) (Mod n 5))) (list 0 0) -> "FizzBuzz" (list 0 :_:) -> "Fizz" (list :_: 0) -> "Buzz" :_: -> (string n)))
std.std.(each (lambda (n) (block std.(println (fizzbuzz n)))))"#;

        parse!(source, expected);
    }

    #[test]
    fn test_anonymous_func() {
        let source = r#"add := func (x, y) x + y"#;
        let expected = "(assignI add (lambda (x y) (Plus x y)))";
        parse!(source, expected);
    }

    #[test]
    fn test_shorthand_anonymous_func() {
        let source = r#"something := -> 100"#;
        let expected = "(assignI something (lambda () 100))";
        parse!(source, expected);
    }

    #[test]
    fn test_callback_func() {
        let source = r#"some_func() <~ (a, b) a + b"#;
        let expected = "(some_func (lambda (a b) (Plus a b)))";
        parse!(source, expected);
    }

    #[test]
    fn test_atom_expr() {
        let source = "name := :nobu";
        let expected = "(assignI name :nobu)";
        parse!(source, expected);
    }

    #[test]
    fn test_underscore_expr() {
        let source = "underscore := _";
        let expected = "(assignI underscore :_:)";
        parse!(source, expected);
    }

    #[test]
    fn list_and_object_expr() {
        let source = r#"[1, 2, "abc", {name: "Nobuharu", age: 16}]"#;
        let expected = r#"(list 1 2 "abc" (object name:"Nobuharu" age:16))"#;
        parse!(source, expected);
    }

    #[test]
    fn match_expr() {
        let source = r#"match name { "nobu" -> println("cool!"), _ -> { println("hello") } }"#;
        let expected =
            r#"(match name "nobu" -> (println "cool!") :_: -> (block (println "hello")))"#;
        parse!(source, expected);
    }

    #[test]
    fn assign_expr() {
        let source = r#"[name, _] := ["Nobu", 16] [some_list.0, _] = [1, 3]"#;
        let expected = r#"(assignI (list name :_:) (list "Nobu" 16))
(assign (list some_list.0 :_:) (list 1 3))"#;
        parse!(source, expected);
    }

    #[test]
    fn short_hand_match_expr() {
        let source = r#"name := cool? ? "nobu" : "sol""#;
        let expected = r#"(assignI name (match cool? true -> "nobu" :_: -> "sol"))"#;
        parse!(source, expected);
    }

    #[test]
    fn unsafe_expr() {
        let source = r#"result := unsafe ( 100 / 0 )"#;
        let expected = r#"(assignI result (unsafe (Div 100 0)))"#;
        parse!(source, expected);
    }

    #[test]
    fn shell_operator() {
        let source = r#"result := $`echo "Hello\nWorld"`"#;
        let expected = r#"(assignI result (shell_op "echo "Hello\nWorld""))"#;
        parse!(source, expected);
    }

    #[test]
    fn unary_expr() {
        let source = "not false";
        let expected = "(Not false)";
        parse!(source, expected);
    }

    #[test]
    fn rest_param() {
        let source = "func some_func(_, nums+) {}";
        let expected = "(func some_func (Underscore nums+) (object))";
        parse!(source, expected);
    }

    #[test]
    fn unpacking_arg() {
        let source = "some_func(...abc)";
        let expected = "(some_func ...abc)";
        parse!(source, expected);
    }

    #[test]
    fn decorators() {
        let source = r#"@[some_decorator("something"), another] public func abc() {}"#;
        let expected =
            r#"(assignPI abc (another (some_decorator (lambda () (object)) "something")))"#;
        parse!(source, expected);
    }

    #[test]
    fn macros() {
        let source = r#"def NAME "Nobu" println(#NAME) redef NAME "Sol" println(#NAME)"#;
        let expected = "null\n(println \"Nobu\")\nnull\n(println \"Sol\")";
        parse!(source, expected);
    }

    #[test]
    fn try_expr() {
        let source = r#"try result {} else {}"#;
        let expected = r#"(match (error? result) true -> (object) false -> (object))"#;
        parse!(source, expected);
    }
}
