use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use hulk_ast::*;

use crate::env::Env;
use crate::error::RuntimeError;
use crate::value::{HulkObject, Value};

const MAX_CALL_DEPTH: usize = 100;

pub struct Interpreter {
    classes:    HashMap<String, ClassDecl>,
    functions:  HashMap<String, FunctionDecl>,
    macros:     HashMap<String, MacroDecl>,
    call_depth: usize,
    /// The current `self` object while inside a method body.
    current_self: Option<Rc<RefCell<HulkObject>>>,
    /// Class name of the currently-executing method (for base() dispatch).
    current_class_name: Option<String>,
    /// Method name of the currently-executing method (for base() dispatch).
    current_method_name: Option<String>,
}

impl Interpreter {
    pub fn new(program: &Program) -> Self {
        let classes   = program.classes.iter()
            .map(|c| (c.name.clone(), c.clone())).collect();
        let functions = program.functions.iter()
            .map(|f| (f.name.clone(), f.clone())).collect();
        let macros    = program.macros.iter()
            .map(|m| (m.name.clone(), m.clone())).collect();
        Self { classes, functions, macros, call_depth: 0, current_self: None, current_class_name: None, current_method_name: None }
    }

    /// Execute the program and return the entry expression value (or Null).
    pub fn run(&mut self) -> Result<Value, RuntimeError> {
        // We need to eval the entry stored in the program.
        // Callers pass the program; we re-borrow entry here.
        // Actual dispatch is via run_program.
        Ok(Value::Null)
    }

    pub fn run_program(&mut self, program: &Program) -> Result<Value, RuntimeError> {
        let mut env = Env::new();
        // Built-in constants
        env.define("PI", Value::Number(std::f64::consts::PI));
        env.define("E",  Value::Number(std::f64::consts::E));
        match &program.entry {
            Some(e) => self.eval(e, &mut env),
            None    => Ok(Value::Null),
        }
    }

    // ── Core evaluator ────────────────────────────────────────────────────────

