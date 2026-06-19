use hulk_ast::*;
use hulk_lexer::{Span, Token, TokenKind};

use crate::error::ParseError;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    /// When true, 'as' is NOT consumed as a cast operator (used inside 'with').
    forbid_as_cast: bool,
    /// When true, '|' is NOT consumed as logical OR (used inside '[...]' to allow vector comprehension).
    forbid_pipe_or: bool,
}

// ── Constructor & token helpers ───────────────────────────────────────────────

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0, forbid_as_cast: false, forbid_pipe_or: false }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek(&self) -> &TokenKind {
        &self.current().kind
    }

    fn span(&self) -> Span {
        self.current().span
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if !self.is_at_end() {
            self.pos += 1;
        }
        tok
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(kind)
    }

    fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind, label: &str) -> Result<Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance().clone())
        } else if self.is_at_end() {
            Err(ParseError::UnexpectedEof { expected: label.into() })
        } else {
            Err(ParseError::Unexpected {
                expected: label.into(),
                got: self.peek().clone(),
                span: self.span(),
            })
        }
    }

    fn expect_ident(&mut self, label: &str) -> Result<(String, Span), ParseError> {
        if let TokenKind::Ident(_) = self.peek() {
            let tok = self.advance().clone();
            if let TokenKind::Ident(name) = tok.kind {
                return Ok((name, tok.span));
            }
        }
        if self.is_at_end() {
            return Err(ParseError::UnexpectedEof { expected: label.into() });
        }
        Err(ParseError::Unexpected {
            expected: label.into(),
            got: self.peek().clone(),
            span: self.span(),
        })
    }
}

// ── Top-level program ─────────────────────────────────────────────────────────

impl Parser {
    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut functions = Vec::new();
        let mut classes = Vec::new();
        let mut top_level_protocols = Vec::new();
        let mut top_exprs: Vec<ExprS> = Vec::new();

        let mut macros = Vec::new();

        while !self.is_at_end() {
            match self.peek() {
                // skip bare semicolons at top level (e.g. after block-body functions)
                TokenKind::Semicolon => { self.advance(); }
                TokenKind::Function => functions.push(self.parse_function_decl()?),
                // 'class' and 'type' both introduce class declarations
                TokenKind::Class | TokenKind::Type => classes.push(self.parse_class_decl()?),
                TokenKind::Protocol => {
                    if let Some(p) = self.parse_protocol_decl()? {
                        top_level_protocols.push(p);
                    }
                }
                // 'def' introduces a macro — parse and skip (not executed)
                TokenKind::Def => {
                    if let Some(m) = self.skip_macro_decl()? {
                        macros.push(m);
                    }
                }
                _ => {
                    let expr = self.parse_expr()?;
                    // Semicolon is optional at top level between expressions
                    if self.check(&TokenKind::Semicolon) {
                        self.advance();
                    }
                    top_exprs.push(expr);
                }
            }
        }

        // Wrap multiple top-level expressions in a Block, or keep single
        let entry = if top_exprs.is_empty() {
            None
        } else if top_exprs.len() == 1 {
            Some(top_exprs.remove(0))
        } else {
            let start = top_exprs.first().unwrap().span;
            let end   = top_exprs.last().unwrap().span;
            let span  = Span::new(start.start, end.end, start.line, start.col);
            Some(Spanned::new(Expr::Block(top_exprs), span))
        };

        Ok(Program { functions, classes, protocols: top_level_protocols, macros, entry })
    }
}

// ── Macro parsing ─────────────────────────────────────────────────────────────

