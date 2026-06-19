use std::collections::{HashMap, HashSet};

use hulk_ast::*;
use hulk_lexer::Span;

use crate::env::Env;
use crate::error::SemanticError;
use crate::types::Type;

// ── Internal tables ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FuncInfo {
    pub params: Vec<Type>,
    pub ret: Type,
}

#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub params: Vec<Type>,
    pub ret: Type,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub base: Option<String>,
    pub ctor_params: Vec<Type>,
    pub attributes: HashMap<String, Type>,
    pub methods: HashMap<String, MethodInfo>,
    pub span: Span,
}

// ── Protocol table ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    /// Protocols this one extends
    pub extends: Vec<String>,
    /// Required methods (name → signature)
    pub methods: HashMap<String, MethodInfo>,
}

// ── Checker ───────────────────────────────────────────────────────────────────

pub struct Checker {
    pub errors: Vec<SemanticError>,
    pub classes: HashMap<String, ClassInfo>,
    pub functions: HashMap<String, FuncInfo>,
    pub protocols: HashMap<String, ProtocolInfo>,
    current_class: Option<String>,
    builtin_names: std::collections::HashSet<String>,
}

impl Checker {
    pub fn new() -> Self {
        let mut c = Self {
            errors: Vec::new(),
            classes: HashMap::new(),
            functions: HashMap::new(),
            protocols: HashMap::new(),
            current_class: None,
            builtin_names: std::collections::HashSet::new(),
        };
        c.register_builtins();
        c
    }

    // ── Built-ins ─────────────────────────────────────────────────────────────

    fn register_builtins(&mut self) {
        // Built-in functions
        let builtins: &[(&str, &[Type], Type)] = &[
            ("print",  &[Type::Object],                        Type::Object),
            ("sqrt",   &[Type::Number],                        Type::Number),
            ("sin",    &[Type::Number],                        Type::Number),
            ("cos",    &[Type::Number],                        Type::Number),
            ("tan",    &[Type::Number],                        Type::Number),
            ("exp",    &[Type::Number],                        Type::Number),
            ("log",    &[Type::Number, Type::Number],          Type::Number),
            ("rand",   &[],                                    Type::Number),
            ("range",  &[Type::Number],                        Type::Array(Box::new(Type::Number))),
        ];
        for (name, params, ret) in builtins {
            self.functions.insert(name.to_string(), FuncInfo {
                params: params.to_vec(),
                ret: ret.clone(),
            });
            self.builtin_names.insert(name.to_string());
        }

        // Built-in class: Object (all user classes implicitly extend it)
        let obj_methods: HashMap<String, MethodInfo> = [
            ("getType", MethodInfo { params: vec![], ret: Type::Str }),
        ].into_iter().map(|(k, v)| (k.to_string(), v)).collect();

        self.classes.insert("Object".into(), ClassInfo {
            base: None,
            ctor_params: vec![],
            attributes: HashMap::new(),
            methods: obj_methods,
            span: Span::default(),
        });

        // Built-in class: String (methods on String values)
        let str_methods: HashMap<String, MethodInfo> = [
            ("length",   MethodInfo { params: vec![],            ret: Type::Number }),
            ("toNumber", MethodInfo { params: vec![],            ret: Type::Number }),
            ("concat",   MethodInfo { params: vec![Type::Str],   ret: Type::Str }),
        ].into_iter().map(|(k, v)| (k.to_string(), v)).collect();

        self.classes.insert("String".into(), ClassInfo {
            base: Some("Object".into()),
            ctor_params: vec![],
            attributes: HashMap::new(),
            methods: str_methods,
            span: Span::default(),
        });

        // Built-in class: Number
        let num_methods: HashMap<String, MethodInfo> = [
            ("toString", MethodInfo { params: vec![], ret: Type::Str }),
        ].into_iter().map(|(k, v)| (k.to_string(), v)).collect();

        self.classes.insert("Number".into(), ClassInfo {
            base: Some("Object".into()),
            ctor_params: vec![],
            attributes: HashMap::new(),
            methods: num_methods,
            span: Span::default(),
        });

        // Built-in class: Boolean
        self.classes.insert("Boolean".into(), ClassInfo {
            base: Some("Object".into()),
            ctor_params: vec![],
            attributes: HashMap::new(),
            methods: HashMap::new(),
            span: Span::default(),
        });
    }

    // ── Type helpers ──────────────────────────────────────────────────────────