    pub fn eval(&mut self, es: &ExprS, env: &mut Env) -> Result<Value, RuntimeError> {
        match &es.node {
            Expr::Number(n)  => Ok(Value::Number(*n)),
            Expr::Bool(b)    => Ok(Value::Boolean(*b)),
            Expr::Str(s)     => Ok(Value::Str(s.clone())),
            Expr::Null       => Ok(Value::Null),

            Expr::Self_ => {
                match &self.current_self {
                    Some(obj) => Ok(Value::Object(obj.clone())),
                    None => Err(RuntimeError::UndefinedVariable { name: "self".into() }),
                }
            }

            Expr::Var(name) => {
                env.lookup(name)
                    .cloned()
                    .ok_or_else(|| RuntimeError::UndefinedVariable { name: name.clone() })
            }

            Expr::Unary { op, operand } => self.eval_unary(op, operand, env),
            Expr::Binary { op, left, right } => self.eval_binary(op, left, right, env),

            Expr::Assign { target, value } => self.eval_assign(target, value, env),

            Expr::Let { bindings, body } => {
                env.push();
                for b in bindings {
                    let val = self.eval(&b.init, env)?;
                    env.define(&b.name, val);
                }
                let result = self.eval(body, env)?;
                env.pop();
                Ok(result)
            }

            Expr::If { cond, then, elif_branches, else_branch } => {
                let cond_val = self.eval(cond, env)?;
                if cond_val.is_truthy() {
                    return self.eval(then, env);
                }
                for (ec, eb) in elif_branches {
                    if self.eval(ec, env)?.is_truthy() {
                        return self.eval(eb, env);
                    }
                }
                match else_branch {
                    Some(eb) => self.eval(eb, env),
                    None     => Ok(Value::Null),
                }
            }

            Expr::While { cond, body } => {
                loop {
                    let c = self.eval(cond, env)?;
                    if !c.is_truthy() { break; }
                    self.eval(body, env)?;
                }
                Ok(Value::Null)
            }

            Expr::Block(stmts) => {
                env.push();
                let mut result = Value::Null;
                for stmt in stmts {
                    result = self.eval(stmt, env)?;
                }
                env.pop();
                Ok(result)
            }

            Expr::Call { callee, args } => {
                // Macros need unevaluated args (AST substitution)
                if let Some(mac) = self.macros.get(callee.as_str()).cloned() {
                    return self.call_macro(&mac, args, env);
                }
                let arg_vals: Vec<Value> = args.iter()
                    .map(|a| self.eval(a, env))
                    .collect::<Result<_, _>>()?;
                // Check if callee is a closure in the current environment
                if let Some(Value::Closure(cdata)) = env.lookup(callee).cloned() {
                    return self.call_closure(&cdata, arg_vals);
                }
                self.call_function(callee, arg_vals)
            }

            Expr::MethodCall { object, method, args } => {
                let obj_val = self.eval(object, env)?;
                let arg_vals: Vec<Value> = args.iter()
                    .map(|a| self.eval(a, env))
                    .collect::<Result<_, _>>()?;
                self.call_method(obj_val, method, arg_vals)
            }

            Expr::FieldAccess { object, field } => {
                let obj_val = self.eval(object, env)?;
                self.get_field(&obj_val, field)
            }

            Expr::Index { array, index } => {
                let arr = self.eval(array, env)?;
                let idx = self.eval(index, env)?;
                self.eval_index(arr, idx)
            }

            Expr::New { type_name, args } => {
                let arg_vals: Vec<Value> = args.iter()
                    .map(|a| self.eval(a, env))
                    .collect::<Result<_, _>>()?;
                self.instantiate(type_name, arg_vals)
            }

            Expr::NewArray { type_name: _, size, init } => {
                let n = self.eval(size, env)?;
                let Value::Number(n) = n else {
                    return Err(RuntimeError::TypeMismatch {
                        expected: "Number".into(),
                        got: n.type_name().into(),
                    });
                };
                let len = n as usize;
                let init_val = match init {
                    Some(blk) => self.eval(blk, env)?,
                    None      => Value::Null,
                };
                Ok(Value::Array(Rc::new(RefCell::new(vec![init_val; len]))))
            }

            Expr::Case { expr, arms } => {
                let val = self.eval(expr, env)?;
                for arm in arms {
                    if self.is_instance_of(&val, arm.type_ann.type_name()) {
                        env.push();
                        env.define(&arm.binding, val);
                        let result = self.eval(&arm.body, env)?;
                        env.pop();
                        return Ok(result);
                    }
                }
                Ok(Value::Null)
            }

            Expr::With { expr, binding, body, fallback } => {
                let val = self.eval(expr, env)?;
                if !val.is_null() {
                    env.push();
                    env.define(binding, val);
                    let result = self.eval(body, env)?;
                    env.pop();
                    Ok(result)
                } else {
                    self.eval(fallback, env)
                }
            }

            Expr::For { var, iter, body } => {
                let iter_val = self.eval(iter, env)?;
                let elements = match &iter_val {
                    Value::Array(rc) => rc.borrow().clone(),
                    other => return Err(RuntimeError::TypeMismatch {
                        expected: "Array".into(),
                        got: other.type_name().into(),
                    }),
                };
                env.push();
                for elem in elements {
                    env.define(var, elem);
                    self.eval(body, env)?;
                }
                env.pop();
                Ok(Value::Null)
            }

            Expr::IsInstance { expr, type_name } => {
                let val = self.eval(expr, env)?;
                Ok(Value::Boolean(self.is_instance_of(&val, type_name)))
            }

            Expr::Cast { expr, .. } => {
                // Runtime doesn't enforce types; just return the value
                self.eval(expr, env)
            }

            Expr::VecLit { elements } => {
                let vals: Vec<Value> = elements.iter()
                    .map(|e| self.eval(e, env))
                    .collect::<Result<_, _>>()?;
                Ok(Value::Array(Rc::new(RefCell::new(vals))))
            }

            Expr::VecComp { body, var, iter } => {
                let iter_val = self.eval(iter, env)?;
                let items = match iter_val {
                    Value::Array(arr) => arr.borrow().clone(),
                    other => return Err(RuntimeError::TypeMismatch {
                        expected: "Array".into(),
                        got: other.type_name().to_string(),
                    }),
                };
                let mut result = Vec::with_capacity(items.len());
                for item in items {
                    env.push();
                    env.define(var, item);
                    let val = self.eval(body, env)?;
                    env.pop();
                    result.push(val);
                }
                Ok(Value::Array(Rc::new(RefCell::new(result))))
            }

            Expr::Lambda { params, body } => {
                let captured = env.snapshot();
                Ok(Value::Closure(Rc::new(crate::value::ClosureData {
                    params: params.clone(),
                    body: *body.clone(),
                    captured,
                })))
            }

            Expr::MacroArgRef(name) => {
                // Should be substituted away before eval; fall back to env lookup
                env.lookup(name).cloned()
                    .ok_or_else(|| RuntimeError::UndefinedVariable { name: format!("@{name}") })
            }

            Expr::MacroArgName(name) => {
                Err(RuntimeError::UndefinedVariable { name: format!("${name} not substituted") })
            }

            Expr::MacroMatch { subject, cases, default_body } => {
                let subj_val = self.eval(subject, env)?;
                for (pat, body) in cases {
                    let pat_val = self.eval(pat, env)?;
                    if subj_val == pat_val {
                        return self.eval(body, env);
                    }
                }
                self.eval(default_body, env)
            }

            Expr::Base { args } => {
                let arg_vals: Vec<Value> = args.iter()
                    .map(|a| self.eval(a, env))
                    .collect::<Result<_, _>>()?;
                self.call_base(arg_vals)
            }
        }
    }

    // ── Unary & binary operators ──────────────────────────────────────────────

    fn eval_unary(&mut self, op: &UnaryOp, operand: &ExprS, env: &mut Env) -> Result<Value, RuntimeError> {
        let val = self.eval(operand, env)?;
        match op {
            UnaryOp::Neg => {
                let Value::Number(n) = val else {
                    return Err(RuntimeError::TypeMismatch { expected: "Number".into(), got: val.type_name().into() });
                };
                Ok(Value::Number(-n))
            }
            UnaryOp::Not => {
                let Value::Boolean(b) = val else {
                    return Err(RuntimeError::TypeMismatch { expected: "Boolean".into(), got: val.type_name().into() });
                };
                Ok(Value::Boolean(!b))
            }
        }
    }

