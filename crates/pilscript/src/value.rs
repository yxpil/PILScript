//! 运行时值模型。

use crate::foreign::ForeignFn;
use libloading::Library;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Array(Rc<RefCell<Vec<Value>>>),
    Object(Rc<RefCell<Vec<(String, Value)>>>),
    /// 用户定义的函数（含闭包环境）
    Function(Rc<Closure>),
    /// Rust 侧内建函数
    Builtin(Rc<BuiltinFn>),
    /// dlopen 打开的外部库
    NativeLib(Rc<Library>),
    /// 已绑定签名的 FFI 函数
    NativeFn(Rc<ForeignFn>),
    /// lib.fn 工厂（等待签名参数）
    LibFnFactory(Rc<Library>),
}

pub struct BuiltinFn {
    pub name: &'static str,
    pub f: fn(&[Value]) -> Result<Value, String>,
}

pub struct Closure {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub body: Rc<Vec<crate::ast::Stmt>>,
    pub env: Rc<RefCell<crate::interp::Env>>,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Num(_) => "num",
            Value::Str(_) => "str",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Function(_) | Value::Builtin(_) => "function",
            Value::NativeLib(_) | Value::LibFnFactory(_) => "lib",
            Value::NativeFn(_) => "ffn",
        }
    }

    /// JS 风格的真值判断
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Num(n) => *n != 0.0,
            Value::Str(s) => !s.is_empty(),
            _ => true,
        }
    }

    pub fn as_num(&self) -> Result<f64, String> {
        match self {
            Value::Num(n) => Ok(*n),
            other => Err(format!("需要一个数字，但得到 {}", other.type_name())),
        }
    }

    /// 严格相等（参考 JS 的 ===，不做类型强转；数组/对象按引用比较）
    pub fn strict_eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Num(a), Value::Num(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => Rc::ptr_eq(a, b),
            (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            (Value::NativeFn(a), Value::NativeFn(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// 转成用于展示的字符串
    pub fn display(&self) -> String {
        match self {
            Value::Null => "null".into(),
            Value::Bool(b) => b.to_string(),
            Value::Num(n) => {
                if n.is_nan() {
                    "NaN".into()
                } else if n.is_infinite() {
                    if *n > 0.0 { "Infinity".into() } else { "-Infinity".into() }
                } else {
                    format!("{}", n)
                }
            }
            Value::Str(s) => s.clone(),
            Value::Array(arr) => {
                let arr = arr.borrow();
                let items: Vec<String> = arr.iter().map(display_quoted).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Object(obj) => {
                let obj = obj.borrow();
                let items: Vec<String> = obj
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, display_quoted(v)))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
            Value::Function(c) => match &c.name {
                Some(n) => format!("<function {}>", n),
                None => "<function>".into(),
            },
            Value::Builtin(b) => format!("<builtin {}>", b.name),
            Value::NativeLib(_) => "<foreign lib>".into(),
            Value::LibFnFactory(_) => "<foreign lib>".into(),
            Value::NativeFn(f) => format!("<ffn {}>", f.name),
        }
    }
}

/// 数组/对象内部嵌套的字符串加引号（类似 JS console.log 的行为）
fn display_quoted(v: &Value) -> String {
    match v {
        Value::Str(s) => format!("\"{}\"", s),
        other => other.display(),
    }
}

/// 供 interp 使用的对象辅助
pub fn obj_get<'a>(obj: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    obj.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

pub type ObjectMap = Vec<(String, Value)>;