impl Parser {
    /// Parse `def name(params) { body }` into a full MacroDecl.
    fn skip_macro_decl(&mut self) -> Result<Option<MacroDecl>, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::Def, "'def'")?;
        let (name, _) = self.expect_ident("macro name")?;
        self.expect(&TokenKind::LParen, "'('")?;
        let params = self.parse_macro_params()?;
        self.expect(&TokenKind::RParen, "')'")?;
        let body = self.parse_block()?;
        if self.check(&TokenKind::Semicolon) { self.advance(); }
        Ok(Some(MacroDecl { name, params, body, span: start }))
    }

    fn parse_macro_params(&mut self) -> Result<Vec<MacroParam>, ParseError> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let start = self.span();
            // Detect prefix: @, *, $
            let kind = if self.matches(&TokenKind::At) {
                MacroParamKind::ByRef
            } else if self.matches(&TokenKind::Star) {
                MacroParamKind::ByName
            } else if self.matches(&TokenKind::Dollar) {
                MacroParamKind::VarName
            } else {
                MacroParamKind::Value
            };
            let (pname, _) = self.expect_ident("macro parameter name")?;
            let type_ann = if self.matches(&TokenKind::Colon) {
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            let span = Span::new(start.start, self.span().start, start.line, start.col);
            params.push(MacroParam { name: pname, kind, type_ann, span });
            if !self.matches(&TokenKind::Comma) { break; }
        }
        Ok(params)
    }

    /// Parse `match(expr) { case (pat) => body; ... default => body; }`
    fn parse_macro_match(&mut self, start: Span) -> Result<ExprS, ParseError> {
        self.expect(&TokenKind::LParen, "'('")?;
        let subject = self.parse_expr()?;
        self.expect(&TokenKind::RParen, "')'")?;
        self.expect(&TokenKind::LBrace, "'{'")?;
        let mut cases: Vec<(ExprS, ExprS)> = Vec::new();
        let mut default_body: Option<ExprS> = None;
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if self.check(&TokenKind::Case) {
                self.advance();
                self.expect(&TokenKind::LParen, "'(' after case")?;
                let pat = self.parse_expr()?;
                self.expect(&TokenKind::RParen, "')' after pattern")?;
                self.expect(&TokenKind::DArrow, "'=>'")?;
                let body = self.parse_expr()?;
                if self.check(&TokenKind::Semicolon) { self.advance(); }
                cases.push((pat, body));
            } else if self.check(&TokenKind::Default) {
                self.advance();
                self.expect(&TokenKind::DArrow, "'=>'")?;
                let body = self.parse_expr()?;
                if self.check(&TokenKind::Semicolon) { self.advance(); }
                default_body = Some(body);
            } else {
                break;
            }
        }
        let end = self.expect(&TokenKind::RBrace, "'}'")?;
        let span = Span::new(start.start, end.span.end, start.line, start.col);
        let default_body = default_body.unwrap_or_else(|| Spanned::new(Expr::Null, span));
        Ok(Spanned::new(Expr::MacroMatch {
            subject: Box::new(subject),
            cases,
            default_body: Box::new(default_body),
        }, span))
    }
}

// ── Declarations ─────────────────────────────────────────────────────────────

