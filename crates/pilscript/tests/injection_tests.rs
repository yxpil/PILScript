//! 注入测试：证明"数据就是数据"——不可信字符串永远不会被当作代码执行，
//! FFI/C 边界上的恶意字节被干净拒绝。
//!
//! 覆盖：
//! - XSS / SQLi 载荷作为字符串数据读写，原样往返（不解释、不执行）；
//! - 一段长得像代码的字符串不会被求值；
//! - FFI 字符串参数里的 NUL 字节被拒绝（CString 防护）；
//! - 文件函数的路径类型写错（对象/数字）得到干净错误而不是 panic。

use pilscript::config::Config;
use pilscript::{run_source, run_source_with};

/// 定位 demo_lib 动态库（与 ffi_tests.rs 相同）
fn demo_lib_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // crates/pilscript
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
fn xss_and_sqli_payloads_are_opaque_string_data() {
    // 这些载荷看起来像攻击代码，但只是字符串内容：写进文件再读回，必须逐字节一致。
    let dir = std::env::temp_dir().join("pilscript_inject_test");
    std::fs::create_dir_all(&dir).ok();
    let f = dir.join("payload.txt");
    let fp = f.to_string_lossy().replace('\\', "/");

    let payload = concat!(
        "<script>alert('xss')</script>",
        "' OR '1'='1'--",
        "; require('child_process').exec('touch pwned');//"
    );

    let src = format!(
        r#"
            let p = "{fp}";
            write_file(p, "{payload}");
            let back = read_file(p);
            assert(back == "{payload}", "payload must round-trip verbatim, got: " + back);
            // 内容里的 <script>、引号、分号都不能触发任何求值；长度一致证明逐字节往返
            assert(len(back) == {len});
        "#,
        fp = fp,
        payload = payload,
        len = payload.len()
    );
    run_source_with(&src, Config::default()).expect("opaque-data round-trip failed");

    std::fs::remove_file(&f).ok();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn script_inside_a_string_is_not_evaluated() {
    // 字符串里哪怕写着 `assert(false)`、`let x = 1`，也只是文本；
    // 如果解释器把它当代码跑，这个断言会直接挂掉。
    let src = r#"
        let s = "assert(false); let x = 999; function evil() { return 1; }";
        assert(type(s) == "str");
        let n = 0;
        for (let i = 0; i < 3; i += 1) { n += 1; }
        assert(n == 3);
    "#;
    run_source(src).expect("code-in-string must be inert data");
}

#[test]
fn ffi_string_argument_with_nul_byte_is_rejected() {
    // C ABI 字符串遇到 NUL 会被截断 —— 这是经典注入点。解释器必须在转 C 之前拒绝。
    let lib = demo_lib_path();
    let src = format!(
        r#"
            let lib = dlopen("{lib}");
            let shout = lib.fn("shout", ["str"], "str");
            shout("ab\0cd");
        "#,
        lib = lib
    );
    match run_source(&src) {
        Ok(()) => panic!("FFI 字符串里的 NUL 字节必须被拒绝"),
        Err(e) => assert!(
            e.contains("\\0") || e.contains("NUL") || e.contains("0 字节"),
            "错误应指向 NUL 防护，实际: {e}"
        ),
    }
}

#[test]
fn fs_path_type_confusion_is_a_clean_error_not_panic() {
    // 把数字/数组当路径：应得到结构化运行时错误，而不是解释器 panic。
    match run_source("read_file(12345);") {
        Ok(()) => panic!("数字路径应当报错"),
        Err(e) => assert!(e.contains("运行时错误"), "实际: {e}"),
    }
    match run_source("read_file([1, 2]);") {
        Ok(()) => panic!("数组路径应当报错"),
        Err(e) => assert!(e.contains("运行时错误"), "实际: {e}"),
    }
}
