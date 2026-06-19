use crate::types::Type;
use hulk_lexer::Span;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticError {
    UndefinedVariable     { name: String, span: Span },
    UndefinedFunction     { name: String, span: Span },
    UndefinedType         { name: String, span: Span },
    UndefinedClass        { name: String, span: Span },
    UndefinedMethod       { type_name: String, method: String, span: Span },
    UndefinedField        { type_name: String, field: String, span: Span },
    TypeMismatch          { expected: Type, got: Type, span: Span },
    ArityMismatch         { name: String, expected: usize, got: usize, span: Span },
    InvalidAssignTarget   { span: Span },
    CircularInheritance   { class: String, span: Span },
    DuplicateDeclaration  { name: String, span: Span },
    ConditionNotBoolean   { got: Type, span: Span },
    NonNumericOperand     { op: String, got: Type, span: Span },
    NonBooleanOperand     { op: String, got: Type, span: Span },
    NonStringConcatArg    { got: Type, span: Span },
    NullAssignedToPrimitive { target_type: Type, span: Span },
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UndefinedVariable { name, span } =>
                write!(f, "[{}:{}] Undefined variable '{name}'", span.line, span.col),
            Self::UndefinedFunction { name, span } =>
                write!(f, "[{}:{}] Undefined function '{name}'", span.line, span.col),
            Self::UndefinedType { name, span } =>
                write!(f, "[{}:{}] Undefined type '{name}'", span.line, span.col),
            Self::UndefinedClass { name, span } =>
                write!(f, "[{}:{}] Undefined class '{name}'", span.line, span.col),
            Self::UndefinedMethod { type_name, method, span } =>
                write!(f, "[{}:{}] Type '{type_name}' has no method '{method}'", span.line, span.col),
            Self::UndefinedField { type_name, field, span } =>
                write!(f, "[{}:{}] Type '{type_name}' has no field '{field}'", span.line, span.col),
            Self::TypeMismatch { expected, got, span } =>
                write!(f, "[{}:{}] Type mismatch: expected '{expected}', got '{got}'", span.line, span.col),
            Self::ArityMismatch { name, expected, got, span } =>
                write!(f, "[{}:{}] '{name}' expects {expected} argument(s), got {got}", span.line, span.col),
            Self::InvalidAssignTarget { span } =>
                write!(f, "[{}:{}] Invalid assignment target", span.line, span.col),
            Self::CircularInheritance { class, span } =>
                write!(f, "[{}:{}] Circular inheritance detected in class '{class}'", span.line, span.col),
            Self::DuplicateDeclaration { name, span } =>
                write!(f, "[{}:{}] Duplicate declaration of '{name}'", span.line, span.col),
            Self::ConditionNotBoolean { got, span } =>
                write!(f, "[{}:{}] Condition must be Boolean, got '{got}'", span.line, span.col),
            Self::NonNumericOperand { op, got, span } =>
                write!(f, "[{}:{}] Operator '{op}' requires Number, got '{got}'", span.line, span.col),
            Self::NonBooleanOperand { op, got, span } =>
                write!(f, "[{}:{}] Operator '{op}' requires Boolean, got '{got}'", span.line, span.col),
            Self::NonStringConcatArg { got, span } =>
                write!(f, "[{}:{}] String concatenation requires String, got '{got}'", span.line, span.col),
            Self::NullAssignedToPrimitive { target_type, span } =>
                write!(f, "[{}:{}] Cannot assign Null to '{target_type}'", span.line, span.col),
        }
    }
}

impl std::error::Error for SemanticError {}

impl SemanticError {
    pub fn position(&self) -> (u32, u32) {
        let span = match self {
            Self::UndefinedVariable     { span, .. } => span,
            Self::UndefinedFunction     { span, .. } => span,
            Self::UndefinedType         { span, .. } => span,
            Self::UndefinedClass        { span, .. } => span,
            Self::UndefinedMethod       { span, .. } => span,
            Self::UndefinedField        { span, .. } => span,
            Self::TypeMismatch          { span, .. } => span,
            Self::ArityMismatch         { span, .. } => span,
            Self::InvalidAssignTarget   { span }     => span,
            Self::CircularInheritance   { span, .. } => span,
            Self::DuplicateDeclaration  { span, .. } => span,
            Self::ConditionNotBoolean   { span, .. } => span,
            Self::NonNumericOperand     { span, .. } => span,
            Self::NonBooleanOperand     { span, .. } => span,
            Self::NonStringConcatArg    { span, .. } => span,
            Self::NullAssignedToPrimitive { span, .. } => span,
        };
        (span.line, span.col)
    }

    pub fn clean_message(&self) -> String {
        match self {
            Self::UndefinedVariable { name, .. } =>
                format!("undefined variable '{name}'"),
            Self::UndefinedFunction { name, .. } =>
                format!("undefined function '{name}'"),
            Self::UndefinedType { name, .. } =>
                format!("undefined type '{name}'"),
            Self::UndefinedClass { name, .. } =>
                format!("undefined class '{name}'"),
            Self::UndefinedMethod { type_name, method, .. } =>
                format!("type '{type_name}' has no method '{method}'"),
            Self::UndefinedField { type_name, field, .. } =>
                format!("type '{type_name}' has no field '{field}'"),
            Self::TypeMismatch { expected, got, .. } =>
                format!("type mismatch — expected {expected}, got {got}"),
            Self::ArityMismatch { name, expected, got, .. } =>
                format!("'{name}' expects {expected} argument(s), got {got}"),
            Self::InvalidAssignTarget { .. } =>
                "invalid assignment target".into(),
            Self::CircularInheritance { class, .. } =>
                format!("circular inheritance detected in class '{class}'"),
            Self::DuplicateDeclaration { name, .. } =>
                format!("duplicate declaration of '{name}'"),
            Self::ConditionNotBoolean { got, .. } =>
                format!("condition must be Boolean, got {got}"),
            Self::NonNumericOperand { op, got, .. } =>
                format!("operator '{op}' requires Number, got {got}"),
            Self::NonBooleanOperand { op, got, .. } =>
                format!("operator '{op}' requires Boolean, got {got}"),
            Self::NonStringConcatArg { got, .. } =>
                format!("string concatenation requires String, got {got}"),
            Self::NullAssignedToPrimitive { target_type, .. } =>
                format!("cannot assign null to '{target_type}'"),
        }
    }
}