impl Parser {
    fn parse_function_decl(&mut self) -> Result<FunctionDecl, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::Function, "'function'")?;
        let (name, _) = self.expect_ident("function name")?;
        self.expect(&TokenKind::LParen, "'('")?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen, "')'")?;

        let return_type = if self.matches(&TokenKind::Colon) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let body = self.parse_body()?;
        let span = Span::new(start.start, body.span.end, start.line, start.col);
        Ok(FunctionDecl { name, params, return_type, body, span })
    }

    fn parse_class_decl(&mut self) -> Result<ClassDecl, ParseError> {
        let start = self.span();
        // Accept both 'class' and 'type'
        if !self.matches(&TokenKind::Class) {
            self.expect(&TokenKind::Type, "'class' or 'type'")?;
        }
        let (name, _) = self.expect_ident("class name")?;

        // optional constructor params
        let ctor_params = if self.matches(&TokenKind::LParen) {
            let p = self.parse_params()?;
            self.expect(&TokenKind::RParen, "')'")?;
            p
        } else {
            Vec::new()
        };

        // optional base class: 'is BaseClass' or 'inherits BaseClass[(args)]'
        let base = if self.matches(&TokenKind::Is) || self.matches(&TokenKind::Inherits) {
            let (base_name, _) = self.expect_ident("base class name")?;
            // optionally consume base constructor args (not stored)
            if self.matches(&TokenKind::LParen) {
                let _args = self.parse_args()?;
                self.expect(&TokenKind::RParen, "')'")?;
            }
            Some(base_name)
        } else {
            None
        };

        self.expect(&TokenKind::LBrace, "'{'")?;
        let mut members = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            members.push(self.parse_class_member()?);
        }
        let end = self.expect(&TokenKind::RBrace, "'}'")?;
        let span = Span::new(start.start, end.span.end, start.line, start.col);
        Ok(ClassDecl { name, ctor_params, base, members, span })
    }

    /// Parse 'protocol Name [extends P1, P2] { method(params): RetType; ... }'
    fn parse_protocol_decl(&mut self) -> Result<Option<ProtocolDecl>, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::Protocol, "'protocol'")?;
        let (name, _) = self.expect_ident("protocol name")?;

        // optional 'extends Proto1, Proto2, ...'
        let mut extends = Vec::new();
        if let TokenKind::Ident(s) = self.peek().clone() {
            if s == "extends" {
                self.advance(); // consume 'extends'
                loop {
                    let (parent, _) = self.expect_ident("protocol name")?;
                    extends.push(parent);
                    if !self.matches(&TokenKind::Comma) { break; }
                }
            }
        }

        self.expect(&TokenKind::LBrace, "'{'")?;
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let (mname, _) = self.expect_ident("method name")?;
            self.expect(&TokenKind::LParen, "'('")?;
            let params = self.parse_params()?;
            self.expect(&TokenKind::RParen, "')'")?;
            let return_type = if self.matches(&TokenKind::Colon) {
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::Semicolon, "';' after protocol method")?;
            methods.push(ProtocolMethod { name: mname, params, return_type });
        }

        let end = self.expect(&TokenKind::RBrace, "'}'")?;
        let span = Span::new(start.start, end.span.end, start.line, start.col);
        Ok(Some(ProtocolDecl { name, extends, methods, span }))
    }

    fn parse_class_member(&mut self) -> Result<ClassMember, ParseError> {
        let start = self.span();

        // optional 'function' keyword for methods
        if self.matches(&TokenKind::Function) {
            return self.parse_method_member(start);
        }

        let (name, name_span) = self.expect_ident("attribute or method name")?;

        // ':=' or '=' for attribute initializer
        if self.matches(&TokenKind::ColonEq) || self.matches(&TokenKind::Eq) {
            let init = self.parse_expr()?;
            self.expect(&TokenKind::Semicolon, "';' after attribute")?;
            let span = Span::new(start.start, init.span.end, start.line, start.col);
            return Ok(ClassMember::Attribute { name, init, span });
        }

        if self.check(&TokenKind::LParen) {
            // method without 'function' keyword: name(params): T -> body
            return self.parse_named_method(name, name_span, start);
        }

        Err(ParseError::Unexpected {
            expected: "':=', '=' for attribute or '(' for method".into(),
            got: self.peek().clone(),
            span: self.span(),
        })
    }

    fn parse_method_member(&mut self, start: Span) -> Result<ClassMember, ParseError> {
        let (name, name_span) = self.expect_ident("method name")?;
        self.parse_named_method(name, name_span, start)
    }

    fn parse_named_method(
        &mut self,
        name: String,
        _name_span: Span,
        start: Span,
    ) -> Result<ClassMember, ParseError> {
        self.expect(&TokenKind::LParen, "'('")?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen, "')'")?;

        let return_type = if self.matches(&TokenKind::Colon) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let body = self.parse_body()?;
        let span = Span::new(start.start, body.span.end, start.line, start.col);
        Ok(ClassMember::Method { name, params, return_type, body, span })
    }

    // '->' expr ';'  or  '=>' expr ';'  or  block '{}'
    fn parse_body(&mut self) -> Result<ExprS, ParseError> {
        if self.matches(&TokenKind::Arrow) || self.matches(&TokenKind::DArrow) {
            let expr = self.parse_expr()?;
            self.expect(&TokenKind::Semicolon, "';' after inline body")?;
            Ok(expr)
        } else if self.check(&TokenKind::LBrace) {
            self.parse_block()
        } else {
            Err(ParseError::Unexpected {
                expected: "'->', '=>' or '{'".into(),
                got: self.peek().clone(),
                span: self.span(),
            })
        }
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let start = self.span();
            let (name, _) = self.expect_ident("parameter name")?;
            let type_ann = if self.matches(&TokenKind::Colon) {
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            let span = Span::new(start.start, self.span().start, start.line, start.col);
            params.push(Param { name, type_ann, span });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_type_expr(&mut self) -> Result<TypeExpr, ParseError> {
        // Function type: (T1, T2, ...) -> R
        if self.check(&TokenKind::LParen) {
            self.advance();
            let mut params = Vec::new();
            if !self.check(&TokenKind::RParen) {
                params.push(self.parse_type_expr()?);
                while self.matches(&TokenKind::Comma) {
                    params.push(self.parse_type_expr()?);
                }
            }
            self.expect(&TokenKind::RParen, "')' in function type")?;
            self.expect(&TokenKind::Arrow, "'->' in function type")?;
            let ret = self.parse_type_expr()?;
            return Ok(TypeExpr::Function { params, ret: Box::new(ret) });
        }

        let (name, _) = self.expect_ident("type name")?;
        let mut ty = TypeExpr::Named(name);
        loop {
            if self.matches(&TokenKind::LBracket) {
                self.expect(&TokenKind::RBracket, "']' in array type")?;
                ty = TypeExpr::Array(Box::new(ty));
            } else if self.matches(&TokenKind::Star) {
                ty = TypeExpr::Iterable(Box::new(ty));
            } else {
                break;
            }
        }
        Ok(ty)
    }
}

// ── Expressions ───────────────────────────────────────────────────────────────

impl Parser {
    pub fn parse_expr(&mut self) -> Result<ExprS, ParseError> {
        match self.peek() {
            TokenKind::Let   => self.parse_let(),
            TokenKind::If    => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For   => self.parse_for(),
            TokenKind::Case  => self.parse_case(),
            TokenKind::With  => self.parse_with(),
            _                => self.parse_assign(),
        }
    }

    // target := value  (right-associative)
    fn parse_assign(&mut self) -> Result<ExprS, ParseError> {
        let lhs = self.parse_type_ops()?;
        if self.matches(&TokenKind::ColonEq) {
            let rhs = self.parse_assign()?; // right-assoc
            let span = Span::new(lhs.span.start, rhs.span.end, lhs.span.line, lhs.span.col);
            return Ok(Spanned::new(
                Expr::Assign { target: Box::new(lhs), value: Box::new(rhs) },
                span,
            ));
        }
        Ok(lhs)
    }

    // expr is TypeName  /  expr as TypeName
    fn parse_type_ops(&mut self) -> Result<ExprS, ParseError> {
        let mut expr = self.parse_or()?;
        loop {
            if self.matches(&TokenKind::Is) {
                let (type_name, type_span) = self.expect_ident("type name")?;
                let span = Span::new(expr.span.start, type_span.end, expr.span.line, expr.span.col);
                expr = Spanned::new(Expr::IsInstance { expr: Box::new(expr), type_name }, span);
            } else if !self.forbid_as_cast && self.matches(&TokenKind::As) {
                let (type_name, type_span) = self.expect_ident("type name")?;
                let span = Span::new(expr.span.start, type_span.end, expr.span.line, expr.span.col);
                expr = Spanned::new(Expr::Cast { expr: Box::new(expr), type_name }, span);
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_or(&mut self) -> Result<ExprS, ParseError> {
        let mut left = self.parse_and()?;
        while !self.forbid_pipe_or && self.matches(&TokenKind::Pipe) {
            let right = self.parse_and()?;
            let span = merge(left.span, right.span);
            left = Spanned::new(
                Expr::Binary { op: BinaryOp::Or, left: Box::new(left), right: Box::new(right) },
                span,
            );
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<ExprS, ParseError> {
        let mut left = self.parse_equality()?;
        while self.matches(&TokenKind::Amp) {
            let right = self.parse_equality()?;
            let span = merge(left.span, right.span);
            left = Spanned::new(
                Expr::Binary { op: BinaryOp::And, left: Box::new(left), right: Box::new(right) },
                span,
            );
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<ExprS, ParseError> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                TokenKind::EqEq   => BinaryOp::Eq,
                TokenKind::BangEq => BinaryOp::Ne,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            let span = merge(left.span, right.span);
            left = Spanned::new(
                Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                span,
            );
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<ExprS, ParseError> {
        let mut left = self.parse_concat()?;
        loop {
            let op = match self.peek() {
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::Le => BinaryOp::Le,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::Ge => BinaryOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_concat()?;
            let span = merge(left.span, right.span);
            left = Spanned::new(
                Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                span,
            );
        }
        Ok(left)
    }

    fn parse_concat(&mut self) -> Result<ExprS, ParseError> {
        let mut left = self.parse_add()?;
        loop {
            let op = match self.peek() {
                TokenKind::At   => BinaryOp::Concat,
                TokenKind::AtAt => BinaryOp::ConcatSpace,
                _ => break,
            };
            self.advance();
            let right = self.parse_add()?;
            let span = merge(left.span, right.span);
            left = Spanned::new(
                Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                span,
            );
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<ExprS, ParseError> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus  => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul()?;
            let span = merge(left.span, right.span);
            left = Spanned::new(
                Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                span,
            );
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<ExprS, ParseError> {
        let mut left = self.parse_pow()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star    => BinaryOp::Mul,
                TokenKind::Slash   => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_pow()?;
            let span = merge(left.span, right.span);
            left = Spanned::new(
                Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                span,
            );
        }
        Ok(left)
    }

    // Right-associative: a ^ b ^ c  →  a ^ (b ^ c)
    fn parse_pow(&mut self) -> Result<ExprS, ParseError> {
        let base = self.parse_unary()?;
        if self.matches(&TokenKind::Caret) {
            let exp = self.parse_pow()?;
            let span = merge(base.span, exp.span);
            return Ok(Spanned::new(
                Expr::Binary { op: BinaryOp::Pow, left: Box::new(base), right: Box::new(exp) },
                span,
            ));
        }
        Ok(base)
    }

    fn parse_unary(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();
        if self.matches(&TokenKind::Minus) {
            let operand = self.parse_unary()?;
            let span = Span::new(start.start, operand.span.end, start.line, start.col);
            return Ok(Spanned::new(
                Expr::Unary { op: UnaryOp::Neg, operand: Box::new(operand) },
                span,
            ));
        }
        if self.matches(&TokenKind::Bang) {
            let operand = self.parse_unary()?;
            let span = Span::new(start.start, operand.span.end, start.line, start.col);
            return Ok(Spanned::new(
                Expr::Unary { op: UnaryOp::Not, operand: Box::new(operand) },
                span,
            ));
        }
        self.parse_postfix()
    }

    // obj.field  /  obj.method(args)  /  arr[idx]
    fn parse_postfix(&mut self) -> Result<ExprS, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.matches(&TokenKind::Dot) {
                let (member, _) = self.expect_ident("field or method name")?;
                if self.matches(&TokenKind::LParen) {
                    let args = self.parse_args()?;
                    let end = self.expect(&TokenKind::RParen, "')'")?;
                    let span = Span::new(expr.span.start, end.span.end, expr.span.line, expr.span.col);
                    expr = Spanned::new(
                        Expr::MethodCall { object: Box::new(expr), method: member, args },
                        span,
                    );
                } else {
                    let span = Span::new(expr.span.start, self.span().start, expr.span.line, expr.span.col);
                    expr = Spanned::new(
                        Expr::FieldAccess { object: Box::new(expr), field: member },
                        span,
                    );
                }
            } else if self.matches(&TokenKind::LBracket) {
                let index = self.parse_expr()?;
                let end = self.expect(&TokenKind::RBracket, "']'")?;
                let span = Span::new(expr.span.start, end.span.end, expr.span.line, expr.span.col);
                expr = Spanned::new(
                    Expr::Index { array: Box::new(expr), index: Box::new(index) },
                    span,
                );
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();

        // Allow control-flow expressions as sub-expressions (e.g. `x + if (c) 1 else 0`)
        match self.peek() {
            TokenKind::If    => return self.parse_if(),
            TokenKind::Let   => return self.parse_let(),
            TokenKind::While => return self.parse_while(),
            TokenKind::For   => return self.parse_for(),
            TokenKind::Case  => return self.parse_case(),
            TokenKind::With  => return self.parse_with(),
            _ => {}
        }

        match self.peek().clone() {
            TokenKind::Number(n) => {
                self.advance();
                Ok(Spanned::new(Expr::Number(n), start))
            }
            TokenKind::StringLit(s) => {
                self.advance();
                Ok(Spanned::new(Expr::Str(s), start))
            }
            TokenKind::True => {
                self.advance();
                Ok(Spanned::new(Expr::Bool(true), start))
            }
            TokenKind::False => {
                self.advance();
                Ok(Spanned::new(Expr::Bool(false), start))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Spanned::new(Expr::Null, start))
            }
            TokenKind::Self_ => {
                self.advance();
                Ok(Spanned::new(Expr::Self_, start))
            }
            TokenKind::Ident(name) => {
                self.advance();
                // base(args) is a super-method call when 'base' is followed by '('
                if name == "base" && self.check(&TokenKind::LParen) {
                    self.advance(); // consume '('
                    let args = self.parse_args()?;
                    let end = self.expect(&TokenKind::RParen, "')'")?;
                    let span = Span::new(start.start, end.span.end, start.line, start.col);
                    return Ok(Spanned::new(Expr::Base { args }, span));
                }
                // match(...) { ... } — macro pattern match
                if name == "match" && self.check(&TokenKind::LParen) {
                    return self.parse_macro_match(start);
                }
                // Regular function call
                if self.matches(&TokenKind::LParen) {
                    let args = self.parse_args()?;
                    let end = self.expect(&TokenKind::RParen, "')'")?;
                    let span = Span::new(start.start, end.span.end, start.line, start.col);
                    Ok(Spanned::new(Expr::Call { callee: name, args }, span))
                } else {
                    Ok(Spanned::new(Expr::Var(name), start))
                }
            }
            // @ident — by-ref macro argument (only valid inside macro body or call site)
            TokenKind::At => {
                self.advance();
                let (name, name_span) = self.expect_ident("variable name after '@'")?;
                let span = Span::new(start.start, name_span.end, start.line, start.col);
                Ok(Spanned::new(Expr::MacroArgRef(name), span))
            }
            // $ident — variable-name macro argument
            TokenKind::Dollar => {
                self.advance();
                let (name, name_span) = self.expect_ident("variable name after '$'")?;
                let span = Span::new(start.start, name_span.end, start.line, start.col);
                Ok(Spanned::new(Expr::MacroArgName(name), span))
            }
            TokenKind::New => self.parse_new(),
            TokenKind::LParen => {
                // Lookahead: if this looks like (params) => it's a lambda
                if self.is_lambda_start() {
                    return self.parse_lambda();
                }
                self.advance(); // consume '('
                let expr = self.parse_expr()?;
                let end = self.expect(&TokenKind::RParen, "')'")?;
                let span = Span::new(start.start, end.span.end, start.line, start.col);
                Ok(Spanned::new(expr.node, span))
            }
            // Vector literal: [e1, e2, ...]
            TokenKind::LBracket => {
                self.advance();
                if self.check(&TokenKind::RBracket) {
                    // empty vector []
                    let end = self.advance();
                    let span = Span::new(start.start, end.span.end, start.line, start.col);
                    return Ok(Spanned::new(Expr::VecLit { elements: vec![] }, span));
                }
                // Forbid '|' as logical OR inside [...] so comprehension separator works
                let prev_pipe = self.forbid_pipe_or;
                self.forbid_pipe_or = true;
                let first = self.parse_expr()?;
                self.forbid_pipe_or = prev_pipe;
                // Check for vector comprehension: [expr | var in iter]
                if self.matches(&TokenKind::Pipe) {
                    let (var, _) = self.expect_ident("variable name in comprehension")?;
                    self.expect(&TokenKind::In, "'in' in vector comprehension")?;
                    let iter = self.parse_expr()?;
                    let end = self.expect(&TokenKind::RBracket, "']'")?;
                    let span = Span::new(start.start, end.span.end, start.line, start.col);
                    return Ok(Spanned::new(Expr::VecComp { body: Box::new(first), var, iter: Box::new(iter) }, span));
                }
                // Otherwise vector literal
                let mut elements = vec![first];
                while self.matches(&TokenKind::Comma) {
                    elements.push(self.parse_expr()?);
                }
                let end = self.expect(&TokenKind::RBracket, "']'")?;
                let span = Span::new(start.start, end.span.end, start.line, start.col);
                Ok(Spanned::new(Expr::VecLit { elements }, span))
            }
            TokenKind::LBrace => self.parse_block(),
            _ => {
                if self.is_at_end() {
                    Err(ParseError::UnexpectedEof { expected: "expression".into() })
                } else {
                    Err(ParseError::Unexpected {
                        expected: "expression".into(),
                        got: self.peek().clone(),
                        span: self.span(),
                    })
                }
            }
        }
    }

    /// Returns true if the current position starts a lambda: `(ident [: Type] [, ...]  ) =>`
    fn is_lambda_start(&self) -> bool {
        let mut i = self.pos;
        // must start with '('
        if !matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::LParen)) {
            return false;
        }
        i += 1;
        // empty lambda () => is also valid
        if matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::RParen)) {
            i += 1;
            return matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::DArrow));
        }
        // each param: ident [: Type]  separated by commas
        loop {
            // must have an ident
            if !matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::Ident(_))) {
                return false;
            }
            i += 1;
            // optional ': Type'  (Type can be 'Ident' or '(' for function type)
            if matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::Colon)) {
                i += 1;
                // skip the type: consume until comma or RParen (simple scan)
                let mut depth = 0usize;
                loop {
                    match self.tokens.get(i).map(|t| &t.kind) {
                        Some(TokenKind::LParen) => { depth += 1; i += 1; }
                        Some(TokenKind::RParen) if depth > 0 => { depth -= 1; i += 1; }
                        Some(TokenKind::RParen) | Some(TokenKind::Comma) | None => break,
                        Some(TokenKind::Star) => { i += 1; } // T*
                        _ => { i += 1; }
                    }
                }
            }
            // comma or end of params
            match self.tokens.get(i).map(|t| &t.kind) {
                Some(TokenKind::Comma) => { i += 1; }
                Some(TokenKind::RParen) => { i += 1; break; }
                _ => return false,
            }
        }
        // after ')' must be '=>'
        matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::DArrow))
    }

    fn parse_lambda(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::LParen, "'('")?;
        let params = if self.check(&TokenKind::RParen) {
            vec![]
        } else {
            self.parse_params()?
        };
        self.expect(&TokenKind::RParen, "')'")?;
        self.expect(&TokenKind::DArrow, "'=>' in lambda")?;
        let body = self.parse_expr()?;
        let span = Span::new(start.start, body.span.end, start.line, start.col);
        Ok(Spanned::new(Expr::Lambda { params, body: Box::new(body) }, span))
    }

    fn parse_args(&mut self) -> Result<Vec<ExprS>, ParseError> {
        let mut args = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        Ok(args)
    }
}

