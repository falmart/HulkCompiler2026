// Placeholder type checker and semantic analyzer for HULK.

use std::collections::HashMap;
use hulk_ast::{Program, Stmt, Expr, Type};

#[derive(Debug)]
pub enum CheckError {
    TypeError(String),
}

pub fn check(prog: &Program) -> Result<(), String> {
    let mut env: HashMap<String, Type> = HashMap::new();
    for stmt in &prog.stmts {
        match stmt {
            Stmt::Let { name, ty, expr } => {
                let expr_ty = infer_expr_type(expr, &env)?;
                if let Some(declared) = ty {
                    if &expr_ty != declared {
                        return Err(format!("Type mismatch in let {}: declared {}, found {}", name, declared, expr_ty));
                    }
                    env.insert(name.clone(), declared.clone());
                } else {
                    env.insert(name.clone(), expr_ty);
                }
            }
            Stmt::Expr(expr) => {
                infer_expr_type(expr, &env)?;
            }
        }
    }
    Ok(())
}

fn infer_expr_type(expr: &Expr, env: &HashMap<String, Type>) -> Result<Type, String> {
    match expr {
        Expr::Int(_) => Ok(Type::Int),
        Expr::Bool(_) => Ok(Type::Bool),
        Expr::Ident(name) => env.get(name).cloned().ok_or_else(|| format!("Unknown identifier: {}", name)),
        Expr::Binary(l, op, r) => {
            let lt = infer_expr_type(l, env)?;
            let rt = infer_expr_type(r, env)?;
            if lt != Type::Int || rt != Type::Int {
                return Err("binary arithmetic requires int operands".to_string());
            }
            Ok(Type::Int)
        }
        Expr::If(cond, then, opt_else) => {
            let ct = infer_expr_type(cond, env)?;
            if ct != Type::Bool { return Err("if condition must be bool".to_string()); }
            let tt = infer_expr_type(then, env)?;
            if let Some(e) = opt_else {
                let et = infer_expr_type(e, env)?;
                if tt != et { return Err("then and else must have same type".to_string()); }
            }
            Ok(tt)
        }
    }
}