    fn eval_binary(&mut self, op: &BinaryOp, left: &ExprS, right: &ExprS, env: &mut Env) -> Result<Value, RuntimeError> {
        // Short-circuit for logical ops
        match op {
            BinaryOp::And => {
                let l = self.eval(left, env)?;
                return if !l.is_truthy() { Ok(Value::Boolean(false)) } else {
                    let r = self.eval(right, env)?;
                    Ok(Value::Boolean(r.is_truthy()))
                };
            }
            BinaryOp::Or => {
                let l = self.eval(left, env)?;
                return if l.is_truthy() { Ok(Value::Boolean(true)) } else {
                    let r = self.eval(right, env)?;
                    Ok(Value::Boolean(r.is_truthy()))
                };
            }
            _ => {}
        }

        let lv = self.eval(left, env)?;
        let rv = self.eval(right, env)?;

        match op {
            BinaryOp::Add => num_op(lv, rv, |a, b| Value::Number(a + b)),
            BinaryOp::Sub => num_op(lv, rv, |a, b| Value::Number(a - b)),
            BinaryOp::Mul => num_op(lv, rv, |a, b| Value::Number(a * b)),
            BinaryOp::Div => {
                let (a, b) = extract_nums(lv, rv)?;
                if b == 0.0 { return Err(RuntimeError::DivisionByZero); }
                Ok(Value::Number(a / b))
            }
            BinaryOp::Mod => {
                let (a, b) = extract_nums(lv, rv)?;
                if b == 0.0 { return Err(RuntimeError::DivisionByZero); }
                Ok(Value::Number(a % b))
            }
            BinaryOp::Pow => num_op(lv, rv, |a, b| Value::Number(a.powf(b))),

            BinaryOp::Concat => {
                let l = coerce_str(lv)?;
                let r = coerce_str(rv)?;
                Ok(Value::Str(l + &r))
            }
            BinaryOp::ConcatSpace => {
                let l = coerce_str(lv)?;
                let r = coerce_str(rv)?;
                Ok(Value::Str(format!("{l} {r}")))
            }

            BinaryOp::Eq  => Ok(Value::Boolean(lv == rv)),
            BinaryOp::Ne  => Ok(Value::Boolean(lv != rv)),

            BinaryOp::Lt  => num_cmp(lv, rv, |a, b| a < b),
            BinaryOp::Le  => num_cmp(lv, rv, |a, b| a <= b),
            BinaryOp::Gt  => num_cmp(lv, rv, |a, b| a > b),
            BinaryOp::Ge  => num_cmp(lv, rv, |a, b| a >= b),

            BinaryOp::And | BinaryOp::Or => unreachable!("handled above"),
        }
    }

    // ── Assignment ────────────────────────────────────────────────────────────

    fn eval_assign(&mut self, target: &ExprS, value: &ExprS, env: &mut Env) -> Result<Value, RuntimeError> {
        let new_val = self.eval(value, env)?;

        match &target.node {
            Expr::Var(name) => {
                // If the name exists in env, update it.
                // Also sync back to current_self fields if applicable.
                if !env.assign(name, new_val.clone()) {
                    // Not in env — define it (shouldn't happen in valid programs)
                    env.define(name, new_val.clone());
                }
                // If current_self has a field with this name, keep it in sync.
                if let Some(obj) = &self.current_self {
                    if obj.borrow().fields.contains_key(name.as_str()) {
                        obj.borrow_mut().fields.insert(name.clone(), new_val.clone());
                    }
                }
                Ok(new_val)
            }
            Expr::Self_ | Expr::FieldAccess { object: _, field: _ } => {
                // Evaluate object to get the Rc
                let obj_val = self.eval(target.node.as_field_object().unwrap(), env)?;
                let field   = target.node.field_name().unwrap();
                match obj_val {
                    Value::Object(rc) => {
                        rc.borrow_mut().fields.insert(field.to_string(), new_val.clone());
                        Ok(new_val)
                    }
                    _ => Err(RuntimeError::TypeMismatch {
                        expected: "Object".into(),
                        got: obj_val.type_name().into(),
                    }),
                }
            }
            Expr::Index { array, index } => {
                let arr = self.eval(array, env)?;
                let idx = self.eval(index, env)?;
                let i = num_to_index(idx, 0)?;
                match &arr {
                    Value::Array(rc) => {
                        let mut v = rc.borrow_mut();
                        if i >= v.len() {
                            return Err(RuntimeError::IndexOutOfBounds { index: i as i64, len: v.len() });
                        }
                        v[i] = new_val.clone();
                        Ok(new_val)
                    }
                    _ => Err(RuntimeError::TypeMismatch { expected: "Array".into(), got: arr.type_name().into() }),
                }
            }
            _ => Err(RuntimeError::InvalidAssignTarget),
        }
    }

    // ── Function calls ────────────────────────────────────────────────────────

