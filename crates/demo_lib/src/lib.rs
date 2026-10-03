//! 一个用 Rust 写成 C-ABI 动态库的示例，
//! 用来演示 PILScript 的跨语言 FFI 调用。

#[no_mangle]
pub extern "C" fn add_i64(a: i64, b: i64) -> i64 {
    a + b
}

#[no_mangle]
pub extern "C" fn mul_f64(a: f64, b: f64) -> f64 {
    a * b
}

#[no_mangle]
pub extern "C" fn mix(a: i64, b: f64, c: i64) -> f64 {
    a as f64 + b * c as f64
}

#[no_mangle]
pub extern "C" fn sqrt_f64(x: f64) -> f64 {
    x.sqrt()
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn shout(s: *const std::os::raw::c_char) -> *const std::os::raw::c_char {
    // 把传入的 C 字符串转成大写后返回（泄漏一块内存保存结果，演示场景可接受）
    use std::ffi::{CStr, CString};
    unsafe {
        let cstr = CStr::from_ptr(s);
        let up = cstr.to_string_lossy().to_uppercase();
        CString::into_raw(CString::new(up).unwrap()) as *const std::os::raw::c_char
    }
}

#[no_mangle]
pub extern "C" fn is_even(n: i64) -> bool {
    n % 2 == 0
}