// ── Special expression forms ──────────────────────────────────────────────────

impl Parser {
    fn parse_let(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::Let, "'let'")?;

        let mut bindings = Vec::new();
        loop {
            let b_start = self.span();
            let (name, _) = self.expect_ident("variable name")?;
            let type_ann = if self.matches(&TokenKind::Colon) {
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::Eq, "'='")?;
            let init = self.parse_expr()?;
            let b_end = init.span.end;
            bindings.push(Binding {
                name,
                type_ann,
                init,
                span: Span::new(b_start.start, b_end, b_start.line, b_start.col),
            });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }

        self.expect(&TokenKind::In, "'in'")?;
        let body = self.parse_expr()?;
        let span = Span::new(start.start, body.span.end, start.line, start.col);
        Ok(Spanned::new(Expr::Let { bindings, body: Box::new(body) }, span))
    }

    fn parse_if(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::If, "'if'")?;
        self.expect(&TokenKind::LParen, "'('")?;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::RParen, "')'")?;
        let then = self.parse_expr()?;

        let mut elif_branches = Vec::new();
        while self.matches(&TokenKind::Elif) {
            self.expect(&TokenKind::LParen, "'('")?;
            let elif_cond = self.parse_expr()?;
            self.expect(&TokenKind::RParen, "')'")?;
            let elif_body = self.parse_expr()?;
            elif_branches.push((elif_cond, elif_body));
        }

        let else_branch = if self.matches(&TokenKind::Else) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        let end = else_branch
            .as_ref()
            .map(|e| e.span.end)
            .or_else(|| elif_branches.last().map(|(_, b)| b.span.end))
            .unwrap_or(then.span.end);

        let span = Span::new(start.start, end, start.line, start.col);
        Ok(Spanned::new(
            Expr::If {
                cond: Box::new(cond),
                then: Box::new(then),
                elif_branches,
                else_branch,
            },
            span,
        ))
    }

    fn parse_while(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::While, "'while'")?;
        self.expect(&TokenKind::LParen, "'('")?;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::RParen, "')'")?;
        let body = self.parse_expr()?;
        let span = Span::new(start.start, body.span.end, start.line, start.col);
        Ok(Spanned::new(Expr::While { cond: Box::new(cond), body: Box::new(body) }, span))
    }

    // for (var in iter) body
    fn parse_for(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::For, "'for'")?;
        self.expect(&TokenKind::LParen, "'('")?;
        let (var, _) = self.expect_ident("loop variable")?;
        self.expect(&TokenKind::In, "'in'")?;
        let iter = self.parse_expr()?;
        self.expect(&TokenKind::RParen, "')'")?;
        let body = self.parse_expr()?;
        let span = Span::new(start.start, body.span.end, start.line, start.col);
        Ok(Spanned::new(
            Expr::For { var, iter: Box::new(iter), body: Box::new(body) },
            span,
        ))
    }

    fn parse_case(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::Case, "'case'")?;
        let expr = self.parse_expr()?;
        self.expect(&TokenKind::Of, "'of'")?;
        self.expect(&TokenKind::LBrace, "'{'")?;

        let mut arms = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let arm_start = self.span();
            let (binding, _) = self.expect_ident("binding name")?;
            self.expect(&TokenKind::Colon, "':'")?;
            let type_ann = self.parse_type_expr()?;
            self.expect(&TokenKind::Arrow, "'->'")?;
            let body = self.parse_expr()?;
            self.expect(&TokenKind::Semicolon, "';'")?;
            let arm_span = Span::new(arm_start.start, body.span.end, arm_start.line, arm_start.col);
            arms.push(CaseArm { binding, type_ann, body, span: arm_span });
        }

        let end = self.expect(&TokenKind::RBrace, "'}'")?;
        let span = Span::new(start.start, end.span.end, start.line, start.col);
        Ok(Spanned::new(Expr::Case { expr: Box::new(expr), arms }, span))
    }

    fn parse_with(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::With, "'with'")?;
        self.expect(&TokenKind::LParen, "'('")?;
        // Parse expression WITHOUT consuming 'as' as a cast operator
        let prev = self.forbid_as_cast;
        self.forbid_as_cast = true;
        let expr = self.parse_expr()?;
        self.forbid_as_cast = prev;
        self.expect(&TokenKind::As, "'as'")?;
        let (binding, _) = self.expect_ident("binding name")?;
        self.expect(&TokenKind::RParen, "')'")?;
        let body = self.parse_expr()?;
        self.expect(&TokenKind::Else, "'else'")?;
        let fallback = self.parse_expr()?;
        let span = Span::new(start.start, fallback.span.end, start.line, start.col);
        Ok(Spanned::new(
            Expr::With {
                expr: Box::new(expr),
                binding,
                body: Box::new(body),
                fallback: Box::new(fallback),
            },
            span,
        ))
    }

    fn parse_new(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::New, "'new'")?;
        let (type_name, _) = self.expect_ident("type name")?;

        if self.matches(&TokenKind::LBracket) {
            // new T[size] [{init}]
            let size = self.parse_expr()?;
            let end = self.expect(&TokenKind::RBracket, "']'")?;
            let init = if self.check(&TokenKind::LBrace) {
                let block = self.parse_block()?;
                Some(Box::new(block))
            } else {
                None
            };
            let end_pos = init.as_ref().map(|b| b.span.end).unwrap_or(end.span.end);
            let span = Span::new(start.start, end_pos, start.line, start.col);
            return Ok(Spanned::new(
                Expr::NewArray { type_name, size: Box::new(size), init },
                span,
            ));
        }

        // new T(args)
        self.expect(&TokenKind::LParen, "'('")?;
        let args = self.parse_args()?;
        let end = self.expect(&TokenKind::RParen, "')'")?;
        let span = Span::new(start.start, end.span.end, start.line, start.col);
        Ok(Spanned::new(Expr::New { type_name, args }, span))
    }

    // { (expr ;)* expr? }
    fn parse_block(&mut self) -> Result<ExprS, ParseError> {
        let start = self.span();
        self.expect(&TokenKind::LBrace, "'{'")?;

        let mut stmts: Vec<ExprS> = Vec::new();

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let expr = self.parse_expr()?;
            if self.matches(&TokenKind::Semicolon) {
                stmts.push(expr);
            } else {
                // trailing expr without ';' — last value
                stmts.push(expr);
                break;
            }
        }

        let end = self.expect(&TokenKind::RBrace, "'}'")?;
        let span = Span::new(start.start, end.span.end, start.line, start.col);
        Ok(Spanned::new(Expr::Block(stmts), span))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn merge(a: Span, b: Span) -> Span {
    Span::new(a.start, b.end, a.line, a.col)
}