    fn call_closure(&mut self, cdata: &crate::value::ClosureData, args: Vec<Value>) -> Result<Value, RuntimeError> {
        self.call_depth += 1;
        if self.call_depth > MAX_CALL_DEPTH {
            self.call_depth -= 1;
            return Err(RuntimeError::StackOverflow);
        }
        let mut env = Env::new();
        // Restore captured variables
        for (k, v) in &cdata.captured {
            env.define(k.clone(), v.clone());
        }
        // Bind parameters
        for (param, val) in cdata.params.iter().zip(args.into_iter()) {
            env.define(param.name.clone(), val);
        }
        let result = self.eval(&cdata.body, &mut env);
        self.call_depth -= 1;
        result
    }

    // ── Macro calls ───────────────────────────────────────────────────────────

    fn call_macro(&mut self, mac: &MacroDecl, arg_exprs: &[ExprS], env: &mut Env) -> Result<Value, RuntimeError> {
        // vsubs: param_name → replacement ExprS (for ByRef and ByName params)
        // nsubs: param_name → caller's variable name string (for VarName params)
        // val_binds: (param_name, evaluated_value) for Value params
        let mut vsubs: HashMap<String, ExprS> = HashMap::new();
        let mut nsubs: HashMap<String, String> = HashMap::new();
        let mut val_binds: Vec<(String, Value)> = Vec::new();

        for (param, arg_expr) in mac.params.iter().zip(arg_exprs.iter()) {
            match &param.kind {
                MacroParamKind::Value => {
                    let v = self.eval(arg_expr, env)?;
                    val_binds.push((param.name.clone(), v));
                }
                MacroParamKind::ByRef => {
                    let caller_var = match &arg_expr.node {
                        Expr::MacroArgRef(name) | Expr::Var(name) => name.clone(),
                        _ => return Err(RuntimeError::UndefinedFunction {
                            name: format!("expected @var for by-ref param '{}'", param.name),
                        }),
                    };
                    vsubs.insert(param.name.clone(), Spanned::new(Expr::Var(caller_var), arg_expr.span));
                }
                MacroParamKind::ByName => {
                    vsubs.insert(param.name.clone(), arg_expr.clone());
                }
                MacroParamKind::VarName => {
                    let caller_var = match &arg_expr.node {
                        Expr::MacroArgName(name) | Expr::Var(name) => name.clone(),
                        _ => return Err(RuntimeError::UndefinedFunction {
                            name: format!("expected $var for varname param '{}'", param.name),
                        }),
                    };
                    nsubs.insert(param.name.clone(), caller_var);
                }
            }
        }

        let substituted = substitute(&mac.body, &vsubs, &nsubs);

        env.push();
        for (name, val) in val_binds {
            env.define(&name, val);
        }
        let result = self.eval(&substituted, env);
        env.pop();
        result
    }

    fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        self.call_depth += 1;
        if self.call_depth > MAX_CALL_DEPTH {
            self.call_depth -= 1;
            return Err(RuntimeError::StackOverflow);
        }

