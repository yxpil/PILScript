//! 内建函数库。

use crate::interp::Env;
use crate::value::{BuiltinFn, Value};
use libloading::Library;
use std::cell::RefCell;
use std::rc::Rc;

fn builtin(name: &'static str, f: fn(&[Value]) -> Result<Value, String>) -> Value {
    Value::Builtin(Rc::new(BuiltinFn { name, f }))
}

pub fn install(env: &Rc<RefCell<Env>>) {
    let mut e = env.borrow_mut();

    e.define("print", builtin("print", bi_print));
    e.define("len", builtin("len", bi_len));
    e.define("str", builtin("str", bi_str));
    e.define("num", builtin("num", bi_num));
    e.define("type", builtin("type", bi_type));
    e.define("push", builtin("push", bi_push));
    e.define("pop", builtin("pop", bi_pop));
    e.define("keys", builtin("keys", bi_keys));
    e.define("abs", builtin("abs", bi_abs));
    e.define("floor", builtin("floor", bi_floor));
    e.define("ceil", builtin("ceil", bi_ceil));
    e.define("sqrt", builtin("sqrt", bi_sqrt));
    e.define("min", builtin("min", bi_min));
    e.define("max", builtin("max", bi_max));
    e.define("assert", builtin("assert", bi_assert));
    e.define("dlopen", builtin("dlopen", bi_dlopen));
}

fn want(args: &[Value], n: usize, name: &str) -> Result<(), String> {
    if args.len() != n {
        Err(format!("{}() 需要 {} 个参数，但传入了 {} 个", name, n, args.len()))
    } else {
        Ok(())
    }
}

fn bi_print(args: &[Value]) -> Result<Value, String> {
    let parts: Vec<String> = args.iter().map(|v| v.display()).collect();
    println!("{}", parts.join(" "));
    Ok(Value::Null)
}

fn bi_len(args: &[Value]) -> Result<Value, String> {
    want(args, 1, "len")?;
    Ok(Value::Num(match &args[0] {
        Value::Str(s) => s.chars().count() as f64,
        Value::Array(a) => a.borrow().len() as f64,
        Value::Object(o) => o.borrow().len() as f64,
        other => return Err(format!("len() 不支持 {} 类型", other.type_name())),
    }))
}

fn bi_str(args: &[Value]) -> Result<Value, String> {
    want(args, 1, "str")?;
    Ok(Value::Str(args[0].display()))
}

fn bi_num(args: &[Value]) -> Result<Value, String> {
    want(args, 1, "num")?;
    match &args[0] {
        Value::Num(n) => Ok(Value::Num(*n)),
        Value::Bool(b) => Ok(Value::Num(*b as i32 as f64)),
        Value::Str(s) => s.trim().parse::<f64>().map(Value::Num).map_err(|_| {
            format!("num() 无法把字符串 \"{}\" 转成数字", s)
        }),
        other => Err(format!("num() 不支持 {} 类型", other.type_name())),
    }
}

fn bi_type(args: &[Value]) -> Result<Value, String> {
    want(args, 1, "type")?;
    Ok(Value::Str(args[0].type_name().into()))
}

fn bi_push(args: &[Value]) -> Result<Value, String> {
    want(args, 2, "push")?;
    match &args[0] {
        Value::Array(a) => {
            a.borrow_mut().push(args[1].clone());
            Ok(args[0].clone())
        }
        other => Err(format!("push() 第一个参数需要数组，但得到 {}", other.type_name())),
    }
}

fn bi_pop(args: &[Value]) -> Result<Value, String> {
    want(args, 1, "pop")?;
    match &args[0] {
        Value::Array(a) => Ok(a.borrow_mut().pop().unwrap_or(Value::Null)),
        other => Err(format!("pop() 需要数组，但得到 {}", other.type_name())),
    }
}

fn bi_keys(args: &[Value]) -> Result<Value, String> {
    want(args, 1, "keys")?;
    match &args[0] {
        Value::Object(o) => {
            let o = o.borrow();
            Ok(Value::Array(Rc::new(RefCell::new(
                o.iter().map(|(k, _)| Value::Str(k.clone())).collect(),
            ))))
        }
        other => Err(format!("keys() 需要对象，但得到 {}", other.type_name())),
    }
}

fn unary_math(name: &str, args: &[Value], f: impl Fn(f64) -> f64) -> Result<Value, String> {
    want(args, 1, name)?;
    Ok(Value::Num(f(args[0].as_num().map_err(|e| format!("{}(): {}", name, e))?)))
}

fn bi_abs(args: &[Value]) -> Result<Value, String> {
    unary_math("abs", args, f64::abs)
}

fn bi_floor(args: &[Value]) -> Result<Value, String> {
    unary_math("floor", args, f64::floor)
}

fn bi_ceil(args: &[Value]) -> Result<Value, String> {
    unary_math("ceil", args, f64::ceil)
}

fn bi_sqrt(args: &[Value]) -> Result<Value, String> {
    unary_math("sqrt", args, f64::sqrt)
}

fn bi_min(args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 {
        return Err("min() 至少需要 2 个参数".into());
    }
    let mut best = args[0].as_num()?;
    for a in &args[1..] {
        let n = a.as_num()?;
        if n < best {
            best = n;
        }
    }
    Ok(Value::Num(best))
}

fn bi_max(args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 {
        return Err("max() 至少需要 2 个参数".into());
    }
    let mut best = args[0].as_num()?;
    for a in &args[1..] {
        let n = a.as_num()?;
        if n > best {
            best = n;
        }
    }
    Ok(Value::Num(best))
}

fn bi_assert(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("assert(条件 [, 消息])".into());
    }
    if args[0].truthy() {
        Ok(Value::Null)
    } else {
        match args.get(1) {
            Some(Value::Str(msg)) => Err(format!("断言失败: {}", msg)),
            _ => Err("断言失败".into()),
        }
    }
}

fn bi_dlopen(args: &[Value]) -> Result<Value, String> {
    want(args, 1, "dlopen")?;
    match &args[0] {
        Value::Str(path) => unsafe {
            Library::new(path)
                .map(|l| Value::NativeLib(Rc::new(l)))
                .map_err(|e| format!("dlopen(\"{}\") 失败: {}", path, e))
        },
        other => Err(format!("dlopen() 需要字符串路径，但得到 {}", other.type_name())),
    }
}
