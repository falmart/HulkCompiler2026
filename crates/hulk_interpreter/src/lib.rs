pub mod env;
pub mod error;
pub mod interpreter;
pub mod value;

pub use error::RuntimeError;
pub use interpreter::Interpreter;
pub use value::Value;

use hulk_ast::Program;

/// Run a program and return the entry expression value.
pub fn run(program: &Program) -> Result<Value, RuntimeError> {
    let mut interp = Interpreter::new(program);
    interp.run_program(program)
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use hulk_parser::parse_program;

    // ── Test helpers ─────────────────────────────────────────────────────────

    fn eval(src: &str) -> Value {
        let prog = parse_program(src).expect("parse failed");
        run(&prog).expect("runtime error")
    }

    fn eval_expr(src: &str) -> Value {
        eval(&format!("{src};"))
    }

    fn num(n: f64) -> Value { Value::Number(n) }
    fn bool_(b: bool) -> Value { Value::Boolean(b) }
    fn str_(s: &str) -> Value { Value::Str(s.into()) }

    // ══════════════════════════════════════════════════════════════════════════
    // 1. LITERALS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn lit_number()  { assert_eq!(eval_expr("42"),    num(42.0)); }
    #[test]
    fn lit_float()   { assert_eq!(eval_expr("3.14"),  num(3.14)); }
    #[test]
    fn lit_bool()    { assert_eq!(eval_expr("true"),  bool_(true)); }
    #[test]
    fn lit_false()   { assert_eq!(eval_expr("false"), bool_(false)); }
    #[test]
    fn lit_string()  { assert_eq!(eval_expr(r#""hi""#), str_("hi")); }
    #[test]
    fn lit_null()    { assert_eq!(eval_expr("null"),  Value::Null); }

    // ══════════════════════════════════════════════════════════════════════════
    // 2. ARITHMETIC
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn arith_add()  { assert_eq!(eval_expr("1 + 2"),   num(3.0)); }
    #[test]
    fn arith_sub()  { assert_eq!(eval_expr("5 - 3"),   num(2.0)); }
    #[test]
    fn arith_mul()  { assert_eq!(eval_expr("4 * 3"),   num(12.0)); }
    #[test]
    fn arith_div()  { assert_eq!(eval_expr("10 / 2"),  num(5.0)); }
    #[test]
    fn arith_mod()  { assert_eq!(eval_expr("10 % 3"),  num(1.0)); }
    #[test]
    fn arith_pow()  { assert_eq!(eval_expr("2 ^ 10"),  num(1024.0)); }
    #[test]
    fn arith_neg()  { assert_eq!(eval_expr("-7"),      num(-7.0)); }

    #[test]
    fn arith_precedence() {
        // 2 + 3 * 4 = 14 (not 20)
        assert_eq!(eval_expr("2 + 3 * 4"), num(14.0));
    }

    #[test]
    fn arith_parens() {
        assert_eq!(eval_expr("(2 + 3) * 4"), num(20.0));
    }

    #[test]
    fn arith_power_right_assoc() {
        // 2 ^ 3 ^ 2 = 2 ^ (3^2) = 2^9 = 512
        assert_eq!(eval_expr("2 ^ 3 ^ 2"), num(512.0));
    }

    #[test]
    fn arith_div_by_zero() {
        let prog = parse_program("1 / 0;").unwrap();
        assert!(matches!(run(&prog), Err(RuntimeError::DivisionByZero)));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 3. COMPARISON & LOGICAL
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn cmp_lt()  { assert_eq!(eval_expr("1 < 2"),  bool_(true)); }
    #[test]
    fn cmp_gt()  { assert_eq!(eval_expr("2 > 1"),  bool_(true)); }
    #[test]
    fn cmp_le()  { assert_eq!(eval_expr("2 <= 2"), bool_(true)); }
    #[test]
    fn cmp_ge()  { assert_eq!(eval_expr("3 >= 4"), bool_(false)); }
    #[test]
    fn cmp_eq()  { assert_eq!(eval_expr("5 == 5"), bool_(true)); }
    #[test]
    fn cmp_ne()  { assert_eq!(eval_expr("5 != 6"), bool_(true)); }

    #[test]
    fn logical_and_short_circuit() {
        // false & (1/0)  should NOT divide by zero
        assert_eq!(eval_expr("false & (1 / 0 == 0)"), bool_(false));
    }

    #[test]
    fn logical_or_short_circuit() {
        // true | (1/0)  should NOT divide by zero
        assert_eq!(eval_expr("true | (1 / 0 == 0)"), bool_(true));
    }

    #[test]
    fn logical_not() {
        assert_eq!(eval_expr("!true"),  bool_(false));
        assert_eq!(eval_expr("!false"), bool_(true));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 4. STRING CONCATENATION
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn concat_at() {
        assert_eq!(eval_expr(r#""hello" @ " world""#), str_("hello world"));
    }

    #[test]
    fn concat_at_at() {
        assert_eq!(eval_expr(r#""hello" @@ "world""#), str_("hello world"));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 5. LET & VARIABLES
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn let_simple() {
        assert_eq!(eval_expr("let x = 5 in x * 2"), num(10.0));
    }

    #[test]
    fn let_multiple() {
        assert_eq!(eval_expr("let x = 3, y = 4 in x * x + y * y"), num(25.0));
    }

    #[test]
    fn let_nested() {
        assert_eq!(eval_expr("let x = 1 in let y = x + 1 in x + y"), num(3.0));
    }

    #[test]
    fn let_shadowing() {
        // inner x shadows outer x
        assert_eq!(eval_expr("let x = 1 in let x = 10 in x"), num(10.0));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 6. IF / ELIF / ELSE
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn if_true_branch()  { assert_eq!(eval_expr("if (true) 1 else 2"),  num(1.0)); }
    #[test]
    fn if_false_branch() { assert_eq!(eval_expr("if (false) 1 else 2"), num(2.0)); }

    #[test]
    fn if_no_else() {
        assert_eq!(eval_expr("if (false) 1"), Value::Null);
    }

    #[test]
    fn if_elif() {
        let src = "if (false) 1 elif (false) 2 elif (true) 3 else 4";
        assert_eq!(eval_expr(src), num(3.0));
    }

    #[test]
    fn if_condition_expression() {
        assert_eq!(eval_expr("if (2 > 1) \"yes\" else \"no\""), str_("yes"));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 7. WHILE LOOPS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn while_counts_to_five() {
        // Use a destructive assignment inside a block body
        let src = r#"
            let i = 0 in {
                while (i < 5) i := i + 1;
                i
            };
        "#;
        assert_eq!(eval(src), num(5.0));
    }

    #[test]
    fn while_never_executes() {
        // Condition is immediately false
        let src = "let x = 0 in { while (false) x := x + 1; x };";
        assert_eq!(eval(src), num(0.0));
    }

    #[test]
    fn while_accumulate() {
        // Sum 1..=10
        let src = r#"
            let i = 1, sum = 0 in {
                while (i <= 10) {
                    sum := sum + i;
                    i := i + 1;
                };
                sum
            };
        "#;
        assert_eq!(eval(src), num(55.0));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 8. BLOCKS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn block_returns_last() {
        assert_eq!(eval_expr("{ 1; 2; 3 }"), num(3.0));
    }

    #[test]
    fn block_empty() {
        assert_eq!(eval_expr("{ }"), Value::Null);
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 9. BUILT-IN FUNCTIONS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn builtin_sqrt() {
        assert_eq!(eval_expr("sqrt(25)"), num(5.0));
    }

    #[test]
    fn builtin_sqrt_float() {
        if let Value::Number(n) = eval_expr("sqrt(2)") {
            assert!((n - std::f64::consts::SQRT_2).abs() < 1e-10);
        } else { panic!("expected Number") }
    }

    #[test]
    fn builtin_sin_cos() {
        // sin(0) = 0, cos(0) = 1
        assert_eq!(eval_expr("sin(0)"), num(0.0));
        assert_eq!(eval_expr("cos(0)"), num(1.0));
    }

    #[test]
    fn builtin_log() {
        // log(10, 100) = 2
        assert_eq!(eval_expr("log(10, 100)"), num(2.0));
    }

    #[test]
    fn builtin_range() {
        let v = eval_expr("range(3)");
        match v {
            Value::Array(rc) => {
                let elems = rc.borrow();
                assert_eq!(elems.len(), 3);
                assert_eq!(elems[0], num(0.0));
                assert_eq!(elems[1], num(1.0));
                assert_eq!(elems[2], num(2.0));
            }
            _ => panic!("expected Array"),
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 10. USER-DEFINED FUNCTIONS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn user_func_simple() {
        let src = "function square(x: Number): Number -> x * x; square(5);";
        assert_eq!(eval(src), num(25.0));
    }

    #[test]
    fn user_func_multiple_params() {
        let src = "function add(a: Number, b: Number): Number -> a + b; add(3, 4);";
        assert_eq!(eval(src), num(7.0));
    }

    #[test]
    fn user_func_recursive_factorial() {
        let src = r#"
            function fact(n: Number): Number ->
                if (n <= 1) 1 else n * fact(n - 1);
            fact(6);
        "#;
        assert_eq!(eval(src), num(720.0));
    }

    #[test]
    fn user_func_fibonacci() {
        let src = r#"
            function fib(n: Number): Number ->
                if (n <= 1) n else fib(n - 1) + fib(n - 2);
            fib(10);
        "#;
        assert_eq!(eval(src), num(55.0));
    }

    #[test]
    fn user_func_block_body() {
        let src = r#"
            function double(x: Number): Number {
                let two = 2 in x * two
            }
            double(7);
        "#;
        assert_eq!(eval(src), num(14.0));
    }

    #[test]
    fn stack_overflow_detected() {
        let src = "function inf(x: Number): Number -> inf(x + 1); inf(0);";
        let prog = parse_program(src).unwrap();
        assert!(matches!(run(&prog), Err(RuntimeError::StackOverflow)));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 11. CLASSES & OOP
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn class_instantiate_and_field() {
        let src = r#"
            class Point(x: Number, y: Number) {
                x := x;
                y := y;
            }
            let p = new Point(3, 4) in p.x;
        "#;
        assert_eq!(eval(src), num(3.0));
    }

    #[test]
    fn class_method_call() {
        let src = r#"
            class Point(x: Number, y: Number) {
                x := x;
                y := y;
                norm(): Number -> sqrt(x * x + y * y);
            }
            let p = new Point(3, 4) in p.norm();
        "#;
        assert_eq!(eval(src), num(5.0));
    }

    #[test]
    fn class_method_with_param() {
        let src = r#"
            class Counter(n: Number) {
                n := n;
                add(k: Number): Number -> n + k;
            }
            let c = new Counter(10) in c.add(5);
        "#;
        assert_eq!(eval(src), num(15.0));
    }

    #[test]
    fn class_mutate_field() {
        let src = r#"
            class Counter(n: Number) {
                n := n;
                inc(): Number -> { n := n + 1; n };
            }
            let c = new Counter(0) in {
                c.inc();
                c.inc();
                c.inc();
                c.n
            };
        "#;
        assert_eq!(eval(src), num(3.0));
    }

    #[test]
    fn class_inheritance_method_override() {
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
        assert_eq!(eval(src), str_("woof"));
    }

    #[test]
    fn class_inherited_method() {
        let src = r#"
            class Animal(name: String) {
                name := name;
                greet(): String -> "hello from " @ name;
            }
            class Cat(name: String) is Animal {
                name := name;
            }
            let c = new Cat("Whiskers") in c.greet();
        "#;
        assert_eq!(eval(src), str_("hello from Whiskers"));
    }

    #[test]
    fn class_self_method_call() {
        let src = r#"
            class Circle(r: Number) {
                r := r;
                area(): Number -> r * r * 3;
                double_area(): Number -> self.area() * 2;
            }
            let c = new Circle(5) in c.double_area();
        "#;
        assert_eq!(eval(src), num(150.0));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 12. ARRAYS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn array_create_and_index() {
        let src = "let a = new Number[3] { 7 } in a[1];";
        assert_eq!(eval(src), num(7.0));
    }

    #[test]
    fn array_assign_element() {
        let src = r#"
            let a = new Number[3] { 0 } in {
                a[1] := 42;
                a[1]
            };
        "#;
        assert_eq!(eval(src), num(42.0));
    }

    #[test]
    fn array_out_of_bounds() {
        let prog = parse_program("let a = new Number[3] { 0 } in a[5];").unwrap();
        assert!(matches!(run(&prog), Err(RuntimeError::IndexOutOfBounds { .. })));
    }

    #[test]
    fn array_range_sum() {
        // sum of range(5) = 0+1+2+3+4 = 10
        let src = r#"
            let arr = range(5), i = 0, sum = 0 in {
                while (i < 5) {
                    sum := sum + arr[i];
                    i := i + 1;
                };
                sum
            };
        "#;
        assert_eq!(eval(src), num(10.0));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 13. CASE & WITH
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn case_matches_class() {
        let src = r#"
            class Animal { }
            class Dog is Animal { }
            let d = new Dog() in
            case d of {
                dog: Dog    -> "is dog";
                anim: Animal -> "is animal";
            };
        "#;
        assert_eq!(eval(src), str_("is dog"));
    }

    #[test]
    fn case_falls_to_base() {
        let src = r#"
            class Animal { }
            class Cat is Animal { }
            let a = new Animal() in
            case a of {
                c: Cat    -> "cat";
                a: Animal -> "animal";
            };
        "#;
        assert_eq!(eval(src), str_("animal"));
    }

    #[test]
    fn with_non_null() {
        let src = r#"with (42 as x) x * 2 else 0;"#;
        assert_eq!(eval(src), num(84.0));
    }

    #[test]
    fn with_null_fallback() {
        let src = r#"with (null as x) 999 else 0;"#;
        assert_eq!(eval(src), num(0.0));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 14. BUILT-IN STRING METHODS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn str_length() {
        assert_eq!(eval_expr(r#""hello".length()"#), num(5.0));
    }

    #[test]
    fn str_to_number() {
        assert_eq!(eval_expr(r#""42".toNumber()"#), num(42.0));
    }

    #[test]
    fn str_concat_method() {
        assert_eq!(eval_expr(r#""hello".concat(" world")"#), str_("hello world"));
    }

    #[test]
    fn num_to_string() {
        assert_eq!(eval_expr(r#"42.toString()"#), str_("42"));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // 15. FULL PROGRAMS
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn program_gcd() {
        let src = r#"
            function gcd(a: Number, b: Number): Number ->
                if (b == 0) a else gcd(b, a % b);
            gcd(48, 18);
        "#;
        assert_eq!(eval(src), num(6.0));
    }

    #[test]
    fn program_power_iter() {
        let src = r#"
            function pow(base: Number, exp: Number): Number {
                let result = 1, e = exp in {
                    while (e > 0) {
                        result := result * base;
                        e := e - 1;
                    };
                    result
                }
            }
            pow(2, 8);
        "#;
        assert_eq!(eval(src), num(256.0));
    }

    #[test]
    fn program_oop_area() {
        // Square extends Shape directly (its own ctor, no Rect intermediate)
        // This avoids the issue of Square not providing w/h to Rect's initializers.
        let src = r#"
            class Shape { area(): Number -> 0; }
            class Rect(w: Number, h: Number) is Shape {
                w := w;
                h := h;
                area(): Number -> w * h;
            }
            class Square(s: Number) is Shape {
                s := s;
                area(): Number -> s * s;
            }
            let r = new Rect(3, 4) in
            let sq = new Square(5) in
            r.area() + sq.area();
        "#;
        assert_eq!(eval(src), num(37.0));
    }
}