        let result = self.call_function_inner(name, args);
        self.call_depth -= 1;
        result
    }

    fn call_function_inner(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        // Built-ins take priority
        if let Some(v) = self.call_builtin(name, &args)? {
            return Ok(v);
        }
        // User-defined function
        let func = self.functions.get(name).cloned()
            .ok_or_else(|| RuntimeError::UndefinedFunction { name: name.into() })?;

        let mut env = Env::new();
        for (param, val) in func.params.iter().zip(args.into_iter()) {
            env.define(&param.name, val);
        }
        self.eval(&func.body, &mut env)
    }

    fn call_builtin(&mut self, name: &str, args: &[Value]) -> Result<Option<Value>, RuntimeError> {
        let v = match name {
            "print" => {
                let s = args.first().map(|v| v.to_display()).unwrap_or_default();
                println!("{s}");
                args.first().cloned().unwrap_or(Value::Null)
            }
            "sqrt"  => math1(args, f64::sqrt)?,
            "sin"   => math1(args, f64::sin)?,
            "cos"   => math1(args, f64::cos)?,
            "tan"   => math1(args, f64::tan)?,
            "exp"   => math1(args, f64::exp)?,
            "log"   => {
                let (base, x) = (num_arg(args, 0, "log")?, num_arg(args, 1, "log")?);
                Value::Number(x.log(base))
            }
            "rand"  => Value::Number(pseudo_rand()),
            "range" => {
                if args.len() == 2 {
                    // range(start, end) → [start, start+1, ..., end-1]
                    let start = num_arg(args, 0, "range")? as i64;
                    let end   = num_arg(args, 1, "range")? as i64;
                    Value::Array(Rc::new(RefCell::new(
                        (start..end).map(|i| Value::Number(i as f64)).collect()
                    )))
                } else {
                    // range(n) → [0, 1, ..., n-1]
                    let n = num_arg(args, 0, "range")? as usize;
                    Value::Array(Rc::new(RefCell::new(
                        (0..n).map(|i| Value::Number(i as f64)).collect()
                    )))
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(v))
    }

    // ── Method calls ─────────────────────────────────────────────────────────

    fn call_method(&mut self, receiver: Value, method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        self.call_depth += 1;
        if self.call_depth > MAX_CALL_DEPTH {
            self.call_depth -= 1;
            return Err(RuntimeError::StackOverflow);
        }
        let result = self.call_method_inner(receiver, method, args);
        self.call_depth -= 1;
        result
    }

    fn call_method_inner(&mut self, receiver: Value, method: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        // Built-in methods on primitive types and arrays
        match &receiver {
            Value::Str(s) => match method {
                "length"   => return Ok(Value::Number(s.chars().count() as f64)),
                "toNumber" => return Ok(Value::Number(s.parse().unwrap_or(f64::NAN))),
                "concat"   => {
                    let rhs = match args.first() {
                        Some(Value::Str(r)) => r.clone(),
                        _ => return Err(RuntimeError::TypeMismatch { expected: "String".into(), got: "?".into() }),
                    };
                    return Ok(Value::Str(format!("{s}{rhs}")));
                }
                _ => return Err(RuntimeError::UndefinedMethod {
                    type_name: "String".into(), method: method.into(),
                }),
            },
            Value::Number(n) => match method {
                "toString" => return Ok(Value::Str(format_num(*n))),
                "getType"  => return Ok(Value::Str("Number".into())),
                _ => return Err(RuntimeError::UndefinedMethod {
                    type_name: "Number".into(), method: method.into(),
                }),
            },
            Value::Boolean(_) => match method {
                "getType" => return Ok(Value::Str("Boolean".into())),
                _ => return Err(RuntimeError::UndefinedMethod {
                    type_name: "Boolean".into(), method: method.into(),
                }),
            },
            Value::Array(rc) => match method {
                "size"    => return Ok(Value::Number(rc.borrow().len() as f64)),
                "getType" => return Ok(Value::Str("Array".into())),
                _ => return Err(RuntimeError::UndefinedMethod {
                    type_name: "Array".into(), method: method.into(),
                }),
            },
            Value::Null => match method {
                "getType" => return Ok(Value::Str("Null".into())),
                _ => return Err(RuntimeError::UndefinedMethod {
                    type_name: "Null".into(), method: method.into(),
                }),
            },
            Value::Closure(_) => return Err(RuntimeError::UndefinedMethod {
                type_name: "Function".into(), method: method.into(),
            }),
            Value::Object(_) => {} // fall through to user-defined
        }

        // getType() works on any value (fallback for Object)
        if method == "getType" {
            return Ok(Value::Str(receiver.type_name().into()));
        }

        // User-defined method dispatch
        let Value::Object(obj_rc) = receiver else {
            return Err(RuntimeError::UndefinedMethod {
                type_name: receiver.type_name().into(),
                method: method.into(),
            });
        };

        let class_name = obj_rc.borrow().class_name.clone();
        let (method_decl, found_in_class) = self.find_method(&class_name, method)
            .ok_or_else(|| RuntimeError::UndefinedMethod {
                type_name: class_name.clone(),
                method: method.into(),
            })?;

        // Build method env
        let mut env = Env::new();
        // self is accessible
        env.define("self", Value::Object(obj_rc.clone()));
        // Object fields in scope (readable by name without self.)
        for (k, v) in obj_rc.borrow().fields.clone() {
            env.define(&k, v);
        }
        // Method params
        for (param, val) in method_decl.params.iter().zip(args.into_iter()) {
            env.define(&param.name, val);
        }

        // Swap current_self and method context
        let prev_self   = self.current_self.take();
        let prev_class  = self.current_class_name.take();
        let prev_method = self.current_method_name.take();
        self.current_self        = Some(obj_rc.clone());
        self.current_class_name  = Some(found_in_class.clone());
        self.current_method_name = Some(method.to_string());
        let result = self.eval(&method_decl.body, &mut env);
        self.current_self        = prev_self;
        self.current_class_name  = prev_class;
        self.current_method_name = prev_method;

        result
    }

    /// Call the parent class's version of the current method (base()).
    fn call_base(&mut self, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let class_name  = self.current_class_name.clone()
            .ok_or_else(|| RuntimeError::Custom("base() used outside method".into()))?;
        let method_name = self.current_method_name.clone()
            .ok_or_else(|| RuntimeError::Custom("base() used outside method".into()))?;

        // Find the parent class
        let parent = self.classes.get(&class_name)
            .and_then(|c| c.base.clone())
            .ok_or_else(|| RuntimeError::Custom(format!("{class_name} has no base class")))?;

        let obj_rc = self.current_self.clone()
            .ok_or_else(|| RuntimeError::Custom("base() used outside method".into()))?;

        let (method_decl, found_in_class) = self.find_method(&parent, &method_name)
            .ok_or_else(|| RuntimeError::UndefinedMethod {
                type_name: parent.clone(),
                method: method_name.clone(),
            })?;

        let mut env = Env::new();
        env.define("self", Value::Object(obj_rc.clone()));
        for (k, v) in obj_rc.borrow().fields.clone() {
            env.define(&k, v);
        }
        for (param, val) in method_decl.params.iter().zip(args.into_iter()) {
            env.define(&param.name, val);
        }

        let prev_class  = self.current_class_name.take();
        let prev_method = self.current_method_name.take();
        self.current_class_name  = Some(found_in_class);
        self.current_method_name = Some(method_name);
        let result = self.eval(&method_decl.body, &mut env);
        self.current_class_name  = prev_class;
        self.current_method_name = prev_method;
        result
    }

    /// Find a method by walking the class hierarchy (derived → base).
    fn find_method(&self, class_name: &str, method: &str) -> Option<(FunctionDecl, String)> {
        let mut current = class_name.to_string();
        let mut visited = std::collections::HashSet::new();
        loop {
            if !visited.insert(current.clone()) { break; }
            if let Some(class) = self.classes.get(&current) {
                for m in &class.members {
                    if let ClassMember::Method { name, params, return_type, body, span } = m {
                        if name == method {
                            let fd = FunctionDecl {
                                name: name.clone(),
                                params: params.clone(),
                                return_type: return_type.clone(),
                                body: body.clone(),
                                span: *span,
                            };
                            return Some((fd, current));
                        }
                    }
                }
                // Try base class
                match &class.base {
                    Some(base) => current = base.clone(),
                    None => break,
                }
            } else {
                break;
            }
        }
        None
    }

    // ── Instantiation ─────────────────────────────────────────────────────────

    /// Find the class whose ctor_params we should use (may be a base class).
    fn find_ctor_class(&self, class_name: &str) -> String {
        let mut current = class_name.to_string();
        let mut seen = std::collections::HashSet::new();
        loop {
            if !seen.insert(current.clone()) { break; }
            if let Some(cls) = self.classes.get(&current) {
                if !cls.ctor_params.is_empty() { return current; }
                match &cls.base {
                    Some(base) => current = base.clone(),
                    None => break,
                }
            } else { break; }
        }
        class_name.to_string()
    }

    fn instantiate(&mut self, class_name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let _class = self.classes.get(class_name).cloned()
            .ok_or_else(|| RuntimeError::UndefinedClass { name: class_name.into() })?;

        let obj_rc = Rc::new(RefCell::new(HulkObject::new(class_name)));

        // Collect all attribute initializers from base to derived
        let init_chain = self.collect_attr_chain(class_name);

        // Bind ctor params — use effective class (may be inherited)
        let ctor_class_name = self.find_ctor_class(class_name);
        let ctor_class = self.classes.get(&ctor_class_name).cloned()
            .unwrap_or_else(|| self.classes.get(class_name).cloned().unwrap());
        let mut env = Env::new();
        for (param, val) in ctor_class.ctor_params.iter().zip(args.into_iter()) {
            env.define(&param.name, val);
        }
        // self is available during init
        env.define("self", Value::Object(obj_rc.clone()));

        let prev_self = self.current_self.take();
        self.current_self = Some(obj_rc.clone());

        // Run attribute initializers from base to derived
        for (attr_name, init_expr) in init_chain {
            let val = self.eval(&init_expr, &mut env)?;
            obj_rc.borrow_mut().fields.insert(attr_name.clone(), val.clone());
            // Also update env so subsequent initializers can see updated fields
            if !env.assign(&attr_name, val.clone()) { env.define(&attr_name, val); }
        }

        self.current_self = prev_self;
        Ok(Value::Object(obj_rc))
    }

    /// Walk from base class to derived, collecting (field_name, init_expr) in order.
    fn collect_attr_chain(&self, class_name: &str) -> Vec<(String, ExprS)> {
        let mut chain = Vec::new();
        self.collect_attr_chain_inner(class_name, &mut chain, &mut std::collections::HashSet::new());
        chain
    }

    fn collect_attr_chain_inner(
        &self,
        class_name: &str,
        chain: &mut Vec<(String, ExprS)>,
        visited: &mut std::collections::HashSet<String>,
    ) {
        if !visited.insert(class_name.to_string()) { return; }
        if let Some(class) = self.classes.get(class_name) {
            // Base first
            if let Some(base) = &class.base {
                if base != "Object" {
                    self.collect_attr_chain_inner(base, chain, visited);
                }
            }
            for m in &class.members {
                if let ClassMember::Attribute { name, init, .. } = m {
                    chain.push((name.clone(), init.clone()));
                }
            }
        }
    }

    // ── Field access ──────────────────────────────────────────────────────────

    fn get_field(&self, val: &Value, field: &str) -> Result<Value, RuntimeError> {
        match val {
            Value::Object(rc) => {
                rc.borrow().fields.get(field)
                    .cloned()
                    .ok_or_else(|| RuntimeError::UndefinedField {
                        type_name: rc.borrow().class_name.clone(),
                        field: field.into(),
                    })
            }
            _ => Err(RuntimeError::UndefinedField {
                type_name: val.type_name().into(),
                field: field.into(),
            }),
        }
    }

    // ── Array indexing ────────────────────────────────────────────────────────

    fn eval_index(&self, arr: Value, idx: Value) -> Result<Value, RuntimeError> {
        let i = match &idx {
            Value::Number(n) => *n as i64,
            _ => return Err(RuntimeError::TypeMismatch { expected: "Number".into(), got: idx.type_name().into() }),
        };
        match &arr {
            Value::Array(rc) => {
                let v = rc.borrow();
                let len = v.len();
                if i < 0 || i as usize >= len {
                    return Err(RuntimeError::IndexOutOfBounds { index: i, len });
                }
                Ok(v[i as usize].clone())
            }
            _ => Err(RuntimeError::TypeMismatch { expected: "Array".into(), got: arr.type_name().into() }),
        }
    }

    // ── Runtime type check (for `case`) ───────────────────────────────────────

    fn is_instance_of(&self, val: &Value, type_name: &str) -> bool {
        match val {
            Value::Number(_)  => type_name == "Number" || type_name == "Object",
            Value::Boolean(_) => type_name == "Boolean" || type_name == "Object",
            Value::Str(_)     => type_name == "String" || type_name == "Object",
            Value::Null       => false,
            Value::Array(_)   => type_name == "Object",
            Value::Closure(_) => type_name == "Object",
            Value::Object(rc) => {
                let class_name = rc.borrow().class_name.clone();
                self.is_class_subtype(&class_name, type_name)
            }
        }
    }

    fn is_class_subtype(&self, sub: &str, sup: &str) -> bool {
        if sub == sup || sup == "Object" { return true; }
        let mut current = sub.to_string();
        let mut seen = std::collections::HashSet::new();
        loop {
            if !seen.insert(current.clone()) { return false; }
            match self.classes.get(&current).and_then(|c| c.base.clone()) {
                Some(base) => {
                    if base == sup { return true; }
                    current = base;
                }
                None => return false,
            }
        }
    }
}

// ── Helper trait for field-access targets ─────────────────────────────────────

trait FieldTarget {
    fn as_field_object(&self) -> Option<&ExprS>;
    fn field_name(&self) -> Option<&str>;
}

impl FieldTarget for Expr {
    fn as_field_object(&self) -> Option<&ExprS> {
        match self {
            Expr::FieldAccess { object, .. } => Some(object),
            _ => None,
        }
    }
    fn field_name(&self) -> Option<&str> {
        match self {
            Expr::FieldAccess { field, .. } => Some(field),
            _ => None,
        }
    }
}

// ── Free helper functions ─────────────────────────────────────────────────────

fn num_op(l: Value, r: Value, f: impl Fn(f64, f64) -> Value) -> Result<Value, RuntimeError> {
    let (a, b) = extract_nums(l, r)?;
    Ok(f(a, b))
}

fn extract_nums(l: Value, r: Value) -> Result<(f64, f64), RuntimeError> {
    match (&l, &r) {
        (Value::Number(a), Value::Number(b)) => Ok((*a, *b)),
        _ => Err(RuntimeError::TypeMismatch {
            expected: "Number".into(),
            got: format!("{} and {}", l.type_name(), r.type_name()),
        }),
    }
}

fn num_cmp(l: Value, r: Value, f: impl Fn(f64, f64) -> bool) -> Result<Value, RuntimeError> {
    let (a, b) = extract_nums(l, r)?;
    Ok(Value::Boolean(f(a, b)))
}

fn coerce_str(v: Value) -> Result<String, RuntimeError> {
    match v {
        Value::Str(s)    => Ok(s),
        Value::Number(n) => Ok(format_num(n)),
        Value::Boolean(b)=> Ok(b.to_string()),
        Value::Null      => Ok("null".into()),
        other => Err(RuntimeError::TypeMismatch { expected: "String".into(), got: other.type_name().into() }),
    }
}

fn format_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) }
    else { format!("{n}") }
}

