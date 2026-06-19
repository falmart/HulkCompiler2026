use crate::types::Type;
use std::collections::HashMap;

/// Lexically-scoped environment mapping names → Types.
pub struct Env {
    scopes: Vec<HashMap<String, Type>>,
}

impl Env {
    pub fn new() -> Self {
        Self { scopes: vec![HashMap::new()] }
    }

    pub fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop(&mut self) {
        self.scopes.pop();
    }

    /// Define a name in the innermost scope.
    pub fn define(&mut self, name: impl Into<String>, ty: Type) {
        self.scopes.last_mut().unwrap().insert(name.into(), ty);
    }

    /// Look up a name, walking from innermost to outermost scope.
    pub fn lookup(&self, name: &str) -> Option<&Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }
}
