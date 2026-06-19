use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    UnexpectedChar { ch: char, line: u32, col: u32 },
    UnterminatedString { line: u32, col: u32 },
    UnknownEscape { ch: char, line: u32, col: u32 },
    InvalidNumber { lexeme: String, line: u32, col: u32 },
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedChar { ch, line, col } =>
                write!(f, "[{line}:{col}] Unexpected character '{ch}'"),
            Self::UnterminatedString { line, col } =>
                write!(f, "[{line}:{col}] Unterminated string literal"),
            Self::UnknownEscape { ch, line, col } =>
                write!(f, "[{line}:{col}] Unknown escape sequence '\\{ch}'"),
            Self::InvalidNumber { lexeme, line, col } =>
                write!(f, "[{line}:{col}] Invalid number literal '{lexeme}'"),
        }
    }
}

impl std::error::Error for LexError {}

impl LexError {
    pub fn position(&self) -> (u32, u32) {
        match self {
            Self::UnexpectedChar      { line, col, .. } => (*line, *col),
            Self::UnterminatedString  { line, col }     => (*line, *col),
            Self::UnknownEscape       { line, col, .. } => (*line, *col),
            Self::InvalidNumber       { line, col, .. } => (*line, *col),
        }
    }

    pub fn clean_message(&self) -> String {
        match self {
            Self::UnexpectedChar     { ch, .. }      => format!("unexpected character '{ch}'"),
            Self::UnterminatedString { .. }          => "unterminated string literal".into(),
            Self::UnknownEscape      { ch, .. }      => format!("unknown escape sequence '\\{ch}'"),
            Self::InvalidNumber      { lexeme, .. }  => format!("invalid number literal '{lexeme}'"),
        }
    }
}
