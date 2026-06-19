pub use hulk_lexer::Span;

// ── Span wrapper ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned { node: f(self.node), span: self.span }
    }
}

// ── Type expressions ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named(String),
    Array(Box<TypeExpr>),
    /// T*  — iterable of T
    Iterable(Box<TypeExpr>),
    /// (T1, T2, ...) -> R  — function type
    Function { params: Vec<TypeExpr>, ret: Box<TypeExpr> },
}

// ── Operators ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg, // -
    Not, // !
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    // Arithmetic
    Add, Sub, Mul, Div, Mod, Pow,
    // String
    Concat,      // @
    ConcatSpace, // @@
    // Comparison
    Eq, Ne, Lt, Le, Gt, Ge,
    // Logical
    And, Or,
}

// ── Expressions ───────────────────────────────────────────────────────────────

pub type ExprS = Spanned<Expr>;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // Literals
    Number(f64),
    Bool(bool),
    Str(String),
    Null,

    // Variables and self
    Var(String),
    Self_,

    // Operators
    Unary { op: UnaryOp, operand: Box<ExprS> },
    Binary { op: BinaryOp, left: Box<ExprS>, right: Box<ExprS> },

    // Destructive assignment:  target := value
    Assign { target: Box<ExprS>, value: Box<ExprS> },

    // let x [: T] = init, ... in body
    Let { bindings: Vec<Binding>, body: Box<ExprS> },

    // if (cond) then [elif (cond) then]* [else fallback]
    If {
        cond: Box<ExprS>,
        then: Box<ExprS>,
        elif_branches: Vec<(ExprS, ExprS)>,
        else_branch: Option<Box<ExprS>>,
    },

    // while (cond) body
    While { cond: Box<ExprS>, body: Box<ExprS> },

    // { expr; expr; ... [expr] }
    Block(Vec<ExprS>),

    // f(args)
    Call { callee: String, args: Vec<ExprS> },

    // obj.method(args)
    MethodCall { object: Box<ExprS>, method: String, args: Vec<ExprS> },

    // obj.field
    FieldAccess { object: Box<ExprS>, field: String },

    // arr[idx]
    Index { array: Box<ExprS>, index: Box<ExprS> },

    // new T(args)
    New { type_name: String, args: Vec<ExprS> },

    // new T[size] [{init}]
    NewArray { type_name: String, size: Box<ExprS>, init: Option<Box<ExprS>> },

    // case expr of { id: T -> body; }
    Case { expr: Box<ExprS>, arms: Vec<CaseArm> },

    // with (expr as id) body else fallback
    With { expr: Box<ExprS>, binding: String, body: Box<ExprS>, fallback: Box<ExprS> },

    // for (var in iter) body
    For { var: String, iter: Box<ExprS>, body: Box<ExprS> },

    // expr is TypeName  (runtime type check)
    IsInstance { expr: Box<ExprS>, type_name: String },

    // expr as TypeName  (downcast)
    Cast { expr: Box<ExprS>, type_name: String },

    // [e1, e2, ...]  (vector literal)
    VecLit { elements: Vec<ExprS> },

    // [expr | var in iter]  (vector comprehension)
    VecComp { body: Box<ExprS>, var: String, iter: Box<ExprS> },

    // base(args)  (call parent class method)
    Base { args: Vec<ExprS> },

    // (x, y) => body  (lambda / anonymous function)
    Lambda { params: Vec<Param>, body: Box<ExprS> },

    // @varname  — by-reference argument at a macro call site
    MacroArgRef(String),

    // $varname  — variable-name capture argument at a macro call site
    MacroArgName(String),

    // match(expr) { case (pat) => body; ... default => body; }
    MacroMatch {
        subject: Box<ExprS>,
        cases: Vec<(ExprS, ExprS)>,
        default_body: Box<ExprS>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
    pub init: ExprS,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaseArm {
    pub binding: String,
    pub type_ann: TypeExpr,
    pub body: ExprS,
    pub span: Span,
}

// ── Parameters ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
    pub span: Span,
}

// ── Declarations ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: ExprS,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub name: String,
    pub ctor_params: Vec<Param>,
    pub base: Option<String>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    Attribute { name: String, init: ExprS, span: Span },
    Method {
        name: String,
        params: Vec<Param>,
        return_type: Option<TypeExpr>,
        body: ExprS,
        span: Span,
    },
}

// ── Protocol declarations ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolDecl {
    pub name: String,
    /// Protocols this one extends (can be multiple)
    pub extends: Vec<String>,
    pub methods: Vec<ProtocolMethod>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
}

// ── Macro declarations (def) ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum MacroParamKind {
    Value,   // regular by-value param
    ByRef,   // @param  — mutates caller's variable
    ByName,  // *param  — lazy / by-name (re-evaluated each use)
    VarName, // $param  — hygienically-bound variable name
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacroParam {
    pub name: String,
    pub kind: MacroParamKind,
    pub type_ann: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacroDecl {
    pub name: String,
    pub params: Vec<MacroParam>,
    pub body: ExprS,
    pub span: Span,
}

// ── Top-level program ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program {
    pub functions: Vec<FunctionDecl>,
    pub classes: Vec<ClassDecl>,
    pub protocols: Vec<ProtocolDecl>,
    pub macros: Vec<MacroDecl>,
    /// Optional entry expression (the "main" expression)
    pub entry: Option<ExprS>,
}
