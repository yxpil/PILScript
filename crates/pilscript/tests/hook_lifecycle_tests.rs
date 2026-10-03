//! 嵌入钩子（FFI 回调注册）生命周期测试。
//!
//! PILScript 没有事件总线；它的"钩子/回调"机制是嵌入方把外部动态库函数
//! **注册**进来（`dlopen` → `lib.fn(名字, 参数类型, 返回类型)`），再像普通函数一样
//! **触发**。这里覆盖生命周期：注册 → 按位置传参触发 → 未注册符号拒绝 →
//! 一个嵌入会话的失败不影响另一个会话（失败隔离）。

use pilscript::config::Config;
use pilscript::{run_source, run_source_with};

fn demo_lib_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let target = std::path::Path::new(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target");
    let dll = if cfg!(windows) {
        "debug/demo_lib.dll"
    } else if cfg!(target_os = "macos") {
        "debug/libdemo_lib.dylib"
    } else {
        "debug/libdemo_lib.so"
    };
    let path = target.join(dll);
    if !path.exists() {
        panic!("demo_lib 未构建: 请先运行 `cargo build`（期望 {}）", path.display());
    }
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn register_bind_then_invoke_passes_args_in_position_order() {
    let lib = demo_lib_path();
    // mix(a_i64, b_f64, c_i64) = a + b + c。这里特意用混合槽位，验证按位置传参：
    // 顺序传错（float/int 位置颠倒）结果会变，顺序对才能得 13。
    let src = format!(
        r#"
            let lib = dlopen("{lib}");
            let add = lib.fn("add_i64", ["i64", "i64"], "i64");
            // mix(a:i64, b:f64, c:i64) = a + b*c。特意用混合槽位，验证按位置传参：
            // 顺序传错（float/int 位置颠倒）结果会变，顺序对才能算对。
            let mix = lib.fn("mix", ["i64", "f64", "i64"], "f64");

            // 注册后即可触发，参数按声明位置传递
            assert(add(40, 2) == 42);
            assert(mix(10, 0.5, 6) == 13.0);   // 10 + 0.5*6
            assert(mix(1, 2.5, 3) == 8.5);     // 1 + 2.5*3
        "#,
        lib = lib
    );
    run_source(&src).expect("FFI 注册→传参触发失败");
}

#[test]
fn unregistered_symbol_and_disabled_ffi_are_rejected() {
    let lib = demo_lib_path();

    // 未注册（库中不存在）的符号必须被拒
    let src = format!(
        r#"let lib = dlopen("{lib}"); lib.fn("does_not_exist_xyz", [], "void");"#,
        lib = lib
    );
    assert!(run_source(&src).is_err(), "未注册符号应当被拒");

    // 越权：嵌入方在 Config 里关掉 FFI，dlopen 这个钩子入口本身就被拒
    let denied = run_source_with(r#"dlopen("anything.dll")"#, Config {
        allow_ffi: false,
        allow_fs: true,
    });
    match denied {
        Ok(()) => panic!("禁用 FFI 后 dlopen 必须被拒"),
        Err(e) => assert!(e.contains("权限拒绝"), "实际: {e}"),
    }
}

#[test]
fn one_broken_embedding_does_not_crash_the_next() {
    // 失败隔离：一个会话里 FFI/脚本出错，另一个全新会话照样正常运行
    // （解释器 panic 被 catch_unwind 隔离，进程不崩）。
    let bad = run_source("function f() { return f(); } f();"); // 深度溢出
    assert!(bad.is_err(), "坏脚本应当失败");

    let good = run_source("assert(20 + 22 == 42);");
    good.expect("一个坏会话之后，新会话必须照常工作");
}
