//! 树遍历求值器：作用域、闭包、控制流。

use crate::ast::*;
use crate::builtins;
use crate::config::Config;
use crate::foreign::ForeignFn;
use crate::value::{Closure, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

pub type InterpResult<T> = Result<T, String>;

/// 环境变量作用域
pub struct Env {
    vars: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    pub fn new() -> Rc<RefCell<Env>> {
        Rc::new(RefCell::new(Env { vars: HashMap::new(), parent: None }))
    }

    pub fn child(parent: &Rc<RefCell<Env>>) -> Rc<RefCell<Env>> {
        Rc::new(RefCell::new(Env { vars: HashMap::new(), parent: Some(parent.clone()) }))
    }

    pub fn define(&mut self, name: impl Into<String>, value: Value) {
        self.vars.insert(name.into(), value);
    }

    pub fn lookup(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.vars.get(name) {
            return Some(v.clone());
        }
        match &self.parent {
            Some(p) => p.borrow().lookup(name),
            None => None,
        }
    }

    /// 给已有变量赋值；未定义时返回 false
    pub fn assign(&mut self, name: &str, value: Value) -> bool {
        if self.vars.contains_key(name) {
            self.vars.insert(name.to_string(), value);
            true
        } else if let Some(p) = &self.parent {
            p.borrow_mut().assign(name, value)
        } else {
            false
        }
    }
}

/// 控制流信号
pub enum Flow {
    Normal,
    Return(Value),
    Break,
    Continue,
}

pub struct Interpreter {
    pub globals: Rc<RefCell<Env>>,
    pub config: Config,
    depth: Cell<usize>,
}

const MAX_CALL_DEPTH: usize = 1500;

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    pub fn with_config(config: Config) -> Self {
        let globals = Env::new();
        builtins::install(&globals, config);
        Interpreter { globals, config, depth: Cell::new(0) }
    }

    pub fn run_program(&mut self, stmts: &[Stmt]) -> InterpResult<()> {
        let globals = self.globals.clone();
        for stmt in stmts {
            self.exec_stmt(stmt, &globals)?;
        }
        Ok(())
    }

    /// 单条语句执行（REPL 也用）
    pub fn exec_stmt(&mut self, stmt: &Stmt, env: &Rc<RefCell<Env>>) -> InterpResult<Flow> {
        match stmt {
            Stmt::Let { name, value } => {
                let v = self.eval_expr(value, env)?;
                env.borrow_mut().define(name.clone(), v);
                Ok(Flow::Normal)
            }
            Stmt::Expr(e) => {
                self.eval_expr(e, env)?;
                Ok(Flow::Normal)
            }
            Stmt::Return(e) => {
                let v = match e {
                    Some(e) => self.eval_expr(e, env)?,
                    None => Value::Null,
                };
                Ok(Flow::Return(v))
            }
            Stmt::If { cond, then_body, else_body } => {
                let c = self.eval_expr(cond, env)?;
                if c.truthy() {
                    self.exec_block(then_body, env)
                } else if let Some(else_body) = else_body {
                    self.exec_block(else_body, env)
                } else {
                    Ok(Flow::Normal)
                }
            }
            Stmt::While { cond, body } => {
                while self.eval_expr(cond, env)?.truthy() {
                    match self.exec_block(body, env)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::For { init, cond, step, body } => {
                let loop_env = Env::child(env);
                if let Some(init) = init {
                    self.exec_stmt(init, &loop_env)?;
                }
                loop {
                    if let Some(cond) = cond {
                        if !self.eval_expr(cond, &loop_env)?.truthy() {
                            break;
                        }
                    }
                    match self.exec_block(body, &loop_env)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                    if let Some(step) = step {
                        self.eval_expr(step, &loop_env)?;
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::FnDecl { name, params, body } => {
                let closure = Closure {
                    name: Some(name.clone()),
                    params: params.clone(),
                    body: body.clone(),
                    env: env.clone(),
                };
                env.borrow_mut().define(name.clone(), Value::Function(Rc::new(closure)));
                Ok(Flow::Normal)
            }
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::Continue),
        }
    }

    fn exec_block(&mut self, stmts: &[Stmt], env: &Rc<RefCell<Env>>) -> InterpResult<Flow> {
        let block_env = Env::child(env);
        for stmt in stmts {
            match self.exec_stmt(stmt, &block_env)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    pub fn eval_expr(&mut self, expr: &Expr, env: &Rc<RefCell<Env>>) -> InterpResult<Value> {
        match expr {
            Expr::Num(n) => Ok(Value::Num(*n)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Null => Ok(Value::Null),
            Expr::Ident { name, line } => env
                .borrow()
                .lookup(name)
                .ok_or_else(|| format!("第 {} 行：未定义的变量 `{}`", line, name)),
            Expr::Array(items) => {
                let mut vals = Vec::with_capacity(items.len());
                for item in items {
                    vals.push(self.eval_expr(item, env)?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(vals))))
            }
            Expr::Object(pairs) => {
                let mut obj: Vec<(String, Value)> = Vec::with_capacity(pairs.len());
                for (k, v) in pairs {
                    obj.push((k.clone(), self.eval_expr(v, env)?));
                }
                Ok(Value::Object(Rc::new(RefCell::new(obj))))
            }
            Expr::Unary { op, expr } => {
                let v = self.eval_expr(expr, env)?;
                match op {
                    UnaryOp::Neg => Ok(Value::Num(-v.as_num()?)),
                    UnaryOp::Not => Ok(Value::Bool(!v.truthy())),
                }
            }
            Expr::Binary { op, left, right } => {
                let l = self.eval_expr(left, env)?;
                let r = self.eval_expr(right, env)?;
                self.binop(*op, &l, &r)
            }
            Expr::Logical { op, left, right } => {
                let l = self.eval_expr(left, env)?;
                match op {
                    LogicalOp::And => {
                        if l.truthy() {
                            self.eval_expr(right, env)
                        } else {
                            Ok(l)
                        }
                    }
                    LogicalOp::Or => {
                        if l.truthy() {
                            Ok(l)
                        } else {
                            self.eval_expr(right, env)
                        }
                    }
                }
            }
            Expr::Assign { target, value } => {
                let v = self.eval_expr(value, env)?;
                self.assign_to(target, v, env)
            }
            Expr::Fn { params, body } => {
                let closure = Closure {
                    name: None,
                    params: params.clone(),
                    body: body.clone(),
                    env: env.clone(),
                };
                Ok(Value::Function(Rc::new(closure)))
            }
            Expr::Call { callee, args, line } => {
                let f = self.eval_expr(callee, env)?;
                let mut argv = Vec::with_capacity(args.len());
                for a in args {
                    argv.push(self.eval_expr(a, env)?);
                }
                self.call_value(&f, &argv, *line)
            }
            Expr::Index { obj, index } => {
                let o = self.eval_expr(obj, env)?;
                let i = self.eval_expr(index, env)?;
                match (&o, &i) {
                    (Value::Array(arr), Value::Num(n)) => {
                        let arr = arr.borrow();
                        let idx = *n as isize;
                        let len = arr.len() as isize;
                        let real = if idx < 0 { len + idx } else { idx };
                        if real >= 0 && real < len {
                            Ok(arr[real as usize].clone())
                        } else {
                            Ok(Value::Null)
                        }
                    }
                    (Value::Str(s), Value::Num(n)) => {
                        let chars: Vec<char> = s.chars().collect();
                        let idx = *n as isize;
                        let len = chars.len() as isize;
                        let real = if idx < 0 { len + idx } else { idx };
                        if real >= 0 && real < len {
                            Ok(Value::Str(chars[real as usize].to_string()))
                        } else {
                            Ok(Value::Null)
                        }
                    }
                    (Value::Object(o), Value::Str(k)) => {
                        let o = o.borrow();
                        Ok(crate::value::obj_get(&o, k).cloned().unwrap_or(Value::Null))
                    }
                    (o, i) => Err(format!(
                        "不支持对 {} 用 {}（{}）做索引访问",
                        o.type_name(),
                        i.type_name(),
                        i.display()
                    )),
                }
            }
            Expr::Member { obj, name } => {
                let o = self.eval_expr(obj, env)?;
                self.get_member(&o, name)
            }
        }
    }

    fn binop(&self, op: BinOp, l: &Value, r: &Value) -> InterpResult<Value> {
        use BinOp::*;
        match op {
            Add => match (l, r) {
                (Value::Num(a), Value::Num(b)) => Ok(Value::Num(a + b)),
                (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{}{}", a, b))),
                (Value::Str(a), b) => Ok(Value::Str(format!("{}{}", a, b.display()))),
                (a, Value::Str(b)) => Ok(Value::Str(format!("{}{}", a.display(), b))),
                (a, b) => Err(format!(
                    "不支持 {} 和 {} 相加（字符串拼接请先 str() 转换）",
                    a.type_name(),
                    b.type_name()
                )),
            },
            _ => {
                // 其余运算符只做数值运算；相等性比较除外
                match op {
                    Eq => Ok(Value::Bool(l.strict_eq(r))),
                    Ne => Ok(Value::Bool(!l.strict_eq(r))),
                    _ => {
                        let (a, b) = match (l, r) {
                            (Value::Num(a), Value::Num(b)) => (*a, *b),
                            _ => {
                                return Err(format!(
                                    "运算符需要数字操作数，但得到 {} 和 {}",
                                    l.type_name(),
                                    r.type_name()
                                ))
                            }
                        };
                        match op {
                            Sub => Ok(Value::Num(a - b)),
                            Mul => Ok(Value::Num(a * b)),
                            Div => {
                                if b == 0.0 {
                                    Err("除以零".into())
                                } else {
                                    Ok(Value::Num(a / b))
                                }
                            }
                            Mod => {
                                if b == 0.0 {
                                    Err("对零取模".into())
                                } else {
                                    Ok(Value::Num(a % b))
                                }
                            }
                            Lt => Ok(Value::Bool(a < b)),
                            Le => Ok(Value::Bool(a <= b)),
                            Gt => Ok(Value::Bool(a > b)),
                            Ge => Ok(Value::Bool(a >= b)),
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
    }

    fn assign_to(&mut self, target: &Expr, value: Value, env: &Rc<RefCell<Env>>) -> InterpResult<Value> {
        match target {
            Expr::Ident { name, line } => {
                if env.borrow_mut().assign(name, value.clone()) {
                    Ok(value)
                } else {
                    Err(format!(
                        "第 {} 行：不能给未定义的变量 `{}` 赋值（请先用 let 声明）",
                        line, name
                    ))
                }
            }
            Expr::Index { obj, index } => {
                let o = self.eval_expr(obj, env)?;
                let i = self.eval_expr(index, env)?;
                match (&o, &i) {
                    (Value::Array(arr), Value::Num(n)) => {
                        let mut arr = arr.borrow_mut();
                        let idx = *n as isize;
                        let len = arr.len() as isize;
                        let real = if idx < 0 { len + idx } else { idx };
                        if real >= 0 && real <= len {
                            if real == len {
                                arr.push(value.clone());
                            } else {
                                arr[real as usize] = value.clone();
                            }
                            Ok(value)
                        } else {
                            Err(format!("数组下标越界: {}", n))
                        }
                    }
                    (Value::Object(o), Value::Str(k)) => {
                        let mut o = o.borrow_mut();
                        match o.iter_mut().find(|(key, _)| key == k) {
                            Some(slot) => slot.1 = value.clone(),
                            None => o.push((k.clone(), value.clone())),
                        }
                        Ok(value)
                    }
                    (o, i) => Err(format!(
                        "不支持给 {} 的 {}（{}）索引赋值",
                        o.type_name(),
                        i.type_name(),
                        i.display()
                    )),
                }
            }
            Expr::Member { obj, name } => {
                let o = self.eval_expr(obj, env)?;
                match &o {
                    Value::Object(map) => {
                        let mut map = map.borrow_mut();
                        match map.iter_mut().find(|(k, _)| k == name) {
                            Some(slot) => slot.1 = value.clone(),
                            None => map.push((name.clone(), value.clone())),
                        }
                        Ok(value)
                    }
                    other => Err(format!("不支持给 {} 的属性 `.` 赋值", other.type_name())),
                }
            }
            _ => Err("无效的赋值目标".into()),
        }
    }

    fn get_member(&self, obj: &Value, name: &str) -> InterpResult<Value> {
        match obj {
            Value::Object(map) => {
                let map = map.borrow();
                Ok(crate::value::obj_get(&map, name).cloned().unwrap_or(Value::Null))
            }
            Value::Array(arr) => match name {
                "length" => Ok(Value::Num(arr.borrow().len() as f64)),
                _ => Err(format!("数组没有属性 `{}`（只有 .length）", name)),
            },
            Value::NativeLib(_) | Value::LibFnFactory(_) => match name {
                "fn" => {
                    let lib = match obj {
                        Value::NativeLib(l) => l.clone(),
                        Value::LibFnFactory(l) => l.clone(),
                        _ => unreachable!(),
                    };
                    Ok(Value::LibFnFactory(lib))
                }
                _ => Err(format!("外部库对象只有 `.fn` 方法（想用 `{}`？）", name)),
            },
            other => Err(format!("{} 类型没有成员访问", other.type_name())),
        }
    }

    /// 调用一个值（函数 / 内建 / FFI）
    pub fn call_value(&mut self, f: &Value, args: &[Value], line: usize) -> InterpResult<Value> {
        match f {
            Value::Function(closure) => {
                if self.depth.get() >= MAX_CALL_DEPTH {
                    return Err(format!(
                        "第 {} 行：调用深度超过 {}，可能是无限递归",
                        line, MAX_CALL_DEPTH
                    ));
                }
                if args.len() != closure.params.len() {
                    return Err(format!(
                        "第 {} 行：函数需要 {} 个参数，但传入了 {} 个",
                        line,
                        closure.params.len(),
                        args.len()
                    ));
                }
                let call_env = Env::child(&closure.env);
                for (p, a) in closure.params.iter().zip(args) {
                    call_env.borrow_mut().define(p.clone(), a.clone());
                }
                self.depth.set(self.depth.get() + 1);
                let result = self.exec_block(&closure.body, &call_env);
                self.depth.set(self.depth.get() - 1);
                match result? {
                    Flow::Return(v) => Ok(v),
                    _ => Ok(Value::Null),
                }
            }
            Value::Builtin(b) => (b.f)(args).map_err(|e| format!("第 {} 行：{}: {}", line, b.name, e)),
            Value::NativeFn(ffn) => ffn.call(args).map_err(|e| format!("第 {} 行：FFI {}: {}", line, ffn.name, e)),
            Value::LibFnFactory(lib) => {
                // lib.fn("名字", [参数类型], 返回类型)
                if args.len() != 3 {
                    return Err(format!(
                        "第 {} 行：lib.fn(名字, [参数类型数组], 返回类型) 需要 3 个参数，但传入了 {} 个",
                        line,
                        args.len()
                    ));
                }
                let name = match &args[0] {
                    Value::Str(s) => s.clone(),
                    other => return Err(format!("第 {} 行：符号名需要字符串，但得到 {}", line, other.type_name())),
                };
                let arg_tys = match &args[1] {
                    Value::Array(arr) => {
                        let arr = arr.borrow();
                        let mut tys = Vec::with_capacity(arr.len());
                        for t in arr.iter() {
                            match t {
                                Value::Str(s) => tys.push(crate::foreign::ArgTy::parse(s)?),
                                other => {
                                    return Err(format!(
                                        "第 {} 行：参数类型数组里应是字符串，但得到 {}",
                                        line,
                                        other.type_name()
                                    ))
                                }
                            }
                        }
                        tys
                    }
                    other => {
                        return Err(format!(
                            "第 {} 行：参数类型应是数组，但得到 {}",
                            line,
                            other.type_name()
                        ))
                    }
                };
                let ret = match &args[2] {
                    Value::Str(s) => crate::foreign::RetTy::parse(s)?,
                    other => {
                        return Err(format!(
                            "第 {} 行：返回类型应是字符串，但得到 {}",
                            line,
                            other.type_name()
                        ))
                    }
                };
                let ffn = ForeignFn::bind(lib.clone(), name, arg_tys, ret)?;
                Ok(Value::NativeFn(ffn))
            }
            other => Err(format!(
                "第 {} 行：{} 不能被调用",
                line,
                other.type_name()
            )),
        }
    }
}
