//! 持久化与权限测试。

use pilscript::config::Config;
use pilscript::run_source_with;

#[test]
fn persistence_roundtrip() {
    let dir = std::env::temp_dir().join("pilscript_persist_test");
    std::fs::create_dir_all(&dir).ok();
    let data = dir.join("data.txt");
    let data_s = data.to_string_lossy().replace('\\', "/");

    let src = format!(r#"
        let path = "{p}";
        // 写入
        let n = write_file(path, "hello 持久化\n");
        assert(n > 0);
        // 读回
        assert(read_file(path) == "hello 持久化\n");
        // 追加
        append_file(path, "line2");
        assert(read_file(path) == "hello 持久化\nline2");
        // 存在性检查
        assert(exists(path) == true);
        // 不存在的文件读取 -> null（不抛错，JS 风格）
        assert(read_file(path + ".missing") == null);
    "#, p = data_s);
    run_source_with(&src, Config::default()).expect("持久化往返失败");

    std::fs::remove_file(&data).ok();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn fs_permission_denied() {
    let src = r#"write_file("x.txt", "y")"#;
    match run_source_with(src, Config { allow_ffi: true, allow_fs: false }) {
        Ok(()) => panic!("禁用 fs 后 write_file 应当失败"),
        Err(e) => {
            assert!(e.contains("权限拒绝"), "实际错误: {}", e);
            assert!(e.contains("--no-fs"), "错误应提示如何开启: {}", e);
        }
    }
}

#[test]
fn ffi_permission_denied() {
    let src = r#"dlopen("anything.dll")"#;
    match run_source_with(src, Config { allow_ffi: false, allow_fs: true }) {
        Ok(()) => panic!("禁用 ffi 后 dlopen 应当失败"),
        Err(e) => {
            assert!(e.contains("权限拒绝"), "实际错误: {}", e);
            assert!(e.contains("--no-ffi"), "错误应提示如何开启: {}", e);
        }
    }
}

#[test]
fn time_builtin() {
    let src = r#"
        let t1 = time();
        let t2 = time();
        assert(t1 > 0);
        assert(t2 >= t1);
    "#;
    run_source_with(src, Config::default()).expect("time() 测试失败");
}
