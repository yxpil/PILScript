//! FFI：通过 libloading 加载任意 C-ABI 动态库（.dll/.so/.dylib）并调用其中的函数。
//!
//! 这意味着 PILScript 可以直接调用 C / C++ / Rust / Zig / Nim / Go(cgo) 等
//! 任何能导出 C ABI 符号的语言编写的库。
//!
//! PILScript 侧用法：
//! ```text
//! let lib = dlopen("demo_lib.dll");
//! let add = lib.fn("add_i64", ["i64", "i64"], "i64");
//! print(add(1, 2));
//! ```

use crate::value::Value;
use libloading::Library;
use std::ffi::{CStr, CString};
use std::os::raw::c_void;
use std::rc::Rc;

// ============ 调用分发宏 ============
//
// slot_ty!  : 槽位标识 -> Rust 类型
// slot_arg! : 槽位标识 + 参数下标 -> 实参表达式
// call_with_ret! : 按(槽位组合, 返回类型)生成转译后的调用

macro_rules! slot_ty {
    (I) => { i64 };
    (D) => { f64 };
}

macro_rules! slot_arg {
    (I, $vals:expr, $i:expr) => {
        match &$vals[$i] {
            CVal::Int(x) => *x,
            CVal::Double(_) => return Err("内部错误：参数槽位不匹配".into()),
        }
    };
    (D, $vals:expr, $i:expr) => {
        match &$vals[$i] {
            CVal::Double(x) => *x,
            CVal::Int(_) => return Err("内部错误：参数槽位不匹配".into()),
        }
    };
}

macro_rules! call_with_ret {
    ($ptr:expr, $vals:expr, $ret:expr, $($t:ident, $i:expr),*) => {{
        match $ret {
            RetTy::Void => {
                let f: unsafe extern "C" fn($(slot_ty!($t),)*) = std::mem::transmute($ptr);
                f($(slot_arg!($t, $vals, $i)),*);
                Ok(Value::Null)
            }
            RetTy::I32 => {
                let f: unsafe extern "C" fn($(slot_ty!($t),)*) -> i32 = std::mem::transmute($ptr);
                Ok(Value::Num(f($(slot_arg!($t, $vals, $i)),*) as f64))
            }
            RetTy::I64 => {
                let f: unsafe extern "C" fn($(slot_ty!($t),)*) -> i64 = std::mem::transmute($ptr);
                Ok(Value::Num(f($(slot_arg!($t, $vals, $i)),*) as f64))
            }
            RetTy::F32 => {
                let f: unsafe extern "C" fn($(slot_ty!($t),)*) -> f32 = std::mem::transmute($ptr);
                Ok(Value::Num(f($(slot_arg!($t, $vals, $i)),*) as f64))
            }
            RetTy::F64 => {
                let f: unsafe extern "C" fn($(slot_ty!($t),)*) -> f64 = std::mem::transmute($ptr);
                Ok(Value::Num(f($(slot_arg!($t, $vals, $i)),*)))
            }
            RetTy::Bool => {
                // C 的 bool 返回值只保证最低字节有效，掩码后转 bool
                let f: unsafe extern "C" fn($(slot_ty!($t),)*) -> i32 = std::mem::transmute($ptr);
                Ok(Value::Bool(f($(slot_arg!($t, $vals, $i)),*) & 0xFF != 0))
            }
            RetTy::Str => {
                // 返回值是 C 字符串指针；NULL 映射为 null
                let f: unsafe extern "C" fn($(slot_ty!($t),)*) -> *const std::os::raw::c_char =
                    std::mem::transmute($ptr);
                let p = f($(slot_arg!($t, $vals, $i)),*);
                if p.is_null() {
                    Ok(Value::Null)
                } else {
                    Ok(Value::Str(CStr::from_ptr(p).to_string_lossy().into_owned()))
                }
            }
            RetTy::Ptr => {
                let f: unsafe extern "C" fn($(slot_ty!($t),)*) -> *const c_void =
                    std::mem::transmute($ptr);
                let p = f($(slot_arg!($t, $vals, $i)),*);
                Ok(Value::Num((p as usize) as f64))
            }
        }
    }};
}

/// 参数类型声明
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArgTy {
    I32,
    I64,
    F64,
    Bool,
    Str,
    Ptr,
}

/// 返回类型声明
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RetTy {
    Void,
    I32,
    I64,
    F32,
    F64,
    Bool,
    Str,
    Ptr,
}

impl ArgTy {
    pub fn parse(s: &str) -> Result<ArgTy, String> {
        match s {
            "i32" | "int" | "int32" => Ok(ArgTy::I32),
            "i64" | "int64" | "long" => Ok(ArgTy::I64),
            "f64" | "double" => Ok(ArgTy::F64),
            "bool" => Ok(ArgTy::Bool),
            "str" | "string" => Ok(ArgTy::Str),
            "ptr" | "pointer" => Ok(ArgTy::Ptr),
            other => Err(format!(
                "未知的 FFI 参数类型 `{}`（支持 i32/i64/f64/bool/str/ptr）",
                other
            )),
        }
    }
}