fn math1(args: &[Value], f: fn(f64) -> f64) -> Result<Value, RuntimeError> {
    let n = num_arg(args, 0, "math")?;
    Ok(Value::Number(f(n)))
}

fn num_arg(args: &[Value], i: usize, ctx: &str) -> Result<f64, RuntimeError> {
    match args.get(i) {
        Some(Value::Number(n)) => Ok(*n),
        Some(other) => Err(RuntimeError::TypeMismatch { expected: "Number".into(), got: other.type_name().into() }),
        None => Err(RuntimeError::Custom(format!("{ctx}: missing argument {i}"))),
    }
}

fn num_to_index(v: Value, _len: usize) -> Result<usize, RuntimeError> {
    match v {
        Value::Number(n) if n >= 0.0 => Ok(n as usize),
        Value::Number(n) => Err(RuntimeError::IndexOutOfBounds { index: n as i64, len: 0 }),
        other => Err(RuntimeError::TypeMismatch { expected: "Number".into(), got: other.type_name().into() }),
    }
}

// Simple LCG pseudo-random (determinism for tests; replace with rand crate in production)
fn pseudo_rand() -> f64 {
    use std::cell::Cell;
    thread_local! { static SEED: Cell<u64> = const { Cell::new(12345) }; }
    SEED.with(|s| {
        let x = s.get().wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s.set(x);
        (x >> 33) as f64 / (u32::MAX as f64)
    })
}

