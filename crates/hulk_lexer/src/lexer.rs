use crate::token::{Span, Token, TokenKind};
use crate::LexError;

pub struct Lexer<'src> {
    src: &'src str,
    chars: std::str::CharIndices<'src>,
    /// Next character to examine (peeked).
    peeked: Option<(usize, char)>,
    pos: usize,
    line: u32,
    col: u32,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        let mut chars = src.char_indices();
        let peeked = chars.next();
        Self { src, chars, peeked, pos: 0, line: 1, col: 1 }
    }

    // ── Advance ─────────────────────────────────────────────────────────────

    fn advance(&mut self) -> Option<char> {
        let (idx, ch) = self.peeked?;
        self.pos = idx;
        self.peeked = self.chars.next();
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    fn peek(&self) -> Option<char> {
        self.peeked.map(|(_, c)| c)
    }

    fn peek_offset(&self) -> usize {
        self.peeked.map(|(i, _)| i).unwrap_or(self.src.len())
    }

    fn matches(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    // ── Skip whitespace & comments ───────────────────────────────────────────

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => { self.advance(); }
                Some('/') => {
                    // peek two chars ahead by reading src directly
                    let next_idx = self.peek_offset();
                    if self.src[next_idx..].starts_with("//") {
                        // line comment
                        while self.peek().map(|c| c != '\n').unwrap_or(false) {
                            self.advance();
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
    }

    // ── Tokenise a number ────────────────────────────────────────────────────

    fn read_number(&mut self, start: usize, start_line: u32, start_col: u32) -> Result<Token, LexError> {
        while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            self.advance();
        }
        // optional decimal part
        if self.peek() == Some('.') {
            let dot_idx = self.peek_offset();
            // check the char after the dot is a digit
            let after_dot = self.src[dot_idx + 1..].chars().next();
            if after_dot.map(|c| c.is_ascii_digit()).unwrap_or(false) {
                self.advance(); // consume '.'
                while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    self.advance();
                }
            }
        }
        let end = self.peek_offset();
        let lexeme = &self.src[start..end];
        let value: f64 = lexeme.parse().map_err(|_| LexError::InvalidNumber {
            lexeme: lexeme.to_string(),
            line: start_line,
            col: start_col,
        })?;
        let span = Span::new(start, end, start_line, start_col);
        Ok(Token::new(TokenKind::Number(value), span, lexeme))
    }

    // ── Tokenise a string literal ────────────────────────────────────────────

    fn read_string(&mut self, start: usize, start_line: u32, start_col: u32) -> Result<Token, LexError> {
        let mut value = String::new();
        loop {
            match self.advance() {
                None | Some('\n') => return Err(LexError::UnterminatedString {
                    line: start_line,
                    col: start_col,
                }),
                Some('"') => break,
                Some('\\') => match self.advance() {
                    Some('n')  => value.push('\n'),
                    Some('t')  => value.push('\t'),
                    Some('\\') => value.push('\\'),
                    Some('"')  => value.push('"'),
                    Some(c) => return Err(LexError::UnknownEscape { ch: c, line: self.line, col: self.col }),
                    None => return Err(LexError::UnterminatedString { line: start_line, col: start_col }),
                },
                Some(c) => value.push(c),
            }
        }
        let end = self.peek_offset();
        let lexeme = &self.src[start..end];
        let span = Span::new(start, end, start_line, start_col);
        Ok(Token::new(TokenKind::StringLit(value), span, lexeme))
    }

    // ── Tokenise an identifier or keyword ───────────────────────────────────

    fn read_ident(&mut self, start: usize, start_line: u32, start_col: u32) -> Token {
        while self.peek().map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false) {
            self.advance();
        }
        let end = self.peek_offset();
        let lexeme = &self.src[start..end];
        let kind = keyword(lexeme).unwrap_or_else(|| TokenKind::Ident(lexeme.to_string()));
        Token::new(kind, Span::new(start, end, start_line, start_col), lexeme)
    }

    // ── Public scan entry ────────────────────────────────────────────────────

    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace_and_comments();

        let start_line = self.line;
        let start_col = self.col;
        let start = self.peek_offset();

        let ch = match self.advance() {
            None => {
                let span = Span::new(start, start, start_line, start_col);
                return Ok(Token::new(TokenKind::Eof, span, ""));
            }
            Some(c) => c,
        };

        macro_rules! tok {
            ($kind:expr, $lex:expr) => {{
                let end = self.peek_offset();
                Token::new($kind, Span::new(start, end, start_line, start_col), $lex)
            }};
        }

        let token = match ch {
            // ── Single-char punctuation ──────────────────────────────────────
            '(' => tok!(TokenKind::LParen,    "("),
            ')' => tok!(TokenKind::RParen,    ")"),
            '{' => tok!(TokenKind::LBrace,    "{"),
            '}' => tok!(TokenKind::RBrace,    "}"),
            '[' => tok!(TokenKind::LBracket,  "["),
            ']' => tok!(TokenKind::RBracket,  "]"),
            ',' => tok!(TokenKind::Comma,     ","),
            ';' => tok!(TokenKind::Semicolon, ";"),
            '.' => tok!(TokenKind::Dot,       "."),
            '%' => tok!(TokenKind::Percent,   "%"),
            '^' => tok!(TokenKind::Caret,     "^"),
            '&' => tok!(TokenKind::Amp,       "&"),
            '|' => tok!(TokenKind::Pipe,      "|"),

            // ── Two-char possibilities ───────────────────────────────────────
            ':' => {
                if self.matches('=') { tok!(TokenKind::ColonEq, ":=") }
                else { tok!(TokenKind::Colon, ":") }
            }
            '!' => {
                if self.matches('=') { tok!(TokenKind::BangEq, "!=") }
                else { tok!(TokenKind::Bang, "!") }
            }
            '=' => {
                if self.matches('=') { tok!(TokenKind::EqEq, "==") }
                else if self.matches('>') { tok!(TokenKind::DArrow, "=>") }
                else { tok!(TokenKind::Eq, "=") }
            }
            '<' => {
                if self.matches('=') { tok!(TokenKind::Le, "<=") }
                else { tok!(TokenKind::Lt, "<") }
            }
            '>' => {
                if self.matches('=') { tok!(TokenKind::Ge, ">=") }
                else { tok!(TokenKind::Gt, ">") }
            }
            '-' => {
                if self.matches('>') { tok!(TokenKind::Arrow, "->") }
                else { tok!(TokenKind::Minus, "-") }
            }
            '+' => tok!(TokenKind::Plus, "+"),
            '*' => tok!(TokenKind::Star, "*"),
            '/' => tok!(TokenKind::Slash, "/"),

            '@' => {
                if self.matches('@') { tok!(TokenKind::AtAt, "@@") }
                else { tok!(TokenKind::At, "@") }
            }

            // ── Literals ─────────────────────────────────────────────────────
            '"' => self.read_string(start, start_line, start_col)?,

            c if c.is_ascii_digit() => self.read_number(start, start_line, start_col)?,

            c if c.is_alphabetic() || c == '_' => self.read_ident(start, start_line, start_col),

            '$' => tok!(TokenKind::Dollar, "$"),
            c => return Err(LexError::UnexpectedChar { ch: c, line: start_line, col: start_col }),
        };

        Ok(token)
    }

    /// Consume all tokens into a Vec. Stops at EOF (included) or first error.
    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof { break; }
        }
        Ok(tokens)
    }
}

fn keyword(s: &str) -> Option<TokenKind> {
    Some(match s {
        "let"      => TokenKind::Let,
        "in"       => TokenKind::In,
        "if"       => TokenKind::If,
        "elif"     => TokenKind::Elif,
        "else"     => TokenKind::Else,
        "while"    => TokenKind::While,
        "for"      => TokenKind::For,
        "function" => TokenKind::Function,
        "class"    => TokenKind::Class,
        "type"     => TokenKind::Type,
        "is"       => TokenKind::Is,
        "inherits" => TokenKind::Inherits,
        "new"      => TokenKind::New,
        "self"     => TokenKind::Self_,
        "case"     => TokenKind::Case,
        "of"       => TokenKind::Of,
        "with"     => TokenKind::With,
        "as"       => TokenKind::As,
        "null"     => TokenKind::Null,
        "true"     => TokenKind::True,
        "false"    => TokenKind::False,
        "protocol" => TokenKind::Protocol,
        "def"      => TokenKind::Def,
        "default"  => TokenKind::Default,
        _          => return None,
    })
}
