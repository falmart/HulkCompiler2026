/// A position in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub col: u32,
}

impl Span {
    pub fn new(start: usize, end: usize, line: u32, col: u32) -> Self {
        Self { start, end, line, col }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// Raw source slice (interned as String for simplicity).
    pub lexeme: String,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span, lexeme: impl Into<String>) -> Self {
        Self { kind, span, lexeme: lexeme.into() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ── Literals ────────────────────────────────────────────────────────────
    Number(f64),
    StringLit(String),
    True,
    False,

    // ── Identifiers ─────────────────────────────────────────────────────────
    Ident(String),

    // ── Keywords ────────────────────────────────────────────────────────────
    Let,
    In,
    If,
    Elif,
    Else,
    While,
    For,
    Function,
    Class,
    Type,     // 'type' keyword (alias for class)
    Is,       // inheritance: class Foo is Bar / type-check: expr is T
    Inherits, // 'inherits' (alias for 'is' in class declaration)
    New,
    Self_,
    Case,
    Of,
    With,
    As,
    Null,
    Protocol,  // 'protocol' keyword
    Interface, // 'interface' keyword (alias for protocol)
    Def,       // 'def' keyword (macro definitions)
    Define,    // 'define' keyword (macro/function shorthand)
    Default,   // 'default' keyword (macro match fallback)
    Dollar,    // $ (macro captured-variable prefix)

    // ── Arithmetic operators ─────────────────────────────────────────────────
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Percent,  // %
    Caret,    // ^ (power)

    // ── String operators ─────────────────────────────────────────────────────
    At,       // @  (concat)
    AtAt,     // @@ (concat with space)

    // ── Comparison operators ─────────────────────────────────────────────────
    Lt,       // <
    Le,       // <=
    Gt,       // >
    Ge,       // >=
    EqEq,     // ==
    BangEq,   // !=

    // ── Logical operators ────────────────────────────────────────────────────
    Amp,      // &
    Pipe,     // |
    Bang,     // !

    // ── Assignment / binding ─────────────────────────────────────────────────
    Eq,       // =
    Arrow,    // ->  (function body)
    DArrow,   // =>  (fat arrow, alternative body syntax)
    ColonEq,  // :=  (destructive assignment)

    // ── Punctuation ──────────────────────────────────────────────────────────
    LParen,   // (
    RParen,   // )
    LBrace,   // {
    RBrace,   // }
    LBracket, // [
    RBracket, // ]
    Comma,    // ,
    Semicolon,// ;
    Colon,    // :
    Dot,      // .
    Pipe2,    // | inside vector comprehension (reuse Pipe context-sensitively)

    // ── Special ──────────────────────────────────────────────────────────────
    Eof,
}
