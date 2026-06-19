use crate::value::Value;
use std::collections::HashMap;

/// Lexically-scoped runtime environment.
pub struct Env {
    scopes: Vec<HashMap<String, Value>>,
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

    /// Define a new binding in the innermost scope.
    pub fn define(&mut self, name: impl Into<String>, val: Value) {
        self.scopes.last_mut().unwrap().insert(name.into(), val);
    }

    /// Look up a name from inner to outer scope.
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    /// Flatten all scopes into a single map (inner scopes shadow outer).
    pub fn snapshot(&self) -> std::collections::HashMap<String, Value> {
        let mut map = std::collections::HashMap::new();
        for scope in &self.scopes {
            for (k, v) in scope {
                map.insert(k.clone(), v.clone());
            }
        }
        map
    }

    /// Mutate an existing binding (closest scope wins).
    /// Returns false if the name is not found anywhere.
    pub fn assign(&mut self, name: &str, val: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), val);
                return true;
            }
        }
        false
    }
}
