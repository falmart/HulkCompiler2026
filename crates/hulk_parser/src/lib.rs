pub mod error;
pub mod parser;

pub use error::ParseError;
pub use parser::Parser;
pub use hulk_lexer::LexError;

use hulk_ast::{ExprS, Program};
use hulk_lexer::Lexer;

/// Error distinguishing lex phase from parse phase — used by the CLI.
#[derive(Debug)]
pub enum PipelineError {
    Lex(LexError),
    Parse(ParseError),
}

/// Lex + parse, returning a typed error so the CLI can emit the right exit code.
pub fn compile(src: &str) -> Result<Program, PipelineError> {
    let tokens = Lexer::new(src).tokenize().map_err(PipelineError::Lex)?;
    Parser::new(tokens).parse_program().map_err(PipelineError::Parse)
}

/// Convenience: lex + parse a source string into a full Program.
pub fn parse_program(src: &str) -> Result<Program, ParseError> {
    let tokens = Lexer::new(src)
        .tokenize()
        .map_err(|e| ParseError::UnexpectedEof { expected: e.to_string() })?;
    Parser::new(tokens).parse_program()
}

/// Convenience: lex + parse a single expression (without trailing ';').
pub fn parse_expr(src: &str) -> Result<ExprS, ParseError> {
    let tokens = Lexer::new(src)
        .tokenize()
        .map_err(|e| ParseError::UnexpectedEof { expected: e.to_string() })?;
    Parser::new(tokens).parse_expr()
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use hulk_ast::*;

    // ── Shorthand helpers ────────────────────────────────────────────────────

    fn expr(src: &str) -> Expr {
        parse_expr(src).expect("parse failed").node
    }

    fn prog(src: &str) -> Program {
        parse_program(src).expect("parse failed")
    }

    fn num(n: f64) -> Expr { Expr::Number(n) }
    fn var(s: &str) -> Expr { Expr::Var(s.into()) }
    fn bool_(b: bool) -> Expr { Expr::Bool(b) }
    fn str_(s: &str) -> Expr { Expr::Str(s.into()) }

    fn bin(op: BinaryOp, l: Expr, r: Expr) -> Expr {
        // Build dummy-spanned nodes for comparison (span is irrelevant in tests)
        let dummy = hulk_lexer::Span::default();
        Expr::Binary {
            op,
            left:  Box::new(Spanned::new(l, dummy)),
            right: Box::new(Spanned::new(r, dummy)),
        }
    }

    fn un(op: UnaryOp, e: Expr) -> Expr {
        let dummy = hulk_lexer::Span::default();
        Expr::Unary { op, operand: Box::new(Spanned::new(e, dummy)) }
    }

    /// Compare only the AST node structure, ignoring spans.
    fn eq(a: &Expr, b: &Expr) -> bool {
        expr_eq(a, b)
    }

    fn expr_eq(a: &Expr, b: &Expr) -> bool {
        match (a, b) {
            (Expr::Number(x), Expr::Number(y))   => (x - y).abs() < 1e-12,
            (Expr::Bool(x),   Expr::Bool(y))     => x == y,
            (Expr::Str(x),    Expr::Str(y))      => x == y,
            (Expr::Null,      Expr::Null)         => true,
            (Expr::Self_,     Expr::Self_)        => true,
            (Expr::Var(x),    Expr::Var(y))       => x == y,
            (Expr::Binary { op: oa, left: la, right: ra },
             Expr::Binary { op: ob, left: lb, right: rb }) =>
                oa == ob && expr_eq(&la.node, &lb.node) && expr_eq(&ra.node, &rb.node),
            (Expr::Unary { op: oa, operand: ea },
             Expr::Unary { op: ob, operand: eb }) =>
                oa == ob && expr_eq(&ea.node, &eb.node),
            (Expr::Assign { target: ta, value: va },
             Expr::Assign { target: tb, value: vb }) =>
                expr_eq(&ta.node, &tb.node) && expr_eq(&va.node, &vb.node),
            (Expr::Call { callee: ca, args: aa },
             Expr::Call { callee: cb, args: ab }) =>
                ca == cb && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(a, b)| expr_eq(&a.node, &b.node)),
            (Expr::MethodCall { method: ma, args: aa, .. },
             Expr::MethodCall { method: mb, args: ab, .. }) =>
                ma == mb && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(a, b)| expr_eq(&a.node, &b.node)),
            (Expr::FieldAccess { field: fa, .. },
             Expr::FieldAccess { field: fb, .. }) => fa == fb,
            (Expr::Block(va), Expr::Block(vb)) =>
                va.len() == vb.len()
                && va.iter().zip(vb.iter()).all(|(a, b)| expr_eq(&a.node, &b.node)),
            _ => false,
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 1. LITERALS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn lit_number_int() {
        assert!(eq(&expr("42"), &num(42.0)));
    }

    #[test]
    fn lit_number_float() {
        assert!(eq(&expr("3.14"), &num(3.14)));
    }

    #[test]
    fn lit_bool_true() {
        assert!(eq(&expr("true"), &bool_(true)));
    }

    #[test]
    fn lit_bool_false() {
        assert!(eq(&expr("false"), &bool_(false)));
    }

    #[test]
    fn lit_string() {
        assert!(eq(&expr(r#""hello""#), &str_("hello")));
    }

    #[test]
    fn lit_null() {
        assert!(eq(&expr("null"), &Expr::Null));
    }

    #[test]
    fn lit_self_() {
        assert!(eq(&expr("self"), &Expr::Self_));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 2. ARITHMETIC OPERATORS & PRECEDENCE
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn arith_addition() {
        assert!(eq(&expr("1 + 2"), &bin(BinaryOp::Add, num(1.0), num(2.0))));
    }

    #[test]
    fn arith_subtraction() {
        assert!(eq(&expr("5 - 3"), &bin(BinaryOp::Sub, num(5.0), num(3.0))));
    }

    #[test]
    fn arith_mul_div() {
        assert!(eq(&expr("4 * 2"), &bin(BinaryOp::Mul, num(4.0), num(2.0))));
        assert!(eq(&expr("8 / 2"), &bin(BinaryOp::Div, num(8.0), num(2.0))));
    }

    #[test]
    fn arith_modulo() {
        assert!(eq(&expr("10 % 3"), &bin(BinaryOp::Mod, num(10.0), num(3.0))));
    }

    #[test]
    fn arith_precedence_mul_over_add() {
        // 1 + 2 * 3  →  1 + (2 * 3)
        let got = expr("1 + 2 * 3");
        let expected = bin(BinaryOp::Add, num(1.0), bin(BinaryOp::Mul, num(2.0), num(3.0)));
        assert!(eq(&got, &expected));
    }

    #[test]
    fn arith_left_associative_add() {
        // 1 + 2 + 3  →  (1 + 2) + 3
        let got = expr("1 + 2 + 3");
        let expected = bin(BinaryOp::Add, bin(BinaryOp::Add, num(1.0), num(2.0)), num(3.0));
        assert!(eq(&got, &expected));
    }

    #[test]
    fn arith_parens_override_precedence() {
        // (1 + 2) * 3
        let got = expr("(1 + 2) * 3");
        let expected = bin(BinaryOp::Mul, bin(BinaryOp::Add, num(1.0), num(2.0)), num(3.0));
        assert!(eq(&got, &expected));
    }

    #[test]
    fn arith_power_right_associative() {
        // 2 ^ 3 ^ 4  →  2 ^ (3 ^ 4)
        let got = expr("2 ^ 3 ^ 4");
        let expected = bin(BinaryOp::Pow, num(2.0), bin(BinaryOp::Pow, num(3.0), num(4.0)));
        assert!(eq(&got, &expected));
    }

    #[test]
    fn arith_power_over_mul() {
        // 2 * 3 ^ 2  →  2 * (3 ^ 2)
        let got = expr("2 * 3 ^ 2");
        let expected = bin(BinaryOp::Mul, num(2.0), bin(BinaryOp::Pow, num(3.0), num(2.0)));
        assert!(eq(&got, &expected));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 3. UNARY OPERATORS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn unary_negation() {
        assert!(eq(&expr("-5"), &un(UnaryOp::Neg, num(5.0))));
    }

    #[test]
    fn unary_not() {
        assert!(eq(&expr("!true"), &un(UnaryOp::Not, bool_(true))));
    }

    #[test]
    fn unary_double_negation() {
        let got = expr("--5");
        let expected = un(UnaryOp::Neg, un(UnaryOp::Neg, num(5.0)));
        assert!(eq(&got, &expected));
    }

    #[test]
    fn unary_neg_expr() {
        // -(1 + 2)
        let got = expr("-(1 + 2)");
        let expected = un(UnaryOp::Neg, bin(BinaryOp::Add, num(1.0), num(2.0)));
        assert!(eq(&got, &expected));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 4. COMPARISON & LOGICAL
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn cmp_less_than() {
        assert!(eq(&expr("x < 5"), &bin(BinaryOp::Lt, var("x"), num(5.0))));
    }

    #[test]
    fn cmp_equal() {
        assert!(eq(&expr("x == y"), &bin(BinaryOp::Eq, var("x"), var("y"))));
    }

    #[test]
    fn cmp_not_equal() {
        assert!(eq(&expr("x != y"), &bin(BinaryOp::Ne, var("x"), var("y"))));
    }

    #[test]
    fn logical_and() {
        assert!(eq(&expr("a & b"), &bin(BinaryOp::And, var("a"), var("b"))));
    }

    #[test]
    fn logical_or() {
        assert!(eq(&expr("a | b"), &bin(BinaryOp::Or, var("a"), var("b"))));
    }

    #[test]
    fn logical_precedence_and_over_or() {
        // a | b & c  →  a | (b & c)
        let got = expr("a | b & c");
        let expected = bin(BinaryOp::Or, var("a"), bin(BinaryOp::And, var("b"), var("c")));
        assert!(eq(&got, &expected));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 5. STRING CONCATENATION
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn concat_at() {
        let got = expr(r#""hello" @ " world""#);
        let expected = bin(BinaryOp::Concat, str_("hello"), str_(" world"));
        assert!(eq(&got, &expected));
    }

    #[test]
    fn concat_at_at() {
        let got = expr(r#""hello" @@ name"#);
        let expected = bin(BinaryOp::ConcatSpace, str_("hello"), var("name"));
        assert!(eq(&got, &expected));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 6. DESTRUCTIVE ASSIGNMENT
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn assign_simple() {
        let got = expr("x := 5");
        let dummy = hulk_lexer::Span::default();
        let expected = Expr::Assign {
            target: Box::new(Spanned::new(var("x"), dummy)),
            value:  Box::new(Spanned::new(num(5.0), dummy)),
        };
        assert!(eq(&got, &expected));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 7. LET EXPRESSIONS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn let_simple() {
        let got = expr("let x = 5 in x");
        match got {
            Expr::Let { bindings, body } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].name, "x");
                assert!(eq(&bindings[0].init.node, &num(5.0)));
                assert!(eq(&body.node, &var("x")));
            }
            _ => panic!("expected Let, got {:?}", got),
        }
    }

    #[test]
    fn let_with_type_annotation() {
        let got = expr("let x: Number = 5 in x");
        match got {
            Expr::Let { bindings, .. } => {
                assert_eq!(bindings[0].type_ann, Some(TypeExpr::Named("Number".into())));
            }
            _ => panic!("expected Let"),
        }
    }

    #[test]
    fn let_multiple_bindings() {
        let got = expr("let x = 1, y = 2 in x + y");
        match got {
            Expr::Let { bindings, body } => {
                assert_eq!(bindings.len(), 2);
                assert_eq!(bindings[0].name, "x");
                assert_eq!(bindings[1].name, "y");
                assert!(eq(&body.node, &bin(BinaryOp::Add, var("x"), var("y"))));
            }
            _ => panic!("expected Let"),
        }
    }

    #[test]
    fn let_nested() {
        // let x = 1 in let y = 2 in x + y
        let got = expr("let x = 1 in let y = 2 in x + y");
        match got {
            Expr::Let { bindings, body } => {
                assert_eq!(bindings[0].name, "x");
                assert!(matches!(body.node, Expr::Let { .. }));
            }
            _ => panic!("expected Let"),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 8. IF EXPRESSIONS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn if_simple() {
        let got = expr("if (x > 0) x else 0");
        match got {
            Expr::If { cond, then, elif_branches, else_branch } => {
                assert!(eq(&cond.node, &bin(BinaryOp::Gt, var("x"), num(0.0))));
                assert!(eq(&then.node, &var("x")));
                assert!(elif_branches.is_empty());
                assert!(eq(&else_branch.unwrap().node, &num(0.0)));
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn if_without_else() {
        let got = expr("if (true) 1");
        match got {
            Expr::If { else_branch, .. } => assert!(else_branch.is_none()),
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn if_elif_else() {
        let got = expr("if (x < 0) -1 elif (x == 0) 0 else 1");
        match got {
            Expr::If { elif_branches, else_branch, .. } => {
                assert_eq!(elif_branches.len(), 1);
                assert!(else_branch.is_some());
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn if_multiple_elif() {
        let got = expr("if (a) 1 elif (b) 2 elif (c) 3 else 4");
        match got {
            Expr::If { elif_branches, .. } => assert_eq!(elif_branches.len(), 2),
            _ => panic!("expected If"),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 9. WHILE LOOPS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn while_simple() {
        let got = expr("while (i < 10) i := i + 1");
        match got {
            Expr::While { cond, body } => {
                assert!(eq(&cond.node, &bin(BinaryOp::Lt, var("i"), num(10.0))));
                assert!(matches!(body.node, Expr::Assign { .. }));
            }
            _ => panic!("expected While"),
        }
    }

    #[test]
    fn while_block_body() {
        let got = expr("while (true) { x := 1; }");
        assert!(matches!(got, Expr::While { .. }));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 10. BLOCKS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn block_single_expr() {
        let got = expr("{ 42 }");
        match got {
            Expr::Block(stmts) => {
                assert_eq!(stmts.len(), 1);
                assert!(eq(&stmts[0].node, &num(42.0)));
            }
            _ => panic!("expected Block"),
        }
    }

    #[test]
    fn block_multiple_stmts() {
        let got = expr("{ x := 1; y := 2; x + y }");
        match got {
            Expr::Block(stmts) => assert_eq!(stmts.len(), 3),
            _ => panic!("expected Block"),
        }
    }

    #[test]
    fn block_all_semicoloned() {
        // All semicolons: last value is still the last expression
        let got = expr("{ 1; 2; }");
        match got {
            Expr::Block(stmts) => assert_eq!(stmts.len(), 2),
            _ => panic!("expected Block"),
        }
    }

    #[test]
    fn block_empty() {
        let got = expr("{ }");
        match got {
            Expr::Block(stmts) => assert!(stmts.is_empty()),
            _ => panic!("expected Block"),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 11. FUNCTION CALLS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn call_no_args() {
        let got = expr("foo()");
        assert!(eq(&got, &Expr::Call { callee: "foo".into(), args: vec![] }));
    }

    #[test]
    fn call_with_args() {
        let got = expr("add(1, 2)");
        match got {
            Expr::Call { callee, args } => {
                assert_eq!(callee, "add");
                assert_eq!(args.len(), 2);
                assert!(eq(&args[0].node, &num(1.0)));
                assert!(eq(&args[1].node, &num(2.0)));
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn call_nested_args() {
        let got = expr("max(min(a, b), c)");
        match got {
            Expr::Call { callee, args } => {
                assert_eq!(callee, "max");
                assert_eq!(args.len(), 2);
                assert!(matches!(args[0].node, Expr::Call { .. }));
            }
            _ => panic!("expected Call"),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 12. METHOD CALLS & FIELD ACCESS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn method_call() {
        let got = expr("obj.speak()");
        match got {
            Expr::MethodCall { method, args, .. } => {
                assert_eq!(method, "speak");
                assert!(args.is_empty());
            }
            _ => panic!("expected MethodCall"),
        }
    }

    #[test]
    fn method_call_with_args() {
        let got = expr("p.translate(1, 2)");
        match got {
            Expr::MethodCall { method, args, .. } => {
                assert_eq!(method, "translate");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected MethodCall"),
        }
    }

    #[test]
    fn field_access() {
        let got = expr("p.x");
        match got {
            Expr::FieldAccess { field, .. } => assert_eq!(field, "x"),
            _ => panic!("expected FieldAccess"),
        }
    }

    #[test]
    fn chained_method_calls() {
        // a.b().c()
        let got = expr("a.b().c()");
        match got {
            Expr::MethodCall { method, object, .. } => {
                assert_eq!(method, "c");
                assert!(matches!(object.node, Expr::MethodCall { .. }));
            }
            _ => panic!("expected MethodCall"),
        }
    }

    #[test]
    fn self_method_call() {
        let got = expr("self.area()");
        match got {
            Expr::MethodCall { object, method, .. } => {
                assert!(eq(&object.node, &Expr::Self_));
                assert_eq!(method, "area");
            }
            _ => panic!("expected MethodCall"),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 13. NEW INSTANCES & ARRAYS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn new_instance_no_args() {
        let got = expr("new Foo()");
        match got {
            Expr::New { type_name, args } => {
                assert_eq!(type_name, "Foo");
                assert!(args.is_empty());
            }
            _ => panic!("expected New"),
        }
    }

    #[test]
    fn new_instance_with_args() {
        let got = expr("new Point(1, 2)");
        match got {
            Expr::New { type_name, args } => {
                assert_eq!(type_name, "Point");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected New"),
        }
    }

    #[test]
    fn new_array() {
        let got = expr("new Number[10]");
        match got {
            Expr::NewArray { type_name, size, init } => {
                assert_eq!(type_name, "Number");
                assert!(eq(&size.node, &num(10.0)));
                assert!(init.is_none());
            }
            _ => panic!("expected NewArray"),
        }
    }

    #[test]
    fn new_array_with_init() {
        let got = expr("new Number[10] { 0 }");
        match got {
            Expr::NewArray { init, .. } => assert!(init.is_some()),
            _ => panic!("expected NewArray"),
        }
    }

    #[test]
    fn array_index() {
        let got = expr("arr[2]");
        match got {
            Expr::Index { index, .. } => assert!(eq(&index.node, &num(2.0))),
            _ => panic!("expected Index"),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 14. CASE & WITH
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn case_expression() {
        let got = expr("case x of { a: Animal -> a.speak(); }");
        match got {
            Expr::Case { arms, .. } => {
                assert_eq!(arms.len(), 1);
                assert_eq!(arms[0].binding, "a");
                assert_eq!(arms[0].type_ann, TypeExpr::Named("Animal".into()));
            }
            _ => panic!("expected Case"),
        }
    }

    #[test]
    fn case_multiple_arms() {
        let got = expr("case x of { a: Dog -> a.bark(); b: Cat -> b.meow(); }");
        match got {
            Expr::Case { arms, .. } => assert_eq!(arms.len(), 2),
            _ => panic!("expected Case"),
        }
    }

    #[test]
    fn with_expression() {
        let got = expr("with (find() as result) result.value else 0");
        match got {
            Expr::With { binding, .. } => assert_eq!(binding, "result"),
            _ => panic!("expected With"),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 15. FUNCTION DECLARATIONS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn func_decl_inline() {
        let p = prog("function square(x: Number): Number -> x * x;");
        assert_eq!(p.functions.len(), 1);
        let f = &p.functions[0];
        assert_eq!(f.name, "square");
        assert_eq!(f.params.len(), 1);
        assert_eq!(f.params[0].name, "x");
        assert_eq!(f.params[0].type_ann, Some(TypeExpr::Named("Number".into())));
        assert_eq!(f.return_type, Some(TypeExpr::Named("Number".into())));
    }

    #[test]
    fn func_decl_block_body() {
        let p = prog("function greet() { \"hello\" }");
        assert_eq!(p.functions.len(), 1);
        assert!(matches!(p.functions[0].body.node, Expr::Block(_)));
    }

    #[test]
    fn func_decl_no_params() {
        let p = prog("function pi(): Number -> 3;");
        assert!(p.functions[0].params.is_empty());
    }

    #[test]
    fn func_decl_multiple_params() {
        let p = prog("function add(a: Number, b: Number): Number -> a + b;");
        assert_eq!(p.functions[0].params.len(), 2);
    }

    #[test]
    fn func_decl_no_return_type() {
        let p = prog("function noop() -> null;");
        assert!(p.functions[0].return_type.is_none());
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 16. CLASS DECLARATIONS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn class_empty() {
        let p = prog("class Foo { }");
        assert_eq!(p.classes.len(), 1);
        assert_eq!(p.classes[0].name, "Foo");
        assert!(p.classes[0].members.is_empty());
    }

    #[test]
    fn class_with_base() {
        let p = prog("class Dog is Animal { }");
        assert_eq!(p.classes[0].base, Some("Animal".into()));
    }

    #[test]
    fn class_ctor_params() {
        let p = prog("class Point(x: Number, y: Number) { }");
        assert_eq!(p.classes[0].ctor_params.len(), 2);
        assert_eq!(p.classes[0].ctor_params[0].name, "x");
    }

    #[test]
    fn class_with_attributes() {
        let p = prog("class Point(x: Number) { x := x; }");
        assert_eq!(p.classes[0].members.len(), 1);
        assert!(matches!(p.classes[0].members[0], ClassMember::Attribute { .. }));
    }

    #[test]
    fn class_with_method_inline() {
        let p = prog("class Circle(r: Number) { area(): Number -> 3 * r * r; }");
        let m = &p.classes[0].members[0];
        match m {
            ClassMember::Method { name, .. } => assert_eq!(name, "area"),
            _ => panic!("expected Method"),
        }
    }

    #[test]
    fn class_with_function_keyword_method() {
        let p = prog("class Foo { function bar() -> 1; }");
        assert!(matches!(p.classes[0].members[0], ClassMember::Method { .. }));
    }

    #[test]
    fn class_full() {
        let src = r#"
            class Animal(name: String) is Object {
                name := name;
                speak(): String -> "...";
            }
        "#;
        let p = prog(src);
        assert_eq!(p.classes[0].name, "Animal");
        assert_eq!(p.classes[0].base, Some("Object".into()));
        assert_eq!(p.classes[0].members.len(), 2);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 17. FULL PROGRAMS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn program_entry_only() {
        let p = prog("42;");
        assert!(p.functions.is_empty());
        assert!(p.classes.is_empty());
        assert!(matches!(p.entry, Some(ref e) if eq(&e.node, &num(42.0))));
    }

    #[test]
    fn program_function_and_entry() {
        let p = prog("function double(x: Number): Number -> x * 2; double(5);");
        assert_eq!(p.functions.len(), 1);
        assert!(p.entry.is_some());
    }

    #[test]
    fn program_multiple_functions() {
        let p = prog("function f() -> 1; function g() -> 2;");
        assert_eq!(p.functions.len(), 2);
    }

    #[test]
    fn program_class_and_function() {
        let p = prog("class Foo { } function bar() -> 1;");
        assert_eq!(p.classes.len(), 1);
        assert_eq!(p.functions.len(), 1);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 18. TYPE EXPRESSIONS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn type_named() {
        let p = prog("function f(x: Number): Boolean -> true;");
        assert_eq!(p.functions[0].params[0].type_ann, Some(TypeExpr::Named("Number".into())));
        assert_eq!(p.functions[0].return_type, Some(TypeExpr::Named("Boolean".into())));
    }

    #[test]
    fn type_array() {
        let p = prog("function f(xs: Number[]): Number[] -> xs;");
        assert_eq!(p.functions[0].params[0].type_ann,
            Some(TypeExpr::Array(Box::new(TypeExpr::Named("Number".into())))));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 19. ERROR CASES
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn error_missing_in_after_let() {
        let err = parse_expr("let x = 5");
        assert!(err.is_err());
    }

    #[test]
    fn error_missing_closing_paren() {
        let err = parse_expr("(1 + 2");
        assert!(err.is_err());
    }

    #[test]
    fn error_missing_closing_brace() {
        let err = parse_expr("{ 1 + 2");
        assert!(err.is_err());
    }

    #[test]
    fn error_unexpected_token() {
        let err = parse_expr("1 + + 2");
        assert!(err.is_err());
    }

    #[test]
    fn error_empty_case() {
        // case with no arms is valid structurally (semantic phase can reject it)
        let got = expr("case x of { }");
        assert!(matches!(got, Expr::Case { .. }));
    }
}
