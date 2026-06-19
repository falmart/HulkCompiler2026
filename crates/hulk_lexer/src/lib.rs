pub mod error;
pub mod lexer;
pub mod token;

pub use error::LexError;
pub use lexer::Lexer;
pub use token::{Span, Token, TokenKind};

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn lex(src: &str) -> Vec<TokenKind> {
        Lexer::new(src)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    fn lex_full(src: &str) -> Vec<Token> {
        Lexer::new(src).tokenize().unwrap()
    }

    fn lex_err(src: &str) -> LexError {
        Lexer::new(src).tokenize().unwrap_err()
    }

    fn kinds_no_eof(src: &str) -> Vec<TokenKind> {
        let mut v = lex(src);
        assert_eq!(v.last(), Some(&TokenKind::Eof));
        v.pop();
        v
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 1. NUMBER LITERALS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn number_integer() {
        assert_eq!(kinds_no_eof("0"),    vec![TokenKind::Number(0.0)]);
        assert_eq!(kinds_no_eof("42"),   vec![TokenKind::Number(42.0)]);
        assert_eq!(kinds_no_eof("1000"), vec![TokenKind::Number(1000.0)]);
    }

    #[test]
    fn number_float() {
        assert_eq!(kinds_no_eof("3.14"),  vec![TokenKind::Number(3.14)]);
        assert_eq!(kinds_no_eof("0.5"),   vec![TokenKind::Number(0.5)]);
        assert_eq!(kinds_no_eof("100.0"), vec![TokenKind::Number(100.0)]);
    }

    #[test]
    fn number_dot_not_float_when_no_digit_after() {
        // "1." should lex as Number(1) followed by Dot, not a float
        let k = kinds_no_eof("1.");
        assert_eq!(k[0], TokenKind::Number(1.0));
        assert_eq!(k[1], TokenKind::Dot);
    }

    #[test]
    fn number_multiple_on_same_line() {
        let k = kinds_no_eof("1 2 3");
        assert_eq!(k, vec![
            TokenKind::Number(1.0),
            TokenKind::Number(2.0),
            TokenKind::Number(3.0),
        ]);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 2. STRING LITERALS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn string_empty() {
        assert_eq!(kinds_no_eof(r#""""#), vec![TokenKind::StringLit("".into())]);
    }

    #[test]
    fn string_plain() {
        assert_eq!(kinds_no_eof(r#""hello""#), vec![TokenKind::StringLit("hello".into())]);
    }

    #[test]
    fn string_with_spaces() {
        assert_eq!(
            kinds_no_eof(r#""hello world""#),
            vec![TokenKind::StringLit("hello world".into())]
        );
    }

    #[test]
    fn string_escape_newline() {
        assert_eq!(
            kinds_no_eof(r#""a\nb""#),
            vec![TokenKind::StringLit("a\nb".into())]
        );
    }

    #[test]
    fn string_escape_tab() {
        assert_eq!(
            kinds_no_eof(r#""a\tb""#),
            vec![TokenKind::StringLit("a\tb".into())]
        );
    }

    #[test]
    fn string_escape_backslash() {
        assert_eq!(
            kinds_no_eof(r#""a\\b""#),
            vec![TokenKind::StringLit("a\\b".into())]
        );
    }

    #[test]
    fn string_escape_quote() {
        assert_eq!(
            kinds_no_eof(r#""say \"hi\"""#),
            vec![TokenKind::StringLit(r#"say "hi""#.into())]
        );
    }

    #[test]
    fn string_all_escapes_combined() {
        assert_eq!(
            kinds_no_eof(r#""a\nb\tc\\d\"e""#),
            vec![TokenKind::StringLit("a\nb\tc\\d\"e".into())]
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 3. BOOLEAN LITERALS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn bool_true_false() {
        let k = kinds_no_eof("true false");
        assert_eq!(k, vec![TokenKind::True, TokenKind::False]);
    }

    #[test]
    fn bool_not_keyword_when_prefix() {
        // "trueish" must be an identifier, not `true` + `ish`
        let k = kinds_no_eof("trueish");
        assert_eq!(k, vec![TokenKind::Ident("trueish".into())]);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 4. KEYWORDS (complete set)
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn keywords_all() {
        use TokenKind::*;
        let k = kinds_no_eof(
            "let in if elif else while for function class is new self case of with as null true false"
        );
        assert_eq!(k, vec![
            Let, In, If, Elif, Else, While, For, Function,
            Class, Is, New, Self_, Case, Of, With, As, Null, True, False,
        ]);
    }

    #[test]
    fn keyword_adjacent_to_identifier() {
        // "letting" must NOT be parsed as Let + "ting"
        let k = kinds_no_eof("letting");
        assert_eq!(k, vec![TokenKind::Ident("letting".into())]);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 5. IDENTIFIERS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn identifier_simple() {
        let k = kinds_no_eof("x");
        assert_eq!(k, vec![TokenKind::Ident("x".into())]);
    }

    #[test]
    fn identifier_with_underscore() {
        let k = kinds_no_eof("my_var _private __dunder");
        assert_eq!(k, vec![
            TokenKind::Ident("my_var".into()),
            TokenKind::Ident("_private".into()),
            TokenKind::Ident("__dunder".into()),
        ]);
    }

    #[test]
    fn identifier_alphanumeric() {
        let k = kinds_no_eof("var1 x2y z99");
        assert_eq!(k, vec![
            TokenKind::Ident("var1".into()),
            TokenKind::Ident("x2y".into()),
            TokenKind::Ident("z99".into()),
        ]);
    }

    #[test]
    fn identifier_type_names() {
        let k = kinds_no_eof("Number Boolean String Object");
        assert_eq!(k, vec![
            TokenKind::Ident("Number".into()),
            TokenKind::Ident("Boolean".into()),
            TokenKind::Ident("String".into()),
            TokenKind::Ident("Object".into()),
        ]);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 6. ARITHMETIC OPERATORS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn operators_arithmetic() {
        use TokenKind::*;
        assert_eq!(kinds_no_eof("+ - * / % ^"), vec![Plus, Minus, Star, Slash, Percent, Caret]);
    }

    #[test]
    fn operators_string_concat() {
        use TokenKind::*;
        assert_eq!(kinds_no_eof("@ @@"), vec![At, AtAt]);
    }

    #[test]
    fn operators_at_disambiguation() {
        // "@@ @" must lex as AtAt then At, not At + At + At
        use TokenKind::*;
        assert_eq!(kinds_no_eof("@@@"), vec![AtAt, At]);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 7. COMPARISON & LOGICAL OPERATORS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn operators_comparison() {
        use TokenKind::*;
        assert_eq!(kinds_no_eof("< <= > >= == !="), vec![Lt, Le, Gt, Ge, EqEq, BangEq]);
    }

    #[test]
    fn operators_logical() {
        use TokenKind::*;
        assert_eq!(kinds_no_eof("& | !"), vec![Amp, Pipe, Bang]);
    }

    #[test]
    fn operators_no_greedy_eq() {
        // "= =" must not collapse into "=="
        use TokenKind::*;
        assert_eq!(kinds_no_eof("= ="), vec![Eq, Eq]);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 8. ASSIGNMENT & ARROW OPERATORS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn arrow_function_body() {
        assert_eq!(kinds_no_eof("->"), vec![TokenKind::Arrow]);
    }

    #[test]
    fn destructive_assign() {
        assert_eq!(kinds_no_eof(":="), vec![TokenKind::ColonEq]);
    }

    #[test]
    fn double_arrow() {
        assert_eq!(kinds_no_eof("=>"), vec![TokenKind::DArrow]);
    }

    #[test]
    fn colon_vs_colon_eq() {
        use TokenKind::*;
        assert_eq!(kinds_no_eof(": :="), vec![Colon, ColonEq]);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 9. PUNCTUATION
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn punctuation_all() {
        use TokenKind::*;
        assert_eq!(
            kinds_no_eof("( ) { } [ ] , ; : ."),
            vec![LParen, RParen, LBrace, RBrace, LBracket, RBracket, Comma, Semicolon, Colon, Dot]
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 10. WHITESPACE & COMMENTS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn whitespace_tabs_and_newlines() {
        let k = kinds_no_eof("1\t2\n3");
        assert_eq!(k, vec![
            TokenKind::Number(1.0),
            TokenKind::Number(2.0),
            TokenKind::Number(3.0),
        ]);
    }

    #[test]
    fn comment_line_skipped() {
        let k = kinds_no_eof("1 // comment here\n2");
        assert_eq!(k, vec![TokenKind::Number(1.0), TokenKind::Number(2.0)]);
    }

    #[test]
    fn comment_at_end_of_file() {
        let k = kinds_no_eof("x // no newline at eof");
        assert_eq!(k, vec![TokenKind::Ident("x".into())]);
    }

    #[test]
    fn comment_only_file() {
        let k = kinds_no_eof("// entire file is a comment");
        assert!(k.is_empty());
    }

    #[test]
    fn multiple_comments() {
        let k = kinds_no_eof("// first\n// second\nx");
        assert_eq!(k, vec![TokenKind::Ident("x".into())]);
    }

    #[test]
    fn empty_source() {
        let k = lex("");
        assert_eq!(k, vec![TokenKind::Eof]);
    }

    #[test]
    fn whitespace_only() {
        let k = lex("   \n\t  ");
        assert_eq!(k, vec![TokenKind::Eof]);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 11. SPAN / POSITION TRACKING
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn span_single_token() {
        let tokens = lex_full("hello");
        let t = &tokens[0];
        assert_eq!(t.span.line, 1);
        assert_eq!(t.span.col,  1);
        assert_eq!(t.span.start, 0);
        assert_eq!(t.span.end,   5);
    }

    #[test]
    fn span_second_token_on_same_line() {
        let tokens = lex_full("x y");
        let y = &tokens[1];
        assert_eq!(y.span.line, 1);
        assert_eq!(y.span.col,  3);
    }

    #[test]
    fn span_token_on_second_line() {
        let tokens = lex_full("x\ny");
        let y = &tokens[1];
        assert_eq!(y.span.line, 2);
        assert_eq!(y.span.col,  1);
    }

    #[test]
    fn lexeme_preserved() {
        let tokens = lex_full("myVar 3.14");
        assert_eq!(tokens[0].lexeme, "myVar");
        assert_eq!(tokens[1].lexeme, "3.14");
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 12. ERROR CASES
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn error_unexpected_char_dollar() {
        // '$' is now a valid token (macro parameter prefix) — test that it lexes correctly
        let toks = Lexer::new("$").tokenize().unwrap();
        assert_eq!(toks[0].kind, TokenKind::Dollar);
    }

    #[test]
    fn error_unexpected_char_hash() {
        assert!(matches!(lex_err("#"), LexError::UnexpectedChar { ch: '#', .. }));
    }

    #[test]
    fn error_unexpected_char_question() {
        assert!(matches!(lex_err("?"), LexError::UnexpectedChar { ch: '?', .. }));
    }

    #[test]
    fn error_unterminated_string() {
        assert!(matches!(lex_err(r#""oops"#), LexError::UnterminatedString { .. }));
    }

    #[test]
    fn error_unterminated_string_with_newline() {
        // A newline inside a string closes it forcibly → error
        assert!(matches!(lex_err("\"bad\nnews\""), LexError::UnterminatedString { .. }));
    }

    #[test]
    fn error_unknown_escape() {
        assert!(matches!(lex_err(r#""\z""#), LexError::UnknownEscape { ch: 'z', .. }));
    }

    #[test]
    fn error_position_reported_correctly() {
        // "x\n  #": '#' is on line 2, at col 3 (two spaces then hash, 1-indexed)
        let err = Lexer::new("x\n  #").tokenize().unwrap_err();
        match err {
            LexError::UnexpectedChar { ch, line, col } => {
                assert_eq!(ch, '#');
                assert_eq!(line, 2);
                assert_eq!(col, 3);
            }
            _ => panic!("wrong error kind"),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 13. HULK PROGRAM SNIPPETS (integration)
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn snippet_let_in() {
        use TokenKind::*;
        let k = kinds_no_eof("let x = 5 in x + 1;");
        assert_eq!(k, vec![
            Let, Ident("x".into()), Eq, Number(5.0),
            In, Ident("x".into()), Plus, Number(1.0), Semicolon,
        ]);
    }

    #[test]
    fn snippet_let_typed() {
        use TokenKind::*;
        let k = kinds_no_eof("let x: Number = 5 in x;");
        assert_eq!(k, vec![
            Let, Ident("x".into()), Colon, Ident("Number".into()),
            Eq, Number(5.0), In, Ident("x".into()), Semicolon,
        ]);
    }

    #[test]
    fn snippet_if_else() {
        use TokenKind::*;
        let k = kinds_no_eof("if (x > 0) x else -1;");
        assert_eq!(k, vec![
            If, LParen, Ident("x".into()), Gt, Number(0.0), RParen,
            Ident("x".into()), Else, Minus, Number(1.0), Semicolon,
        ]);
    }

    #[test]
    fn snippet_while_loop() {
        use TokenKind::*;
        let k = kinds_no_eof("while (i < 10) { i := i + 1; }");
        assert_eq!(k, vec![
            While, LParen, Ident("i".into()), Lt, Number(10.0), RParen,
            LBrace, Ident("i".into()), ColonEq, Ident("i".into()), Plus, Number(1.0),
            Semicolon, RBrace,
        ]);
    }

    #[test]
    fn snippet_function_decl() {
        use TokenKind::*;
        let k = kinds_no_eof("function add(a: Number, b: Number): Number -> a + b;");
        assert_eq!(k, vec![
            Function, Ident("add".into()),
            LParen,
                Ident("a".into()), Colon, Ident("Number".into()), Comma,
                Ident("b".into()), Colon, Ident("Number".into()),
            RParen,
            Colon, Ident("Number".into()),
            Arrow,
            Ident("a".into()), Plus, Ident("b".into()), Semicolon,
        ]);
    }

    #[test]
    fn snippet_class_decl() {
        use TokenKind::*;
        let k = kinds_no_eof("class Animal is Object { }");
        assert_eq!(k, vec![
            Class, Ident("Animal".into()), Is, Ident("Object".into()),
            LBrace, RBrace,
        ]);
    }

    #[test]
    fn snippet_new_instance() {
        use TokenKind::*;
        let k = kinds_no_eof("let p = new Point(1, 2) in p;");
        assert_eq!(k, vec![
            Let, Ident("p".into()), Eq,
            New, Ident("Point".into()), LParen, Number(1.0), Comma, Number(2.0), RParen,
            In, Ident("p".into()), Semicolon,
        ]);
    }

    #[test]
    fn snippet_string_concat() {
        use TokenKind::*;
        let k = kinds_no_eof(r#""Hello" @ " " @ name"#);
        assert_eq!(k, vec![
            StringLit("Hello".into()), At,
            StringLit(" ".into()), At,
            Ident("name".into()),
        ]);
    }

    #[test]
    fn snippet_string_concat_spaced() {
        use TokenKind::*;
        let k = kinds_no_eof(r#""Hello" @@ name"#);
        assert_eq!(k, vec![
            StringLit("Hello".into()), AtAt, Ident("name".into()),
        ]);
    }

    #[test]
    fn snippet_case_expression() {
        use TokenKind::*;
        let k = kinds_no_eof("case x of { id: Animal -> id.speak(); }");
        assert_eq!(k, vec![
            Case, Ident("x".into()), Of,
            LBrace,
                Ident("id".into()), Colon, Ident("Animal".into()), Arrow,
                Ident("id".into()), Dot, Ident("speak".into()), LParen, RParen, Semicolon,
            RBrace,
        ]);
    }

    #[test]
    fn snippet_with_expression() {
        use TokenKind::*;
        let k = kinds_no_eof("with (foo() as x) x.value else 0;");
        assert_eq!(k, vec![
            With, LParen,
                Ident("foo".into()), LParen, RParen, As, Ident("x".into()),
            RParen,
            Ident("x".into()), Dot, Ident("value".into()),
            Else, Number(0.0), Semicolon,
        ]);
    }

    #[test]
    fn snippet_array_new() {
        use TokenKind::*;
        let k = kinds_no_eof("new Number[10]");
        assert_eq!(k, vec![
            New, Ident("Number".into()), LBracket, Number(10.0), RBracket,
        ]);
    }

    #[test]
    fn snippet_self_method_call() {
        use TokenKind::*;
        let k = kinds_no_eof("self.area()");
        assert_eq!(k, vec![
            Self_, Dot, Ident("area".into()), LParen, RParen,
        ]);
    }

    #[test]
    fn snippet_multiline_program() {
        use TokenKind::*;
        let src = r#"
            function square(x: Number): Number -> x * x;
            let result = square(5) in
                result;
        "#;
        let k = kinds_no_eof(src);
        assert_eq!(k[0], Function);
        assert_eq!(k[1], Ident("square".into()));
        // last real token before EOF
        let last = k.last().unwrap();
        assert_eq!(*last, Semicolon);
    }
}
