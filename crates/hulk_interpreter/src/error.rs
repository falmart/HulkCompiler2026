use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    UndefinedVariable   { name: String },
    UndefinedFunction   { name: String },
    UndefinedMethod     { type_name: String, method: String },
    UndefinedField      { type_name: String, field: String },
    UndefinedClass      { name: String },
    TypeMismatch        { expected: String, got: String },
    DivisionByZero,
    IndexOutOfBounds    { index: i64, len: usize },
    InvalidAssignTarget,
    StackOverflow,
    Custom(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndefinedVariable { name }    => write!(f, "Undefined variable '{name}'"),
            Self::UndefinedFunction { name }    => write!(f, "Undefined function '{name}'"),
            Self::UndefinedMethod { type_name, method } =>
                write!(f, "'{type_name}' has no method '{method}'"),
            Self::UndefinedField { type_name, field } =>
                write!(f, "'{type_name}' has no field '{field}'"),
            Self::UndefinedClass { name }       => write!(f, "Undefined class '{name}'"),
            Self::TypeMismatch { expected, got } =>
                write!(f, "Type mismatch: expected {expected}, got {got}"),
            Self::DivisionByZero                => write!(f, "Division by zero"),
            Self::IndexOutOfBounds { index, len } =>
                write!(f, "Index {index} out of bounds (len={len})"),
            Self::InvalidAssignTarget           => write!(f, "Invalid assignment target"),
            Self::StackOverflow                 => write!(f, "Stack overflow"),
            Self::Custom(msg)                   => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for RuntimeError {}
