// Placeholder parser crate for HULK. Parser will produce AST nodes from source.

use hulk_ast::{Expr, Op, Program};
use hulk_lexer::Token;
use hulk_ast::{Stmt, Type};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<Token> {
        if self.pos >= self.tokens.len() {
            None
        } else {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        }
    }

    pub fn parse_expr(&mut self) -> Result<Expr, String> {
        // parse primary
        match self.bump() {
            Some(Token::Int(s)) => {
                let n = s.parse::<i64>().map_err(|e| format!("invalid int: {}", e))?;
                Ok(Expr::Int(n))
            }
            Some(Token::Ident(s)) => Ok(Expr::Ident(s)),
            Some(Token::LParen) => {
                // check for parenthesized expression: either (expr) or (expr op expr)
                let left = self.parse_expr()?;
                match self.peek() {
                    Some(Token::RParen) => {
                        self.bump(); // consume )
                        Ok(left)
                    }
                    Some(Token::Plus) | Some(Token::Minus) | Some(Token::Star) | Some(Token::Slash) => {
                        // binary
                        let op = match self.bump() {
                            Some(Token::Plus) => Op::Add,
                            Some(Token::Minus) => Op::Sub,
                            Some(Token::Star) => Op::Mul,
                            Some(Token::Slash) => Op::Div,
                            _ => unreachable!(),
                        };
                        let right = self.parse_expr()?;
                        match self.bump() {
                            Some(Token::RParen) => Ok(Expr::Binary(Box::new(left), op, Box::new(right))),
                            other => Err(format!("expected ')', got {:?}", other)),
                        }
                    }
                    _ => Err("unexpected token after '('".to_string()),
                }
            }
            other => Err(format!("unexpected token: {:?}", other)),
        }
    }
}

pub fn parse(src: &str) -> Result<Program, String> {
    let tokens = hulk_lexer::lex(src);
    let mut parser = Parser::new(tokens);
    let mut prog = Program::new();
    while parser.peek().is_some() {
        // parse statements: let or expr
        match parser.peek().cloned() {
            Some(Token::Let) => {
                parser.bump(); // consume let
                // expect identifier
                let name = match parser.bump() {
                    Some(Token::Ident(s)) => s,
                    other => return Err(format!("expected identifier after let, got {:?}", other)),
                };
                let mut ty: Option<Type> = None;
                if let Some(Token::Colon) = parser.peek() {
                    parser.bump(); // consume :
                    // only support basic types for now
                    ty = match parser.bump() {
                        Some(Token::Ident(ref s)) if s == "int" => Some(Type::Int),
                        Some(Token::Ident(ref s)) if s == "bool" => Some(Type::Bool),
                        other => return Err(format!("expected type after ':', got {:?}", other)),
                    };
                }
                // expect =
                match parser.bump() {
                    Some(Token::Assign) => {}
                    other => return Err(format!("expected '=' after let binding, got {:?}", other)),
                }
                let expr = parser.parse_expr()?;
                // optional semicolon
                if let Some(Token::Semicolon) = parser.peek() {
                    parser.bump();
                }
                prog.stmts.push(Stmt::Let { name, ty, expr });
            }
            _ => {
                let expr = parser.parse_expr()?;
                if let Some(Token::Semicolon) = parser.peek() {
                    parser.bump();
                }
                prog.stmts.push(Stmt::Expr(expr));
            }
        }
    }
    Ok(prog)
}
