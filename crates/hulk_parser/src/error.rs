use hulk_lexer::{Span, TokenKind};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    Unexpected {
        expected: String,
        got: TokenKind,
        span: Span,
    },
    UnexpectedEof {
        expected: String,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unexpected { expected, got, span } => write!(
                f,
                "[{}:{}] Expected {expected}, got {:?}",
                span.line, span.col, got
            ),
            Self::UnexpectedEof { expected } => {
                write!(f, "Unexpected end of file, expected {expected}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl ParseError {
    pub fn position(&self) -> (u32, u32) {
        match self {
            Self::Unexpected { span, .. } => (span.line, span.col),
            Self::UnexpectedEof { .. }   => (0, 0),
        }
    }

    pub fn clean_message(&self) -> String {
        match self {
            Self::Unexpected { expected, got, .. } =>
                format!("expected {expected}, got {got:?}"),
            Self::UnexpectedEof { expected } =>
                format!("unexpected end of file, expected {expected}"),
        }
    }
}
