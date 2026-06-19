pub mod checker;
pub mod env;
pub mod error;
pub mod types;

pub use checker::Checker;
pub use error::SemanticError;
pub use types::Type;

use hulk_ast::Program;
use env::Env;

/// Run all semantic checks on a parsed program.
/// Returns the list of errors found (empty = valid).
pub fn check(program: &Program) -> Vec<SemanticError> {
    let mut checker = Checker::new();
    checker.collect_declarations(program);
    let mut global = Env::new();
    checker.check_program(program, &mut global);
    checker.errors
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use hulk_parser::{parse_expr as pexpr, parse_program as pprog};
    use hulk_ast::Program;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn errors(src: &str) -> Vec<SemanticError> {
        let prog = pprog(src).expect("parse failed");
        check(&prog)
    }

    fn errors_expr(src: &str) -> Vec<SemanticError> {
        // Wrap a bare expression in a minimal program with trailing ';'
        errors(&format!("{src};"))
    }

    fn ok(src: &str) {
        let errs = errors(src);
        assert!(errs.is_empty(), "Expected no errors but got: {errs:#?}");
    }

    fn ok_expr(src: &str) {
        ok(&format!("{src};"))
    }

    fn has_error<F: Fn(&SemanticError) -> bool>(src: &str, pred: F) {
        let errs = errors(src);
        assert!(
            errs.iter().any(|e| pred(e)),
            "Expected matching error in: {errs:#?}"
        );
    }

    fn has_error_expr<F: Fn(&SemanticError) -> bool>(src: &str, pred: F) {
        has_error(&format!("{src};"), pred);
    }

    fn infer_expr(src: &str) -> Type {
        let prog = pprog(&format!("{src};")).expect("parse failed");
        let mut checker = Checker::new();
        checker.collect_declarations(&prog);
        let mut env = Env::new();
        checker.check_expr(prog.entry.as_ref().unwrap(), &mut env)
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 1. LITERALS — type inference
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn infer_number_literal() {
        assert_eq!(infer_expr("42"), Type::Number);
    }

    #[test]
    fn infer_float_literal() {
        assert_eq!(infer_expr("3.14"), Type::Number);
    }

    #[test]
    fn infer_bool_true() {
        assert_eq!(infer_expr("true"), Type::Boolean);
    }

    #[test]
    fn infer_bool_false() {
        assert_eq!(infer_expr("false"), Type::Boolean);
    }

    #[test]
    fn infer_string_literal() {
        assert_eq!(infer_expr(r#""hello""#), Type::Str);
    }

    #[test]
    fn infer_null() {
        assert_eq!(infer_expr("null"), Type::Null);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 2. ARITHMETIC OPERATORS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn arith_add_numbers_ok() {
        assert_eq!(infer_expr("1 + 2"), Type::Number);
        ok_expr("1 + 2");
    }

    #[test]
    fn arith_mixed_chain_ok() {
        assert_eq!(infer_expr("1 + 2 * 3 - 4 / 2"), Type::Number);
    }

    #[test]
    fn arith_pow_ok() {
        assert_eq!(infer_expr("2 ^ 10"), Type::Number);
    }

    #[test]
    fn arith_type_error_bool_plus_number() {
        has_error_expr("true + 1", |e| matches!(e, SemanticError::NonNumericOperand { .. }));
    }

    #[test]
    fn arith_type_error_string_plus_number() {
        has_error_expr(r#""hello" + 1"#, |e| matches!(e, SemanticError::NonNumericOperand { .. }));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 3. COMPARISON & LOGICAL
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn cmp_less_than_returns_bool() {
        assert_eq!(infer_expr("1 < 2"), Type::Boolean);
    }

    #[test]
    fn cmp_eq_any_types() {
        assert_eq!(infer_expr("1 == 1"), Type::Boolean);
        assert_eq!(infer_expr(r#""a" == "b""#), Type::Boolean);
    }

    #[test]
    fn cmp_type_error_string_lt() {
        has_error_expr(r#""a" < "b""#, |e| matches!(e, SemanticError::NonNumericOperand { .. }));
    }

    #[test]
    fn logical_and_booleans_ok() {
        assert_eq!(infer_expr("true & false"), Type::Boolean);
    }

    #[test]
    fn logical_type_error_number_and() {
        has_error_expr("1 & 2", |e| matches!(e, SemanticError::NonBooleanOperand { .. }));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 4. STRING CONCATENATION
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn concat_strings_ok() {
        assert_eq!(infer_expr(r#""a" @ "b""#), Type::Str);
        ok_expr(r#""hello" @@ "world""#);
    }

    #[test]
    fn concat_number_coerced_ok() {
        // HULK coerces Number/Boolean/Null to String in @ and @@ — no error
        ok_expr(r#"1 @ "b""#);
        ok_expr(r#""a" @@ 42"#);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 5. UNARY OPERATORS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn unary_neg_number_ok() {
        assert_eq!(infer_expr("-5"), Type::Number);
    }

    #[test]
    fn unary_not_bool_ok() {
        assert_eq!(infer_expr("!true"), Type::Boolean);
    }

    #[test]
    fn unary_neg_type_error() {
        has_error_expr("-true", |e| matches!(e, SemanticError::NonNumericOperand { .. }));
    }

    #[test]
    fn unary_not_type_error() {
        has_error_expr("!1", |e| matches!(e, SemanticError::NonBooleanOperand { .. }));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 6. VARIABLES & LET
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn let_simple_ok() {
        assert_eq!(infer_expr("let x = 5 in x"), Type::Number);
    }

    #[test]
    fn let_typed_ok() {
        assert_eq!(infer_expr("let x: Number = 5 in x"), Type::Number);
    }

    #[test]
    fn let_multiple_bindings_ok() {
        assert_eq!(infer_expr("let x = 1, y = 2 in x + y"), Type::Number);
    }

    #[test]
    fn let_nested_ok() {
        assert_eq!(infer_expr("let x = 1 in let y = 2 in x + y"), Type::Number);
    }

    #[test]
    fn let_type_mismatch_annotation() {
        has_error_expr("let x: Boolean = 5 in x",
            |e| matches!(e, SemanticError::TypeMismatch { .. }));
    }

    #[test]
    fn undefined_variable() {
        has_error_expr("x + 1", |e| matches!(e, SemanticError::UndefinedVariable { name, .. } if name == "x"));
    }

    #[test]
    fn variable_out_of_scope() {
        // x is bound in let but not accessible outside
        has_error_expr("let x = 1 in x + (let y = 2 in y) + y",
            |e| matches!(e, SemanticError::UndefinedVariable { name, .. } if name == "y"));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 7. IF EXPRESSIONS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn if_bool_cond_ok() {
        assert_eq!(infer_expr("if (true) 1 else 0"), Type::Number);
    }

    #[test]
    fn if_non_bool_cond_error() {
        has_error_expr("if (1) 2 else 3",
            |e| matches!(e, SemanticError::ConditionNotBoolean { .. }));
    }

    #[test]
    fn if_branches_joined_to_object() {
        // Number branch and String branch → Object (common ancestor)
        let ty = infer_expr(r#"if (true) 1 else "a""#);
        assert_eq!(ty, Type::Object);
    }

    #[test]
    fn if_without_else_ok() {
        // No else branch — just check no errors
        ok_expr("if (true) 1");
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 8. WHILE LOOPS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn while_ok() {
        assert_eq!(infer_expr("while (true) 1"), Type::Null);
    }

    #[test]
    fn while_non_bool_cond_error() {
        has_error_expr("while (1) 2",
            |e| matches!(e, SemanticError::ConditionNotBoolean { .. }));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 9. BLOCKS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn block_returns_last_expr() {
        assert_eq!(infer_expr("{ 1; 2; 3 }"), Type::Number);
    }

    #[test]
    fn block_empty_returns_null() {
        assert_eq!(infer_expr("{ }"), Type::Null);
    }

    #[test]
    fn block_errors_propagate() {
        has_error_expr("{ x + 1; 2 }",
            |e| matches!(e, SemanticError::UndefinedVariable { .. }));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 10. FUNCTION CALLS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn call_builtin_sqrt_ok() {
        assert_eq!(infer_expr("sqrt(4)"), Type::Number);
    }

    #[test]
    fn call_builtin_print_ok() {
        ok_expr("print(42)");
    }

    #[test]
    fn call_builtin_rand_ok() {
        assert_eq!(infer_expr("rand()"), Type::Number);
    }

    #[test]
    fn call_undefined_function() {
        has_error_expr("foo()", |e| matches!(e, SemanticError::UndefinedFunction { name, .. } if name == "foo"));
    }

    #[test]
    fn call_arity_mismatch() {
        has_error_expr("sqrt(1, 2)", |e| matches!(e, SemanticError::ArityMismatch { .. }));
    }

    #[test]
    fn call_wrong_arg_type() {
        has_error_expr(r#"sqrt("hello")"#, |e| matches!(e, SemanticError::TypeMismatch { .. }));
    }

    #[test]
    fn call_user_function_ok() {
        ok("function add(a: Number, b: Number): Number -> a + b; add(1, 2);");
    }

    #[test]
    fn call_user_function_return_type_mismatch() {
        has_error("function bad(): Number -> true;",
            |e| matches!(e, SemanticError::TypeMismatch { .. }));
    }

    #[test]
    fn call_user_function_arg_type_mismatch() {
        has_error("function f(x: Number): Number -> x; f(true);",
            |e| matches!(e, SemanticError::TypeMismatch { .. }));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 11. CLASSES
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn class_empty_ok() {
        ok("class Foo { }");
    }

    #[test]
    fn class_with_base_ok() {
        ok("class Animal { } class Dog is Animal { }");
    }

    #[test]
    fn class_undefined_base() {
        has_error("class Dog is Nonexistent { }",
            |e| matches!(e, SemanticError::UndefinedClass { .. }));
    }

    #[test]
    fn class_duplicate_declaration() {
        has_error("class Foo { } class Foo { }",
            |e| matches!(e, SemanticError::DuplicateDeclaration { name, .. } if name == "Foo"));
    }

    #[test]
    fn class_with_attribute_ok() {
        ok("class Point(x: Number, y: Number) { x := x; y := y; }");
    }

    #[test]
    fn class_with_method_ok() {
        ok("class Circle(r: Number) { area(): Number -> r * r * 3; }");
    }

    #[test]
    fn class_method_return_type_mismatch() {
        has_error("class Foo { bad(): Number -> true; }",
            |e| matches!(e, SemanticError::TypeMismatch { .. }));
    }

    #[test]
    fn class_instantiation_ok() {
        ok("class Point(x: Number, y: Number) { } new Point(1, 2);");
    }

    #[test]
    fn class_instantiation_arity_mismatch() {
        has_error("class Point(x: Number, y: Number) { } new Point(1);",
            |e| matches!(e, SemanticError::ArityMismatch { .. }));
    }

    #[test]
    fn class_instantiation_arg_type_mismatch() {
        has_error("class Point(x: Number, y: Number) { } new Point(1, true);",
            |e| matches!(e, SemanticError::TypeMismatch { .. }));
    }

    #[test]
    fn class_undefined_instantiation() {
        has_error_expr("new Ghost()", |e| matches!(e, SemanticError::UndefinedClass { .. }));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 12. METHODS & FIELD ACCESS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn method_call_ok() {
        ok(r#"let s = "hello" in s.length();"#);
    }

    #[test]
    fn method_undefined() {
        has_error_expr(r#""hello".nonexistent()"#,
            |e| matches!(e, SemanticError::UndefinedMethod { .. }));
    }

    #[test]
    fn method_arity_mismatch() {
        has_error_expr(r#""hello".length(1)"#,
            |e| matches!(e, SemanticError::ArityMismatch { .. }));
    }

    #[test]
    fn field_access_ok() {
        ok("class Point(x: Number, y: Number) { x := x; y := y; px(): Number -> self.x; }");
    }

    #[test]
    fn field_access_undefined() {
        has_error(
            "class Foo { } let f = new Foo() in f.missing;",
            |e| matches!(e, SemanticError::UndefinedField { .. }),
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 13. ARRAYS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn new_array_ok() {
        assert_eq!(infer_expr("new Number[10]"), Type::Array(Box::new(Type::Number)));
    }

    #[test]
    fn array_index_ok() {
        assert_eq!(infer_expr("let a = new Number[5] in a[0]"), Type::Number);
    }

    #[test]
    fn array_non_number_index_error() {
        has_error_expr("let a = new Number[5] in a[true]",
            |e| matches!(e, SemanticError::TypeMismatch { .. }));
    }

    #[test]
    fn new_array_non_number_size_error() {
        has_error_expr("new Number[true]",
            |e| matches!(e, SemanticError::TypeMismatch { .. }));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 14. CASE & WITH
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn case_ok() {
        ok("class Animal { } class Dog is Animal { speak(): String -> \"woof\"; } let a = new Dog() in case a of { d: Dog -> d.speak(); };");
    }

    #[test]
    fn with_ok() {
        ok(r#"with (null as x) 1 else 0;"#);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 15. INHERITANCE & SUBTYPING
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn subtype_dog_is_animal() {
        ok("class Animal { } class Dog is Animal { } function f(a: Animal): Animal -> a; let d = new Dog() in f(d);");
    }

    #[test]
    fn subtype_if_branches_same_class() {
        let ty = infer_expr("let d = new Object() in if (true) d else d");
        // Should not be Unknown
        assert_ne!(ty, Type::Unknown);
    }

    #[test]
    fn circular_inheritance_detected() {
        // A extends B extends A
        has_error("class A is B { } class B is A { }",
            |e| matches!(e, SemanticError::CircularInheritance { .. }));
    }

    #[test]
    fn duplicate_function_declaration() {
        has_error("function f() -> 1; function f() -> 2;",
            |e| matches!(e, SemanticError::DuplicateDeclaration { name, .. } if name == "f"));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 16. FULL PROGRAMS — valid
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn fibonacci_ok() {
        let src = r#"
            function fib(n: Number): Number ->
                if (n <= 1) n else fib(n - 1) + fib(n - 2);
            fib(10);
        "#;
        ok(src);
    }

    #[test]
    fn point_class_ok() {
        let src = r#"
            class Point(x: Number, y: Number) {
                x := x;
                y := y;
                norm(): Number -> sqrt(x * x + y * y);
            }
            let p = new Point(3, 4) in p.norm();
        "#;
        ok(src);
    }

    #[test]
    fn inheritance_method_call_ok() {
        let src = r#"
            class Animal(name: String) {
                name := name;
                speak(): String -> "...";
            }
            class Dog(name: String) is Animal {
                name := name;
                speak(): String -> "woof";
            }
            let d = new Dog("Rex") in d.speak();
        "#;
        ok(src);
    }

    #[test]
    fn builtin_string_methods_ok() {
        let src = r#"
            let s = "hello" in let n = s.length() in n + 1;
        "#;
        assert_eq!(infer_expr(r#"let s = "hello" in s.length()"#), Type::Number);
    }
}