impl RetTy {
    pub fn parse(s: &str) -> Result<RetTy, String> {
        match s {
            "void" | "()" => Ok(RetTy::Void),
            "i32" | "int" | "int32" => Ok(RetTy::I32),
            "i64" | "int64" | "long" => Ok(RetTy::I64),
            "f32" | "float" => Ok(RetTy::F32),
            "f64" | "double" => Ok(RetTy::F64),
            "bool" => Ok(RetTy::Bool),
            "str" | "string" => Ok(RetTy::Str),
            "ptr" | "pointer" => Ok(RetTy::Ptr),
            other => Err(format!(
                "未知的 FFI 返回类型 `{}`（支持 void/i32/i64/f32/f64/bool/str/ptr）",
                other
            )),
        }
    }
}

/// 已转换到槽位表示的参数（整数族统一走 i64 槽，浮点走 f64 槽）
#[derive(Clone, Copy)]
pub enum CVal {
    Int(i64),
    Double(f64),
}

/// 槽位种类（用于在编译期选择函数指针类型）
#[derive(Clone, Copy, PartialEq)]
pub enum Slot {
    I,
    D,
}

/// 一个已绑定签名的 FFI 函数
pub struct ForeignFn {
    pub name: String,
    /// 持有 Library 保证符号在函数存活期间有效
    #[allow(dead_code)]
    lib: Rc<Library>,
    ptr: *const c_void,
    arg_tys: Vec<ArgTy>,
    slots: Vec<Slot>,
    ret: RetTy,
}

// 裸函数指针只读，跨线程共享是安全的（调用仍由单线程解释器发起）
unsafe impl Send for ForeignFn {}

impl ForeignFn {
    /// 从 lib.fn(名字, [参数类型], 返回类型) 构造
    pub fn bind(
        lib: Rc<Library>,
        name: String,
        arg_tys: Vec<ArgTy>,
        ret: RetTy,
    ) -> Result<Rc<ForeignFn>, String> {
        let symbol: libloading::Symbol<unsafe extern "C" fn()> = unsafe {
            lib.get(name.as_bytes())
                .map_err(|e| format!("在动态库中找不到符号 `{}`: {}", name, e))?
        };
        let ptr = *symbol as *const std::ffi::c_void;
        let slots = arg_tys
            .iter()
            .map(|t| match t {
                ArgTy::F64 => Slot::D,
                _ => Slot::I,
            })
            .collect();
        Ok(Rc::new(ForeignFn { name, lib, ptr, arg_tys, slots, ret }))
    }

    pub fn arity(&self) -> usize {
        self.arg_tys.len()
    }

    /// 把 PILScript 值按声明类型转换后调用
    pub fn call(&self, args: &[Value]) -> Result<Value, String> {
        if args.len() != self.arg_tys.len() {
            return Err(format!(
                "FFI 函数 `{}` 需要 {} 个参数，但传入了 {} 个",
                self.name,
                self.arg_tys.len(),
                args.len()
            ));
        }
        // 字符串参数需要保活到调用结束
        let mut keepalive: Vec<CString> = Vec::new();
        let mut vals: Vec<CVal> = Vec::with_capacity(args.len());
        for (v, ty) in args.iter().zip(&self.arg_tys) {
            match (v, ty) {
                (Value::Num(n), ArgTy::I32) => vals.push(CVal::Int(*n as i32 as i64)),
                (Value::Num(n), ArgTy::I64) => vals.push(CVal::Int(*n as i64)),
                (Value::Num(n), ArgTy::F64) => vals.push(CVal::Double(*n)),
                (Value::Bool(b), ArgTy::Bool) => vals.push(CVal::Int(*b as i64)),
                (Value::Bool(b), ArgTy::I32) => vals.push(CVal::Int(*b as i64)),
                (Value::Str(s), ArgTy::Str) => {
                    let cs = CString::new(s.as_str())
                        .map_err(|_| "FFI 字符串参数不能包含 \\0 字节".to_string())?;
                    vals.push(CVal::Int(cs.as_ptr() as i64));
                    keepalive.push(cs);
                }
                (Value::Num(n), ArgTy::Ptr) => {
                    vals.push(CVal::Int(*n as i64));
                }
                (v, t) => {
                    return Err(format!(
                        "FFI 函数 `{}` 的参数类型声明为 {:?}，但传入 {}（{}）",
                        self.name,
                        t,
                        v.type_name(),
                        v.display()
                    ))
                }
            }
        }
        unsafe { self.dispatch(&vals) }
    }