    pub fn resolve_type(&mut self, te: &TypeExpr) -> Type {
        match te {
            TypeExpr::Named(n) => match n.as_str() {
                "Number"  => Type::Number,
                "Boolean" => Type::Boolean,
                "String"  => Type::Str,
                "Object"  => Type::Object,
                "Null"    => Type::Null,
                _ => Type::Named(n.clone()), // class or protocol name
            },
            TypeExpr::Array(inner) => Type::Array(Box::new(self.resolve_type(inner))),
            // T* iterable treated as Array<T> for type-checking purposes
            TypeExpr::Iterable(inner) => Type::Array(Box::new(self.resolve_type(inner))),
            // Function types treated as Object (first-class functions not fully typed yet)
            TypeExpr::Function { .. } => Type::Object,
        }
    }

    /// Returns true if `named` is a known protocol.
    pub fn is_protocol(&self, named: &str) -> bool {
        self.protocols.contains_key(named)
    }

    /// Collect all method signatures required by a protocol (including inherited).
    fn protocol_methods(&self, proto_name: &str) -> HashMap<String, MethodInfo> {
        let mut all = HashMap::new();
        let mut visited = std::collections::HashSet::new();
        self.collect_protocol_methods(proto_name, &mut all, &mut visited);
        all
    }

    fn collect_protocol_methods(
        &self,
        proto_name: &str,
        out: &mut HashMap<String, MethodInfo>,
        visited: &mut std::collections::HashSet<String>,
    ) {
        if !visited.insert(proto_name.to_string()) { return; }
        if let Some(info) = self.protocols.get(proto_name) {
            // First collect parent methods
            for parent in &info.extends {
                self.collect_protocol_methods(parent, out, visited);
            }
            // Then own methods (may override parent)
            for (name, m) in &info.methods {
                out.insert(name.clone(), m.clone());
            }
        }
    }

    /// Check whether a named class satisfies a protocol (structural typing).
    fn class_satisfies_protocol(&self, class_name: &str, proto_name: &str) -> bool {
        let required = self.protocol_methods(proto_name);
        for (method_name, required_sig) in &required {
            match self.lookup_method(&Type::Named(class_name.to_string()), method_name) {
                None => return false,
                Some(actual) => {
                    // Return type must be compatible
                    if !self.is_subtype(&actual.ret, &required_sig.ret) { return false; }
                    // Parameter count must match
                    if actual.params.len() != required_sig.params.len() { return false; }
                }
            }
        }
        true
    }

    /// True if `sub` is assignable where `sup` is expected.
    pub fn is_subtype(&self, sub: &Type, sup: &Type) -> bool {
        if sub == sup { return true; }
        if matches!(sub, Type::Unknown) || matches!(sup, Type::Unknown) { return true; }
        // Everything is an Object
        if matches!(sup, Type::Object) && !matches!(sub, Type::Unknown) { return true; }
        // Null is valid for any non-primitive
        if matches!(sub, Type::Null) && !sup.is_primitive() { return true; }
        // Named types: class hierarchy OR protocol structural conformance
        if let (Type::Named(s), Type::Named(p)) = (sub, sup) {
            // If sup is a protocol, check structural conformance
            if self.is_protocol(p) {
                return self.class_satisfies_protocol(s, p);
            }
            return self.is_class_subtype(s, p);
        }
        // Named class <: Object handled above
        // Array covariance
        if let (Type::Array(s), Type::Array(p)) = (sub, sup) {
            return self.is_subtype(s, p);
        }
        // Map named primitives to their Type equivalents for method lookup context
        if let Type::Named(n) = sub {
            let mapped = match n.as_str() {
                "Number"  => Some(Type::Number),
                "Boolean" => Some(Type::Boolean),
                "String"  => Some(Type::Str),
                _ => None,
            };
            if let Some(m) = mapped {
                return self.is_subtype(&m, sup);
            }
        }
        // Primitive <: protocol: check if the primitive's class satisfies it
        if let Type::Named(p) = sup {
            if self.is_protocol(p) {
                let class_name = match sub {
                    Type::Number  => "Number",
                    Type::Boolean => "Boolean",
                    Type::Str     => "String",
                    _ => return false,
                };
                return self.class_satisfies_protocol(class_name, p);
            }
        }
        false
    }

    fn is_class_subtype(&self, sub: &str, sup: &str) -> bool {
        if sub == sup { return true; }
        if sup == "Object" { return true; }
        let mut visited = HashSet::new();
        let mut current = sub.to_string();
        loop {
            if visited.contains(&current) { return false; } // cycle guard
            visited.insert(current.clone());
            match self.classes.get(&current).and_then(|i| i.base.clone()) {
                Some(base) => {
                    if base == sup { return true; }
                    current = base;
                }
                None => return false,
            }
        }
    }

    /// Compute least-upper-bound (join) of two types for branch merging.
    fn join(&self, a: &Type, b: &Type) -> Type {
        if a == b { return a.clone(); }
        if matches!(a, Type::Unknown) { return b.clone(); }
        if matches!(b, Type::Unknown) { return a.clone(); }
        if self.is_subtype(a, b) { return b.clone(); }
        if self.is_subtype(b, a) { return a.clone(); }
        Type::Object
    }

