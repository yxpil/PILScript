//! FFI 集成测试：加载 Rust cdylib 演示库（demo_lib）并跨语言调用。

use pilscript::run_source;

/// 定位 demo_lib 动态库（workspace 的 target 目录）
fn demo_lib_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // crates/pilscript
    let target = std::path::Path::new(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target");
    let (debug, dll) = if cfg!(windows) {
        ("debug", "demo_lib.dll")
    } else if cfg!(target_os = "macos") {
        ("debug", "libdemo_lib.dylib")
    } else {
        ("debug", "libdemo_lib.so")
    };
    let path = target.join(debug).join(dll);
    if !path.exists() {
        panic!("demo_lib 未构建: 请先运行 `cargo build`（期望路径 {}）", path.display());
    }
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn ffi_all_type_combinations() {
    let path = demo_lib_path();
    let src = r#"
        let lib = dlopen("__LIB__");

        // 整数参数 + 整数返回
        let add = lib.fn("add_i64", ["i64", "i64"], "i64");
        assert(add(20, 22) == 42);
        assert(add(-5, 5) == 0);

        // 浮点参数 + 浮点返回
        let mul = lib.fn("mul_f64", ["f64", "f64"], "f64");
        assert(mul(1.5, 4.0) == 6.0);

        // 混合槽位（int, float, int）
        let mix = lib.fn("mix", ["i64", "f64", "i64"], "f64");
        assert(mix(10, 0.5, 6) == 13.0);

        // 单浮点参数
        let root = lib.fn("sqrt_f64", ["f64"], "f64");
        assert(root(144.0) == 12.0);

        // 字符串进、字符串出
        let shout = lib.fn("shout", ["str"], "str");
        assert(shout("hello ffi") == "HELLO FFI");

        // bool 返回（只保证最低字节有效）
        let even = lib.fn("is_even", ["i64"], "bool");
        assert(even(42) == true);
        assert(even(43) == false);
    "#;
    let src = src.replace("__LIB__", &path);
    if let Err(e) = run_source(&src) {
        panic!("FFI 测试失败: {}", e);
    }
}

#[test]
fn ffi_ffi_values_are_first_class() {
    let path = demo_lib_path();
    let src = r#"
        let lib = dlopen("__LIB__");
        let add = lib.fn("add_i64", ["i64", "i64"], "i64");

        // FFI 函数可以作为值传递
        function apply(f, a, b) { return f(a, b); }
        assert(apply(add, 1, 2) == 3);

        // 也能放进数组
        let ops = [add];
        assert(ops[0](7, 8) == 15);

        // 重复绑定同名符号得到独立函数
        let add2 = lib.fn("add_i64", ["i64", "i64"], "i64");
        assert(add2(1, 1) == 2);
    "#;
    let src = src.replace("__LIB__", &path);
    if let Err(e) = run_source(&src) {
        panic!("FFI 一等公民测试失败: {}", e);
    }
}

#[test]
fn ffi_error_reporting() {
    let path = demo_lib_path();
    let missing = format!(
        r#"dlopen("{}"); let lib = dlopen(""); "#,
        path
    );
    match run_source(&missing) {
        Ok(()) => panic!("空路径 dlopen 应当失败"),
        Err(e) => assert!(e.contains("dlopen"), "实际错误: {}", e),
    }

    let bad_symbol = format!(
        r#"let lib = dlopen("{}"); lib.fn("no_such_symbol", [], "void");"#,
        path
    );
    match run_source(&bad_symbol) {
        Ok(()) => panic!("不存在的符号应当失败"),
        Err(e) => assert!(e.contains("找不到符号"), "实际错误: {}", e),
    }

    let bad_arity = format!(
        r#"let lib = dlopen("{}"); let add = lib.fn("add_i64", ["i64", "i64"], "i64"); add(1);"#,
        path
    );
    match run_source(&bad_arity) {
        Ok(()) => panic!("参数个数不符应当失败"),
        Err(e) => assert!(e.contains("2 个参数"), "实际错误: {}", e),
    }

    let bad_type = format!(
        r#"let lib = dlopen("{}"); let add = lib.fn("add_i64", ["i64", "i64"], "i64"); add("a", "b");"#,
        path
    );
    match run_source(&bad_type) {
        Ok(()) => panic!("参数类型不符应当失败"),
        Err(e) => assert!(e.contains("str"), "实际错误: {}", e),
    }
}
