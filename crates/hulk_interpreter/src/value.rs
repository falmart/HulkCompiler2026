use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use hulk_ast::{ExprS, Param};

/// A HULK runtime value.
#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    Boolean(bool),
    Str(String),
    Null,
    Object(Rc<RefCell<HulkObject>>),
    Array(Rc<RefCell<Vec<Value>>>),
    Closure(Rc<ClosureData>),
}

#[derive(Debug, Clone)]
pub struct ClosureData {
    pub params: Vec<Param>,
    pub body: ExprS,
    pub captured: HashMap<String, Value>,
}

/// A heap-allocated HULK object instance.
#[derive(Debug, Clone)]
pub struct HulkObject {
    pub class_name: String,
    /// Instance fields, keyed by name.
    pub fields: HashMap<String, Value>,
}

impl HulkObject {
    pub fn new(class_name: impl Into<String>) -> Self {
        Self { class_name: class_name.into(), fields: HashMap::new() }
    }
}

impl Value {
    pub fn type_name(&self) -> &str {
        match self {
            Value::Number(_)  => "Number",
            Value::Boolean(_) => "Boolean",
            Value::Str(_)     => "String",
            Value::Null       => "Null",
            Value::Object(o)  => {
                // SAFETY: we only read here
                let ptr = o.as_ptr();
                unsafe { &(*ptr).class_name }
            }
            Value::Array(_)   => "Array",
            Value::Closure(_) => "Function",
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Boolean(b) => *b,
            Value::Null       => false,
            _                 => true,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Convert to display string (used by print()).
    pub fn to_display(&self) -> String {
        match self {
            Value::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            Value::Boolean(b) => b.to_string(),
            Value::Str(s)     => s.clone(),
            Value::Null       => "null".into(),
            Value::Object(o)  => format!("[{} object]", o.borrow().class_name),
            Value::Array(a)   => {
                let inner = a.borrow()
                    .iter()
                    .map(|v| v.to_display())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{inner}]")
            }
            Value::Closure(_) => "<function>".into(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_display())
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(a),  Value::Number(b))  => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Str(a),     Value::Str(b))     => a == b,
            (Value::Null,       Value::Null)        => true,
            // Reference equality for objects, arrays, closures
            (Value::Object(a),  Value::Object(b))  => Rc::ptr_eq(a, b),
            (Value::Array(a),   Value::Array(b))   => Rc::ptr_eq(a, b),
            (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
            _                                       => false,
        }
    }
}