    /// Look up a method on a type, including inherited ones.
    fn lookup_method(&self, ty: &Type, name: &str) -> Option<MethodInfo> {
        // Array built-in methods
        if let Type::Array(_) = ty {
            return match name {
                "size" => Some(MethodInfo { params: vec![], ret: Type::Number }),
                _ => None,
            };
        }
        let class_name = match ty {
            Type::Str          => "String",
            Type::Number       => "Number",
            Type::Boolean      => "Boolean",
            Type::Object       => "Object",
            Type::Named(n) => {
                // If the type is a protocol, look in the protocol's method table
                if self.is_protocol(n) {
                    return self.protocol_methods(n).get(name).cloned();
                }
                n.as_str()
            }
            _                  => return None,
        };
        let mut current = class_name.to_string();
        let mut visited = HashSet::new();
        loop {
            if visited.contains(&current) { break; }
            visited.insert(current.clone());
            if let Some(info) = self.classes.get(&current) {
                if let Some(m) = info.methods.get(name) {
                    return Some(m.clone());
                }
                match &info.base {
                    Some(base) => current = base.clone(),
                    None => break,
                }
            } else {
                break;
            }
        }
        None
    }

    /// Look up an attribute on a type, including inherited ones.
    fn lookup_attribute(&self, ty: &Type, name: &str) -> Option<Type> {
        let class_name = match ty {
            Type::Named(n) => n.as_str(),
            _              => return None,
        };
        let mut current = class_name.to_string();
        let mut visited = HashSet::new();
        loop {
            if visited.contains(&current) { break; }
            visited.insert(current.clone());
            if let Some(info) = self.classes.get(&current) {
                if let Some(t) = info.attributes.get(name) {
                    return Some(t.clone());
                }
                match &info.base {
                    Some(base) => current = base.clone(),
                    None => break,
                }
            } else {
                break;
            }
        }
        None
    }

    /// Walk the inheritance chain to find effective constructor params.
    fn effective_ctor_params(&self, class_name: &str) -> Vec<Type> {
        let mut current = class_name.to_string();
        let mut seen = std::collections::HashSet::new();
        loop {
            if !seen.insert(current.clone()) { break; }
            if let Some(info) = self.classes.get(&current) {
                if !info.ctor_params.is_empty() {
                    return info.ctor_params.clone();
                }
                match &info.base {
                    Some(base) => current = base.clone(),
                    None => break,
                }
            } else {
                break;
            }
        }
        vec![]
    }

    fn check_circular_inheritance(&mut self) {
        let names: Vec<String> = self.classes.keys().cloned().collect();
        for name in names {
            let span = self.classes[&name].span;
            if self.has_cycle(&name, &mut HashSet::new()) {
                self.errors.push(SemanticError::CircularInheritance {
                    class: name,
                    span,
                });
            }
        }
    }

    fn has_cycle(&self, name: &str, visited: &mut HashSet<String>) -> bool {
        if !visited.insert(name.to_string()) { return true; }
        if let Some(info) = self.classes.get(name) {
            if let Some(base) = &info.base {
                return self.has_cycle(base, visited);
            }
        }
        false
    }

    // ── Declaration collection (pass 1) ───────────────────────────────────────

