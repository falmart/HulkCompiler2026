// Minimal AST crate for HULK: placeholder types

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Bool(bool),
    Ident(String),
    Binary(Box<Expr>, Op, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>), // cond, then, else?
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { name: String, ty: Option<Type>, expr: Expr },
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Bool,
    Func(Vec<Type>, Box<Type>),
}

#[derive(Debug, Clone)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}

impl Program {
    pub fn new() -> Self {
        Program { stmts: vec![] }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Bool => write!(f, "bool"),
            Type::Func(args, ret) => {
                let args_s: Vec<String> = args.iter().map(|a| format!("{}", a)).collect();
                write!(f, "({}) -> {}", args_s.join(", "), ret)
            }
        }
    }
}
