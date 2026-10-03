//! 抽象语法树（AST）定义。

#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
    Ident(String),
    Array(Vec<Expr>),
    Object(Vec<(String, Expr)>),
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    Logical { op: LogicalOp, left: Box<Expr>, right: Box<Expr> },
    Assign { target: Box<Expr>, value: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr>, line: usize },
    Index { obj: Box<Expr>, index: Box<Expr> },
    Member { obj: Box<Expr>, name: String },
    Fn { params: Vec<String>, body: Vec<Stmt> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { name: String, value: Expr },
    Expr(Expr),
    Return(Option<Expr>),
    If { cond: Expr, then_body: Vec<Stmt>, else_body: Option<Vec<Stmt>> },
    While { cond: Expr, body: Vec<Stmt> },
    For { init: Option<Box<Stmt>>, cond: Option<Expr>, step: Option<Expr>, body: Vec<Stmt> },
    FnDecl { name: String, params: Vec<String>, body: Vec<Stmt> },
    Break,
    Continue,
}
