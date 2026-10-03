//! 内建函数库。

use crate::config::Config;
use crate::interp::Env;
use crate::value::{BuiltinFn, Value};
use libloading::Library;
use std::cell::RefCell;
use std::rc::Rc;

fn builtin(name: &'static str, f: fn(&[Value]) -> Result<Value, String>) -> Value {
    Value::Builtin(Rc::new(BuiltinFn { name, f }))
}

pub fn install(env: &Rc<RefCell<Env>>, config: Config) {
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
    e.define("time", builtin("time", bi_time));

    // 能力受权限控制的内建：被禁用时注册"拒绝桩"，调用时给出清晰告警
    if config.allow_ffi {
        e.define("dlopen", builtin("dlopen", bi_dlopen));
    } else {
        e.define("dlopen", builtin("dlopen", deny_ffi));
    }
    if config.allow_fs {
        e.define("read_file", builtin("read_file", bi_read_file));
        e.define("write_file", builtin("write_file", bi_write_file));
        e.define("append_file", builtin("append_file", bi_append_file));
        e.define("exists", builtin("exists", bi_exists));
    } else {
        e.define("read_file", builtin("read_file", deny_fs));
        e.define("write_file", builtin("write_file", deny_fs));
        e.define("append_file", builtin("append_file", deny_fs));
        e.define("exists", builtin("exists", deny_fs));
    }
}

fn deny_ffi(_args: &[Value]) -> Result<Value, String> {
    Err("[权限拒绝] FFI 能力已被禁用（启动时使用 --no-ffi）".into())
}

fn deny_fs(_args: &[Value]) -> Result<Value, String> {
    Err("[权限拒绝] 文件系统访问已被禁用（启动时使用 --no-fs）".into())
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

// ============ 文件系统（持久化）内建 ============

fn bi_read_file(args: &[Value]) -> Result<Value, String> {
    want(args, 1, "read_file")?;
    match &args[0] {
        Value::Str(path) => match std::fs::read_to_string(path) {
            Ok(s) => Ok(Value::Str(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
            Err(e) => Err(format!("read_file(\"{}\") 失败: {}", path, e)),
        },
        other => Err(format!("read_file() 需要字符串路径，但得到 {}", other.type_name())),
    }
}

fn bi_write_file(args: &[Value]) -> Result<Value, String> {
    want(args, 2, "write_file")?;
    let path = match &args[0] {
        Value::Str(s) => s.clone(),
        other => return Err(format!("write_file() 路径需要字符串，但得到 {}", other.type_name())),
    };
    let content = match &args[1] {
        Value::Str(s) => s.clone(),
        other => other.display(),
    };
    std::fs::write(&path, content.as_bytes())
        .map(|_| Value::Num(content.len() as f64))
        .map_err(|e| format!("write_file(\"{}\") 失败: {}", path, e))
}

fn bi_append_file(args: &[Value]) -> Result<Value, String> {
    want(args, 2, "append_file")?;
    let path = match &args[0] {
        Value::Str(s) => s.clone(),
        other => return Err(format!("append_file() 路径需要字符串，但得到 {}", other.type_name())),
    };
    let content = match &args[1] {
        Value::Str(s) => s.clone(),
        other => other.display(),
    };
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("append_file(\"{}\") 失败: {}", path, e))?;
    f.write_all(content.as_bytes())
        .map(|_| Value::Num(content.len() as f64))
        .map_err(|e| format!("append_file(\"{}\") 写入失败: {}", path, e))
}

fn bi_exists(args: &[Value]) -> Result<Value, String> {
    want(args, 1, "exists")?;
    match &args[0] {
        Value::Str(path) => Ok(Value::Bool(std::path::Path::new(path).exists())),
        other => Err(format!("exists() 需要字符串路径，但得到 {}", other.type_name())),
    }
}

// ============ 时间内建（供脚本自测性能） ============

fn bi_time(args: &[Value]) -> Result<Value, String> {
    want(args, 0, "time")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("time() 获取系统时间失败: {}", e))?;
    Ok(Value::Num(now.as_millis() as f64))
}