    /// 根据槽位组合选择编译期确定的函数指针类型并调用
    unsafe fn dispatch(&self, vals: &[CVal]) -> Result<Value, String> {
        let ptr = self.ptr;
        let ret = self.ret;
        match &self.slots[..] {
            [] => call_with_ret!(ptr, vals, ret, ),
            [Slot::I] => call_with_ret!(ptr, vals, ret, I, 0),
            [Slot::D] => call_with_ret!(ptr, vals, ret, D, 0),
            [Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, I, 1),
            [Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, D, 1),
            [Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, I, 1),
            [Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, D, 1),
            [Slot::I, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, I, 2),
            [Slot::I, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, D, 2),
            [Slot::I, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, I, 2),
            [Slot::I, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, D, 2),
            [Slot::D, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, I, 2),
            [Slot::D, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, D, 2),
            [Slot::D, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, I, 2),
            [Slot::D, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, D, 2),
            [Slot::I, Slot::I, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, I, 2, I, 3),
            [Slot::I, Slot::I, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, I, 2, D, 3),
            [Slot::I, Slot::I, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, D, 2, I, 3),
            [Slot::I, Slot::I, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, D, 2, D, 3),
            [Slot::I, Slot::D, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, I, 2, I, 3),
            [Slot::I, Slot::D, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, I, 2, D, 3),
            [Slot::I, Slot::D, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, D, 2, I, 3),
            [Slot::I, Slot::D, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, D, 2, D, 3),
            [Slot::D, Slot::I, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, I, 2, I, 3),
            [Slot::D, Slot::I, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, I, 2, D, 3),
            [Slot::D, Slot::I, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, D, 2, I, 3),
            [Slot::D, Slot::I, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, D, 2, D, 3),
            [Slot::D, Slot::D, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, I, 2, I, 3),
            [Slot::D, Slot::D, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, I, 2, D, 3),
            [Slot::D, Slot::D, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, D, 2, I, 3),
            [Slot::D, Slot::D, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, D, 2, D, 3),
            [Slot::I, Slot::I, Slot::I, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, I, 2, I, 3, I, 4),
            [Slot::I, Slot::I, Slot::I, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, I, 2, I, 3, D, 4),
            [Slot::I, Slot::I, Slot::I, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, I, 2, D, 3, I, 4),
            [Slot::I, Slot::I, Slot::I, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, I, 2, D, 3, D, 4),
            [Slot::I, Slot::I, Slot::D, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, D, 2, I, 3, I, 4),
            [Slot::I, Slot::I, Slot::D, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, D, 2, I, 3, D, 4),
            [Slot::I, Slot::I, Slot::D, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, D, 2, D, 3, I, 4),
            [Slot::I, Slot::I, Slot::D, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, I, 1, D, 2, D, 3, D, 4),
            [Slot::I, Slot::D, Slot::I, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, I, 2, I, 3, I, 4),
            [Slot::I, Slot::D, Slot::I, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, I, 2, I, 3, D, 4),
            [Slot::I, Slot::D, Slot::I, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, I, 2, D, 3, I, 4),
            [Slot::I, Slot::D, Slot::I, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, I, 2, D, 3, D, 4),
            [Slot::I, Slot::D, Slot::D, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, D, 2, I, 3, I, 4),
            [Slot::I, Slot::D, Slot::D, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, D, 2, I, 3, D, 4),
            [Slot::I, Slot::D, Slot::D, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, D, 2, D, 3, I, 4),
            [Slot::I, Slot::D, Slot::D, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, I, 0, D, 1, D, 2, D, 3, D, 4),
            [Slot::D, Slot::I, Slot::I, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, I, 2, I, 3, I, 4),
            [Slot::D, Slot::I, Slot::I, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, I, 2, I, 3, D, 4),
            [Slot::D, Slot::I, Slot::I, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, I, 2, D, 3, I, 4),
            [Slot::D, Slot::I, Slot::I, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, I, 2, D, 3, D, 4),
            [Slot::D, Slot::I, Slot::D, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, D, 2, I, 3, I, 4),
            [Slot::D, Slot::I, Slot::D, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, D, 2, I, 3, D, 4),
            [Slot::D, Slot::I, Slot::D, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, D, 2, D, 3, I, 4),
            [Slot::D, Slot::I, Slot::D, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, I, 1, D, 2, D, 3, D, 4),
            [Slot::D, Slot::D, Slot::I, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, I, 2, I, 3, I, 4),
            [Slot::D, Slot::D, Slot::I, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, I, 2, I, 3, D, 4),
            [Slot::D, Slot::D, Slot::I, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, I, 2, D, 3, I, 4),
            [Slot::D, Slot::D, Slot::I, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, I, 2, D, 3, D, 4),
            [Slot::D, Slot::D, Slot::D, Slot::I, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, D, 2, I, 3, I, 4),
            [Slot::D, Slot::D, Slot::D, Slot::I, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, D, 2, I, 3, D, 4),
            [Slot::D, Slot::D, Slot::D, Slot::D, Slot::I] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, D, 2, D, 3, I, 4),
            [Slot::D, Slot::D, Slot::D, Slot::D, Slot::D] => call_with_ret!(ptr, vals, ret, D, 0, D, 1, D, 2, D, 3, D, 4),
            // 元数 > 5 的签名不支持
            _ => Err(format!("FFI 暂不支持超过 5 个参数的签名（本函数有 {} 个）", vals.len())),
        }
    }
}
