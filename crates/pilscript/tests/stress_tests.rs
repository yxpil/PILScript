//! 高并发压测：多线程并发执行脚本，验证解释器实例隔离与稳定性。
//!
//! 设计说明：解释器是单线程同步模型（每个 Interpreter 实例只属于一个线程），
//! 并发能力体现在嵌入场景——可以同时驱动任意多个完全隔离的解释器实例。

use pilscript::run_source;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// 每个线程重复执行的脚本：混合递归、循环、数组、对象操作
const WORKLOAD: &str = r#"
    function fib(n) {
        if (n < 2) { return n }
        return fib(n - 1) + fib(n - 2)
    }

    let acc = { sum: 0, count: 0 };
    for (let round = 0; round < 5; round += 1) {
        acc.sum += fib(15);
        acc.count += 1;
    }

    let arr = [];
    for (let i = 0; i < 200; i += 1) {
        push(arr, i * i);
    }
    assert(arr.length == 200);
    assert(arr[199] == 39601);
    assert(acc.sum == 610 * 5);
    assert(acc.count == 5);
"#;

#[test]
fn concurrent_execution_stress() {
    const THREADS: usize = 16;
    const ITERATIONS: usize = 4; // 16 线程 × 4 次 = 64 个完全隔离的解释器实例

    let failures = Arc::new(AtomicUsize::new(0));
    let completed = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let failures = failures.clone();
            let completed = completed.clone();
            std::thread::spawn(move || {
                for i in 0..ITERATIONS {
                    let mut src = WORKLOAD.to_string();
                    // 每个实例的脚本略有不同，避免共享任何缓存带来的假阳性
                    src.push_str(&format!("\nlet tag = \"t{}-i{}\";\nassert(len(tag) > 0);\n", t, i));
                    if let Err(e) = run_source(&src) {
                        eprintln!("线程 {} 第 {} 轮失败: {}", t, i, e);
                        failures.fetch_add(1, Ordering::SeqCst);
                    }
                    completed.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("压测线程崩溃");
    }

    let elapsed = start.elapsed();
    let done = completed.load(Ordering::SeqCst);
    let failed = failures.load(Ordering::SeqCst);
    println!(
        "压测结果: {} 个并发解释器实例全部执行完成，耗时 {:.2?}，失败 {} 个",
        done, elapsed, failed
    );
    assert_eq!(failed, 0, "并发执行存在失败实例");
    assert_eq!(done, THREADS * ITERATIONS);
}

#[test]
fn error_isolation_between_threads() {
    // 一个线程里的脚本错误不应影响其他线程
    const BAD: &str = "undefined_variable_xxx;";
    const GOOD: &str = "let ok = 40 + 2; assert(ok == 42);";

    let handles: Vec<_> = (0..8)
        .map(|i| {
            if i % 2 == 0 {
                std::thread::spawn(|| {
                    assert!(run_source(BAD).is_err(), "错误脚本应当失败");
                })
            } else {
                std::thread::spawn(|| {
                    run_source(GOOD).expect("正常脚本不应当失败");
                })
            }
        })
        .collect();
    for h in handles {
        h.join().expect("线程崩溃");
    }
}

#[test]
fn depth_limit_isolation() {
    // 无限递归被深度限制拦截，而不是栈溢出崩溃进程
    let result = run_source("function f() { return f(); } f();");
    match result {
        Err(e) => assert!(e.contains("深度"), "应报告调用深度超限: {}", e),
        Ok(()) => panic!("无限递归应当被拦截"),
    }
}