// ── TypeExpr helper ───────────────────────────────────────────────────────────

trait TypeExprName {
    fn type_name(&self) -> &str;
}

impl TypeExprName for TypeExpr {
    fn type_name(&self) -> &str {
        match self {
            TypeExpr::Named(n) => n.as_str(),
            TypeExpr::Array(_) => "Array",
            TypeExpr::Iterable(_) => "Array",
            TypeExpr::Function { .. } => "Function",
        }
    }
}

// ── AST substitution for macros ───────────────────────────────────────────────
// vsubs: param_name → replacement ExprS (substitutes Var(name) and MacroArgRef(name))
// nsubs: param_name → caller variable name string (substitutes MacroArgName(name) → Str)

fn substitute(es: &ExprS, vsubs: &HashMap<String, ExprS>, nsubs: &HashMap<String, String>) -> ExprS {
    let span = es.span;
    macro_rules! sub {
        ($e:expr) => { substitute($e, vsubs, nsubs) }
    }
    macro_rules! sub_box {
        ($e:expr) => { Box::new(sub!($e)) }
    }
    macro_rules! sub_vec {
        ($v:expr) => { $v.iter().map(|e| sub!(e)).collect() }
    }
    let new_node = match &es.node {
        Expr::Number(n)  => Expr::Number(*n),
        Expr::Bool(b)    => Expr::Bool(*b),
        Expr::Str(s)     => Expr::Str(s.clone()),
        Expr::Null       => Expr::Null,
        Expr::Self_      => Expr::Self_,

        Expr::Var(name) => {
            if let Some(repl) = vsubs.get(name) { return repl.clone(); }
            Expr::Var(name.clone())
        }
        Expr::MacroArgRef(name) => {
            if let Some(repl) = vsubs.get(name) { return repl.clone(); }
            Expr::MacroArgRef(name.clone())
        }
        Expr::MacroArgName(name) => {
            if let Some(s) = nsubs.get(name) {
                return Spanned::new(Expr::Str(s.clone()), span);
            }
            Expr::MacroArgName(name.clone())
        }

        Expr::Unary { op, operand } => Expr::Unary { op: op.clone(), operand: sub_box!(operand) },
        Expr::Binary { op, left, right } => Expr::Binary { op: op.clone(), left: sub_box!(left), right: sub_box!(right) },
        Expr::Assign { target, value } => Expr::Assign { target: sub_box!(target), value: sub_box!(value) },

        Expr::Let { bindings, body } => Expr::Let {
            bindings: bindings.iter().map(|b| Binding {
                name: b.name.clone(),
                type_ann: b.type_ann.clone(),
                init: sub!(&b.init),
                span: b.span,
            }).collect(),
            body: sub_box!(body),
        },

        Expr::If { cond, then, elif_branches, else_branch } => Expr::If {
            cond: sub_box!(cond),
            then: sub_box!(then),
            elif_branches: elif_branches.iter().map(|(c, b)| (sub!(c), sub!(b))).collect(),
            else_branch: else_branch.as_ref().map(|b| sub_box!(b)),
        },

        Expr::While { cond, body }       => Expr::While { cond: sub_box!(cond), body: sub_box!(body) },
        Expr::Block(stmts)               => Expr::Block(sub_vec!(stmts)),

        Expr::Call { callee, args }      => Expr::Call { callee: callee.clone(), args: sub_vec!(args) },
        Expr::MethodCall { object, method, args } => Expr::MethodCall {
            object: sub_box!(object), method: method.clone(), args: sub_vec!(args),
        },
        Expr::FieldAccess { object, field } => Expr::FieldAccess { object: sub_box!(object), field: field.clone() },
        Expr::Index { array, index }     => Expr::Index { array: sub_box!(array), index: sub_box!(index) },
        Expr::New { type_name, args }    => Expr::New { type_name: type_name.clone(), args: sub_vec!(args) },
        Expr::NewArray { type_name, size, init } => Expr::NewArray {
            type_name: type_name.clone(),
            size: sub_box!(size),
            init: init.as_ref().map(|i| sub_box!(i)),
        },

        Expr::Case { expr, arms } => Expr::Case {
            expr: sub_box!(expr),
            arms: arms.iter().map(|arm| CaseArm {
                binding: arm.binding.clone(),
                type_ann: arm.type_ann.clone(),
                body: sub!(&arm.body),
                span: arm.span,
            }).collect(),
        },

        Expr::With { expr, binding, body, fallback } => Expr::With {
            expr: sub_box!(expr),
            binding: binding.clone(),
            body: sub_box!(body),
            fallback: sub_box!(fallback),
        },

        Expr::For { var, iter, body }    => Expr::For { var: var.clone(), iter: sub_box!(iter), body: sub_box!(body) },
        Expr::IsInstance { expr, type_name } => Expr::IsInstance { expr: sub_box!(expr), type_name: type_name.clone() },
        Expr::Cast { expr, type_name }   => Expr::Cast { expr: sub_box!(expr), type_name: type_name.clone() },

        Expr::VecLit { elements }        => Expr::VecLit { elements: sub_vec!(elements) },
        Expr::VecComp { body, var, iter } => Expr::VecComp { body: sub_box!(body), var: var.clone(), iter: sub_box!(iter) },
        Expr::Base { args }              => Expr::Base { args: sub_vec!(args) },
        Expr::Lambda { params, body }    => Expr::Lambda { params: params.clone(), body: sub_box!(body) },

        Expr::MacroMatch { subject, cases, default_body } => Expr::MacroMatch {
            subject: sub_box!(subject),
            cases: cases.iter().map(|(p, b)| (sub!(p), sub!(b))).collect(),
            default_body: sub_box!(default_body),
        },
    };
    Spanned::new(new_node, span)
}
