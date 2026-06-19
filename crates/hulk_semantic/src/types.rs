/// The resolved type of an expression or binding.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number,
    Boolean,
    Str,             // String
    Object,          // root of hierarchy
    Null,
    Named(String),   // user-defined class
    Array(Box<Type>),
    Unknown,         // error-recovery placeholder
}

impl Type {
    pub fn name(&self) -> String {
        match self {
            Type::Number       => "Number".into(),
            Type::Boolean      => "Boolean".into(),
            Type::Str          => "String".into(),
            Type::Object       => "Object".into(),
            Type::Null         => "Null".into(),
            Type::Named(n)     => n.clone(),
            Type::Array(t)     => format!("{}[]", t.name()),
            Type::Unknown      => "<unknown>".into(),
        }
    }

    /// True for primitive scalar types that cannot hold Null.
    pub fn is_primitive(&self) -> bool {
        matches!(self, Type::Number | Type::Boolean)
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