    pub fn collect_declarations(&mut self, program: &Program) {
        // Collect protocol declarations
        for proto in &program.protocols {
            if self.protocols.contains_key(&proto.name) {
                // Duplicate protocol — ignore silently
                continue;
            }
            let methods: HashMap<String, MethodInfo> = proto.methods.iter().map(|m| {
                let params: Vec<Type> = m.params.iter()
                    .map(|p| p.type_ann.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown))
                    .collect();
                let ret = m.return_type.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown);
                (m.name.clone(), MethodInfo { params, ret })
            }).collect();
            self.protocols.insert(proto.name.clone(), ProtocolInfo {
                extends: proto.extends.clone(),
                methods,
            });
        }

        // Collect class shells (no body yet) — needed for forward refs
        for class in &program.classes {
            if self.classes.contains_key(&class.name) {
                self.errors.push(SemanticError::DuplicateDeclaration {
                    name: class.name.clone(),
                    span: class.span,
                });
                continue;
            }
            let ctor_params: Vec<Type> = class.ctor_params.iter()
                .map(|p| p.type_ann.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown))
                .collect();
            let base = class.base.clone().or(Some("Object".into()));
            self.classes.insert(class.name.clone(), ClassInfo {
                base,
                ctor_params,
                attributes: HashMap::new(),
                methods: HashMap::new(),
                span: class.span,
            });
        }

        // Fill in class members
        for class in &program.classes {
            let mut attrs: HashMap<String, Type> = HashMap::new();
            let mut methods: HashMap<String, MethodInfo> = HashMap::new();
            for member in &class.members {
                match member {
                    ClassMember::Attribute { name, init: _, span: _ } => {
                        // Type of attribute is inferred later (during body check)
                        // For now, mark as Unknown — will be refined
                        attrs.insert(name.clone(), Type::Unknown);
                    }
                    ClassMember::Method { name, params, return_type, .. } => {
                        let param_types: Vec<Type> = params.iter()
                            .map(|p| p.type_ann.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown))
                            .collect();
                        let ret = return_type.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown);
                        methods.insert(name.clone(), MethodInfo { params: param_types, ret });
                    }
                }
            }
            if let Some(info) = self.classes.get_mut(&class.name) {
                info.attributes = attrs;
                info.methods = methods;
            }
        }

        // Collect function declarations (user definitions silently shadow builtins)
        for func in &program.functions {
            if self.functions.contains_key(&func.name)
                && !self.builtin_names.contains(&func.name)
            {
                self.errors.push(SemanticError::DuplicateDeclaration {
                    name: func.name.clone(),
                    span: func.span,
                });
                continue;
            }
            let params: Vec<Type> = func.params.iter()
                .map(|p| p.type_ann.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown))
                .collect();
            let ret = func.return_type.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown);
            self.functions.insert(func.name.clone(), FuncInfo { params, ret });
        }

        // Collect macro declarations — register as callable with Object params/return
        for mac in &program.macros {
            let params: Vec<Type> = mac.params.iter()
                .map(|_| Type::Object)
                .collect();
            self.functions.insert(mac.name.clone(), FuncInfo { params, ret: Type::Object });
        }

        self.check_circular_inheritance();
    }

    // ── Checking pass (pass 2) ────────────────────────────────────────────────

    pub fn check_program(&mut self, program: &Program, global_env: &mut Env) {
        // Built-in constants
        global_env.define("PI", Type::Number);
        global_env.define("E",  Type::Number);
        // Check class declarations
        for class in &program.classes {
            self.check_class_decl(class);
        }
        // Check function declarations
        for func in &program.functions {
            self.check_function_decl(func, global_env);
        }
        // Macro bodies are not type-checked in isolation (they're polymorphic at call sites)
        // Validate protocol conformance for classes that claim to implement protocols
        // (HULK uses structural typing — no explicit 'implements' needed, but we validate
        //  annotations when used as protocol types at call sites)
        if let Some(entry) = &program.entry {
            self.check_expr(entry, global_env);
        }
    }

    fn check_function_decl(&mut self, func: &FunctionDecl, _outer: &mut Env) {
        let mut env = Env::new();
        // Bind parameters
        for param in &func.params {
            let ty = param.type_ann.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown);
            env.define(&param.name, ty);
        }
        let body_ty = self.check_expr(&func.body, &mut env);
        // Check return type if declared
        if let Some(ret_te) = &func.return_type {
            let ret_ty = self.resolve_type(ret_te);
            if !self.is_subtype(&body_ty, &ret_ty) {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: ret_ty,
                    got: body_ty,
                    span: func.body.span,
                });
            }
        }
    }

    fn check_macro_decl(&mut self, mac: &MacroDecl) {
        let mut env = Env::new();
        for param in &mac.params {
            let ty = param.type_ann.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Object);
            env.define(&param.name, ty);
        }
        self.check_expr(&mac.body, &mut env);
    }

    fn check_class_decl(&mut self, class: &ClassDecl) {
        // Validate base class exists
        if let Some(base) = &class.base {
            if !self.classes.contains_key(base.as_str()) {
                self.errors.push(SemanticError::UndefinedClass {
                    name: base.clone(),
                    span: class.span,
                });
            }
        }
        self.current_class = Some(class.name.clone());
        for member in &class.members {
            match member {
                ClassMember::Attribute { name, init, .. } => {
                    let mut env = Env::new();
                    env.define("self", Type::Named(class.name.clone()));
                    // Constructor params are in scope for attribute initializers
                    for p in &class.ctor_params {
                        let ty = p.type_ann.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown);
                        env.define(&p.name, ty);
                    }
                    let init_ty = self.check_expr(init, &mut env);
                    // Update attribute type with inferred type
                    if let Some(info) = self.classes.get_mut(&class.name) {
                        info.attributes.insert(name.clone(), init_ty);
                    }
                }
                ClassMember::Method { params, return_type, body, .. } => {
                    let mut env = Env::new();
                    env.define("self", Type::Named(class.name.clone()));
                    // Constructor params are in scope in method bodies (they become fields)
                    for p in &class.ctor_params {
                        let ty = p.type_ann.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown);
                        env.define(&p.name, ty);
                    }
                    for p in params {
                        let ty = p.type_ann.as_ref().map(|t| self.resolve_type(t)).unwrap_or(Type::Unknown);
                        env.define(&p.name, ty);
                    }
                    let body_ty = self.check_expr(body, &mut env);
                    if let Some(ret_te) = return_type {
                        let ret_ty = self.resolve_type(ret_te);
                        if !self.is_subtype(&body_ty, &ret_ty) {
                            self.errors.push(SemanticError::TypeMismatch {
                                expected: ret_ty,
                                got: body_ty,
                                span: body.span,
                            });
                        }
                    }
                }
            }
        }
        self.current_class = None;
    }

    // ── Expression type inference ─────────────────────────────────────────────

    pub fn check_expr(&mut self, es: &ExprS, env: &mut Env) -> Type {
        let span = es.span;
        match &es.node {
            Expr::Number(_) => Type::Number,
            Expr::Bool(_)   => Type::Boolean,
            Expr::Str(_)    => Type::Str,
            Expr::Null      => Type::Null,
            Expr::Self_     => {
                match &self.current_class {
                    Some(name) => Type::Named(name.clone()),
                    None => {
                        // 'self' outside a class — semantic error
                        self.errors.push(SemanticError::UndefinedVariable {
                            name: "self".into(),
                            span,
                        });
                        Type::Unknown
                    }
                }
            }

            Expr::Var(name) => {
                match env.lookup(name) {
                    Some(ty) => ty.clone(),
                    None => {
                        self.errors.push(SemanticError::UndefinedVariable {
                            name: name.clone(),
                            span,
                        });
                        Type::Unknown
                    }
                }
            }

            Expr::Unary { op, operand } => {
                let ty = self.check_expr(operand, env);
                match op {
                    UnaryOp::Neg => {
                        if !self.is_subtype(&ty, &Type::Number) && !matches!(ty, Type::Unknown) {
                            self.errors.push(SemanticError::NonNumericOperand {
                                op: "-".into(), got: ty, span,
                            });
                        }
                        Type::Number
                    }
                    UnaryOp::Not => {
                        if !self.is_subtype(&ty, &Type::Boolean) && !matches!(ty, Type::Unknown) {
                            self.errors.push(SemanticError::NonBooleanOperand {
                                op: "!".into(), got: ty, span,
                            });
                        }
                        Type::Boolean
                    }
                }
            }

            Expr::Binary { op, left, right } => {
                let lt = self.check_expr(left, env);
                let rt = self.check_expr(right, env);
                self.check_binary(op, &lt, &rt, span)
            }

            Expr::Assign { target, value } => {
                // Target must be a variable or field access
                match &target.node {
                    Expr::Var(_) | Expr::FieldAccess { .. } => {}
                    Expr::Index { .. } => {}
                    _ => {
                        self.errors.push(SemanticError::InvalidAssignTarget { span: target.span });
                    }
                }
                let target_ty = self.check_expr(target, env);
                let value_ty  = self.check_expr(value, env);
                if matches!(value_ty, Type::Null) && target_ty.is_primitive() {
                    self.errors.push(SemanticError::NullAssignedToPrimitive {
                        target_type: target_ty.clone(),
                        span,
                    });
                } else if !self.is_subtype(&value_ty, &target_ty) && !matches!(target_ty, Type::Unknown) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: target_ty,
                        got: value_ty.clone(),
                        span,
                    });
                }
                value_ty
            }

            Expr::Let { bindings, body } => {
                env.push();
                for binding in bindings {
                    let init_ty = self.check_expr(&binding.init, env);
                    let declared_ty = binding.type_ann.as_ref().map(|t| self.resolve_type(t));
                    if let Some(ref decl) = declared_ty {
                        if !self.is_subtype(&init_ty, decl) {
                            self.errors.push(SemanticError::TypeMismatch {
                                expected: decl.clone(),
                                got: init_ty.clone(),
                                span: binding.span,
                            });
                        }
                    }
                    let effective_ty = declared_ty.unwrap_or(init_ty);
                    env.define(&binding.name, effective_ty);
                }
                let result = self.check_expr(body, env);
                env.pop();
                result
            }

            Expr::If { cond, then, elif_branches, else_branch } => {
                let cond_ty = self.check_expr(cond, env);
                if !self.is_subtype(&cond_ty, &Type::Boolean) && !matches!(cond_ty, Type::Unknown) {
                    self.errors.push(SemanticError::ConditionNotBoolean { got: cond_ty, span });
                }
                let mut result = self.check_expr(then, env);
                for (ec, eb) in elif_branches {
                    let ec_ty = self.check_expr(ec, env);
                    if !self.is_subtype(&ec_ty, &Type::Boolean) && !matches!(ec_ty, Type::Unknown) {
                        self.errors.push(SemanticError::ConditionNotBoolean { got: ec_ty, span });
                    }
                    let eb_ty = self.check_expr(eb, env);
                    result = self.join(&result, &eb_ty);
                }
                if let Some(eb) = else_branch {
                    let eb_ty = self.check_expr(eb, env);
                    result = self.join(&result, &eb_ty);
                }
                result
            }

            Expr::While { cond, body } => {
                let cond_ty = self.check_expr(cond, env);
                if !self.is_subtype(&cond_ty, &Type::Boolean) && !matches!(cond_ty, Type::Unknown) {
                    self.errors.push(SemanticError::ConditionNotBoolean { got: cond_ty, span });
                }
                self.check_expr(body, env);
                Type::Null
            }

            Expr::Block(stmts) => {
                env.push();
                let mut ty = Type::Null;
                for stmt in stmts {
                    ty = self.check_expr(stmt, env);
                }
                env.pop();
                ty
            }

            Expr::Call { callee, args } => {
                // Special case: range accepts 1 or 2 Number args
                if callee == "range" {
                    if args.len() < 1 || args.len() > 2 {
                        self.errors.push(SemanticError::ArityMismatch {
                            name: "range".into(),
                            expected: 1,
                            got: args.len(),
                            span,
                        });
                    }
                    for arg in args {
                        let ty = self.check_expr(arg, env);
                        if !self.is_subtype(&ty, &Type::Number) && !matches!(ty, Type::Unknown) {
                            self.errors.push(SemanticError::TypeMismatch {
                                expected: Type::Number,
                                got: ty,
                                span: arg.span,
                            });
                        }
                    }
                    return Type::Array(Box::new(Type::Number));
                }
                // If callee is a closure/function value in scope, allow the call
                if let Some(ty) = env.lookup(callee) {
                    if matches!(ty, Type::Object | Type::Unknown) {
                        for arg in args { self.check_expr(arg, env); }
                        return Type::Unknown;
                    }
                }
                let info = self.functions.get(callee).cloned();
                match info {
                    None => {
                        self.errors.push(SemanticError::UndefinedFunction {
                            name: callee.clone(),
                            span,
                        });
                        for a in args { self.check_expr(a, env); }
                        Type::Unknown
                    }
                    Some(info) => {
                        if info.params.len() != args.len() {
                            self.errors.push(SemanticError::ArityMismatch {
                                name: callee.clone(),
                                expected: info.params.len(),
                                got: args.len(),
                                span,
                            });
                        }
                        for (arg, expected) in args.iter().zip(info.params.iter()) {
                            let arg_ty = self.check_expr(arg, env);
                            if !self.is_subtype(&arg_ty, expected) && !matches!(arg_ty, Type::Unknown) {
                                self.errors.push(SemanticError::TypeMismatch {
                                    expected: expected.clone(),
                                    got: arg_ty,
                                    span: arg.span,
                                });
                            }
                        }
                        // Check remaining args if arity mismatch
                        for arg in args.iter().skip(info.params.len()) {
                            self.check_expr(arg, env);
                        }
                        info.ret.clone()
                    }
                }
            }

            Expr::MethodCall { object, method, args } => {
                let obj_ty = self.check_expr(object, env);
                let method_info = self.lookup_method(&obj_ty, method);
                match method_info {
                    None => {
                        if !matches!(obj_ty, Type::Unknown) {
                            self.errors.push(SemanticError::UndefinedMethod {
                                type_name: obj_ty.name(),
                                method: method.clone(),
                                span,
                            });
                        }
                        for a in args { self.check_expr(a, env); }
                        Type::Unknown
                    }
                    Some(info) => {
                        if info.params.len() != args.len() {
                            self.errors.push(SemanticError::ArityMismatch {
                                name: format!("{}.{method}", obj_ty.name()),
                                expected: info.params.len(),
                                got: args.len(),
                                span,
                            });
                        }
                        for (arg, expected) in args.iter().zip(info.params.iter()) {
                            let arg_ty = self.check_expr(arg, env);
                            if !self.is_subtype(&arg_ty, expected) && !matches!(arg_ty, Type::Unknown) {
                                self.errors.push(SemanticError::TypeMismatch {
                                    expected: expected.clone(),
                                    got: arg_ty,
                                    span: arg.span,
                                });
                            }
                        }
                        for arg in args.iter().skip(info.params.len()) {
                            self.check_expr(arg, env);
                        }
                        info.ret.clone()
                    }
                }
            }

            Expr::FieldAccess { object, field } => {
                let obj_ty = self.check_expr(object, env);
                match self.lookup_attribute(&obj_ty, field) {
                    Some(ty) => ty,
                    None => {
                        if !matches!(obj_ty, Type::Unknown) {
                            self.errors.push(SemanticError::UndefinedField {
                                type_name: obj_ty.name(),
                                field: field.clone(),
                                span,
                            });
                        }
                        Type::Unknown
                    }
                }
            }

            Expr::Index { array, index } => {
                let arr_ty = self.check_expr(array, env);
                let idx_ty = self.check_expr(index, env);
                if !self.is_subtype(&idx_ty, &Type::Number) && !matches!(idx_ty, Type::Unknown) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: Type::Number,
                        got: idx_ty,
                        span: index.span,
                    });
                }
                match arr_ty {
                    Type::Array(elem) => *elem,
                    Type::Unknown     => Type::Unknown,
                    other => {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: Type::Array(Box::new(Type::Unknown)),
                            got: other,
                            span: array.span,
                        });
                        Type::Unknown
                    }
                }
            }

            Expr::New { type_name, args } => {
                let class_info = self.classes.get(type_name).cloned();
                match class_info {
                    None => {
                        self.errors.push(SemanticError::UndefinedClass {
                            name: type_name.clone(),
                            span,
                        });
                        for a in args { self.check_expr(a, env); }
                        Type::Unknown
                    }
                    Some(_) => {
                        // Use effective ctor params (may be inherited from base)
                        let ctor_params = self.effective_ctor_params(type_name);
                        if ctor_params.len() != args.len() {
                            self.errors.push(SemanticError::ArityMismatch {
                                name: format!("new {type_name}"),
                                expected: ctor_params.len(),
                                got: args.len(),
                                span,
                            });
                        }
                        for (arg, expected) in args.iter().zip(ctor_params.iter()) {
                            let arg_ty = self.check_expr(arg, env);
                            if !self.is_subtype(&arg_ty, expected) && !matches!(arg_ty, Type::Unknown) {
                                self.errors.push(SemanticError::TypeMismatch {
                                    expected: expected.clone(),
                                    got: arg_ty,
                                    span: arg.span,
                                });
                            }
                        }
                        for arg in args.iter().skip(ctor_params.len()) {
                            self.check_expr(arg, env);
                        }
                        Type::Named(type_name.clone())
                    }
                }
            }

            Expr::NewArray { type_name, size, init } => {
                let size_ty = self.check_expr(size, env);
                if !self.is_subtype(&size_ty, &Type::Number) && !matches!(size_ty, Type::Unknown) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: Type::Number,
                        got: size_ty,
                        span: size.span,
                    });
                }
                if let Some(init_block) = init {
                    self.check_expr(init_block, env);
                }
                let elem_ty = self.resolve_type(&TypeExpr::Named(type_name.clone()));
                Type::Array(Box::new(elem_ty))
            }

            Expr::Case { expr, arms } => {
                let _ = self.check_expr(expr, env);
                let mut result = Type::Unknown;
                for arm in arms {
                    let arm_ty = self.resolve_type(&arm.type_ann);
                    // Validate arm type exists
                    if let TypeExpr::Named(n) = &arm.type_ann {
                        if !self.classes.contains_key(n.as_str()) && !matches!(arm_ty, Type::Number | Type::Boolean | Type::Str | Type::Object) {
                            self.errors.push(SemanticError::UndefinedType {
                                name: n.clone(),
                                span: arm.span,
                            });
                        }
                    }
                    env.push();
                    env.define(&arm.binding, arm_ty);
                    let body_ty = self.check_expr(&arm.body, env);
                    env.pop();
                    result = self.join(&result, &body_ty);
                }
                result
            }

            Expr::With { expr, binding, body, fallback } => {
                let expr_ty = self.check_expr(expr, env);
                env.push();
                env.define(binding, expr_ty);
                let body_ty = self.check_expr(body, env);
                env.pop();
                let fallback_ty = self.check_expr(fallback, env);
                self.join(&body_ty, &fallback_ty)
            }

            Expr::For { var, iter, body } => {
                let iter_ty = self.check_expr(iter, env);
                // infer element type from array
                let elem_ty = match &iter_ty {
                    Type::Array(elem) => *elem.clone(),
                    Type::Unknown => Type::Unknown,
                    _ => Type::Object,
                };
                env.push();
                env.define(var, elem_ty);
                self.check_expr(body, env);
                env.pop();
                Type::Null
            }

            Expr::IsInstance { expr, .. } => {
                self.check_expr(expr, env);
                Type::Boolean
            }

            Expr::Cast { expr, type_name } => {
                self.check_expr(expr, env);
                // Resolve the cast target type
                if self.classes.contains_key(type_name.as_str()) {
                    Type::Named(type_name.clone())
                } else {
                    match type_name.as_str() {
                        "Number"  => Type::Number,
                        "Boolean" => Type::Boolean,
                        "String"  => Type::Str,
                        "Object"  => Type::Object,
                        _ => Type::Named(type_name.clone()),
                    }
                }
            }

            Expr::VecLit { elements } => {
                let mut elem_ty = Type::Unknown;
                for e in elements {
                    let t = self.check_expr(e, env);
                    elem_ty = self.join(&elem_ty, &t);
                }
                if matches!(elem_ty, Type::Unknown) {
                    elem_ty = Type::Object;
                }
                Type::Array(Box::new(elem_ty))
            }

            Expr::VecComp { body, var, iter } => {
                let iter_ty = self.check_expr(iter, env);
                let elem_ty = match iter_ty {
                    Type::Array(inner) => *inner,
                    _ => Type::Unknown,
                };
                env.push();
                env.define(var.clone(), elem_ty);
                let body_ty = self.check_expr(body, env);
                env.pop();
                Type::Array(Box::new(body_ty))
            }

            Expr::Base { args } => {
                for a in args { self.check_expr(a, env); }
                // Return type unknown statically — would require tracking current method
                Type::Object
            }

            Expr::Lambda { params, body } => {
                env.push();
                for p in params {
                    let ty = p.type_ann.as_ref()
                        .map(|te| self.resolve_type(te))
                        .unwrap_or(Type::Unknown);
                    env.define(p.name.clone(), ty);
                }
                self.check_expr(body, env);
                env.pop();
                // Lambdas are first-class values; treat as Object for now
                Type::Object
            }

            Expr::MacroArgRef(_) | Expr::MacroArgName(_) => {
                // These appear inside macro bodies; treat as Object at type-check time
                Type::Object
            }

            Expr::MacroMatch { subject, cases, default_body } => {
                self.check_expr(subject, env);
                for (pat, body) in cases {
                    self.check_expr(pat, env);
                    self.check_expr(body, env);
                }
                self.check_expr(default_body, env);
                Type::Object
            }
        }
    }

    fn check_binary(&mut self, op: &BinaryOp, lt: &Type, rt: &Type, span: Span) -> Type {
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul |
            BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
                if !self.is_subtype(lt, &Type::Number) && !matches!(lt, Type::Unknown) {
                    self.errors.push(SemanticError::NonNumericOperand {
                        op: format!("{op:?}"), got: lt.clone(), span,
                    });
                }
                if !self.is_subtype(rt, &Type::Number) && !matches!(rt, Type::Unknown) {
                    self.errors.push(SemanticError::NonNumericOperand {
                        op: format!("{op:?}"), got: rt.clone(), span,
                    });
                }
                Type::Number
            }
            BinaryOp::Concat | BinaryOp::ConcatSpace => {
                // HULK coerces any value to String at runtime — only reject if we KNOW it's wrong
                if !self.is_subtype(lt, &Type::Str)
                    && !matches!(lt, Type::Unknown | Type::Object | Type::Number | Type::Boolean | Type::Null)
                {
                    self.errors.push(SemanticError::NonStringConcatArg { got: lt.clone(), span });
                }
                if !self.is_subtype(rt, &Type::Str)
                    && !matches!(rt, Type::Unknown | Type::Object | Type::Number | Type::Boolean | Type::Null)
                {
                    self.errors.push(SemanticError::NonStringConcatArg { got: rt.clone(), span });
                }
                Type::Str
            }
            BinaryOp::Eq | BinaryOp::Ne => Type::Boolean,
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                if !self.is_subtype(lt, &Type::Number) && !matches!(lt, Type::Unknown) {
                    self.errors.push(SemanticError::NonNumericOperand {
                        op: format!("{op:?}"), got: lt.clone(), span,
                    });
                }
                if !self.is_subtype(rt, &Type::Number) && !matches!(rt, Type::Unknown) {
                    self.errors.push(SemanticError::NonNumericOperand {
                        op: format!("{op:?}"), got: rt.clone(), span,
                    });
                }
                Type::Boolean
            }
            BinaryOp::And | BinaryOp::Or => {
                if !self.is_subtype(lt, &Type::Boolean) && !matches!(lt, Type::Unknown) {
                    self.errors.push(SemanticError::NonBooleanOperand {
                        op: format!("{op:?}"), got: lt.clone(), span,
                    });
                }
                if !self.is_subtype(rt, &Type::Boolean) && !matches!(rt, Type::Unknown) {
                    self.errors.push(SemanticError::NonBooleanOperand {
                        op: format!("{op:?}"), got: rt.clone(), span,
                    });
                }
                Type::Boolean
            }
        }
    }
}
