//! 语言核心集成测试：通过内建 assert 验证求值行为。

use pilscript::run_source;

fn expect_ok(src: &str) {
    if let Err(e) = run_source(src) {
        panic!("脚本执行失败: {}\n源码:\n{}", e, src);
    }
}

fn expect_err(src: &str, keyword: &str) {
    match run_source(src) {
        Ok(()) => panic!("脚本应当失败，却成功了:\n{}", src),
        Err(e) => assert!(
            e.contains(keyword),
            "错误信息应包含 `{}`，实际: {}\n源码:\n{}",
            keyword,
            e,
            src
        ),
    }
}

#[test]
fn arithmetic_and_precedence() {
    expect_ok("assert(1 + 2 * 3 == 7); assert((1 + 2) * 3 == 9); assert(7 % 3 == 1); assert(10 / 4 == 2.5);");
}

#[test]
fn string_concat() {
    expect_ok(r#"
        let name = "世界";
        assert("你好, " + name == "你好, 世界");
        assert("n=" + 42 == "n=42");
    "#);
}

#[test]
fn scoping_and_closures() {
    expect_ok(r#"
        let x = 1;
        function outer() {
            let x = 2;
            function inner() { return x; }
            return inner();
        }
        assert(outer() == 2);
        assert(x == 1);

        function make_adder(n) {
            return function(m) { return n + m; };
        }
        let add5 = make_adder(5);
        assert(add5(3) == 8);
        assert(add5(10) == 15);
    "#);
}

#[test]
fn recursion() {
    expect_ok(r#"
        function fact(n) {
            if (n <= 1) { return 1; }
            return n * fact(n - 1);
        }
        assert(fact(10) == 3628800);
    "#);
}

#[test]
fn loops_and_control_flow() {
    expect_ok(r#"
        let total = 0;
        for (let i = 1; i <= 10; i += 1) {
            if (i % 2 == 0) { continue; }
            total += i;
        }
        assert(total == 25);

        let n = 0;
        while (true) {
            n += 1;
            if (n >= 5) { break; }
        }
        assert(n == 5);
    "#);
}

#[test]
fn arrays_and_objects() {
    expect_ok(r#"
        let a = [1, 2, 3];
        push(a, 4);
        assert(a.length == 4);
        assert(a[0] == 1 && a[3] == 4);
        assert(a[-1] == 4);
        assert(pop(a) == 4);
        assert(a.length == 3);

        let obj = { name: "Ada", age: 36 };
        obj.lang = "math";
        assert(obj.name == "Ada");
        assert(obj["age"] == 36);
        assert(keys(obj).length == 3);
        assert(len(obj) == 3);
        assert(obj.missing == null);
    "#);
}

#[test]
fn logical_short_circuit() {
    expect_ok(r#"
        let calls = 0;
        function bump() { calls += 1; return true; }
        let r = false && bump();
        assert(calls == 0);
        r = true || bump();
        assert(calls == 0);
        r = true && bump();
        assert(calls == 1);
        assert(!false && !null && !0 && !"");
    "#);
}

#[test]
fn semicolons_optional_like_go() {
    expect_ok(r#"
        function add(a, b) {
            return a + b
        }
        let x = add(1, 2)
        assert(x == 3)
        let list = [
            1,
            2,
            3
        ]
        let obj = {
            a: 1,
            b: 2
        }
        assert(len(list) == 3 && obj.b == 2)
    "#);
}

#[test]
fn assignment_ops() {
    expect_ok(r#"
        let x = 10;
        x += 5; assert(x == 15);
        x -= 3; assert(x == 12);
        x *= 2; assert(x == 24);
        x /= 4; assert(x == 6);
        x %= 4; assert(x == 2);
    "#);
}

#[test]
fn error_cases() {
    expect_err("undefined_var;", "未定义");
    expect_err("let x = 1; x = 2; y = 3;", "未定义");
    expect_err("1 / 0;", "除以零");
    // 越界索引返回 null（JS 风格），不报错
    expect_ok("let a = [1]; assert(a[5] == null); assert(a[-5] == null);");
    // 无限递归应被深度限制拦下
    expect_err("function f() { return f(); } f();", "深度");
}

#[test]
fn higher_order_functions() {
    expect_ok(r#"
        function apply(f, x) { return f(x); }
        assert(apply(function(n) { return n * 2; }, 21) == 42);

        function twice(f) {
            return function(x) { return f(f(x)); };
        }
        let inc = function(n) { return n + 1; };
        assert(twice(inc)(0) == 2);
    "#);
}

#[test]
fn builtins() {
    expect_ok(r#"
        assert(type(1) == "num");
        assert(type("s") == "str");
        assert(type(null) == "null");
        assert(type([]) == "array");
        assert(type({}) == "object");
        assert(num("3.14") == 3.14);
        assert(str(12) == "12");
        assert(len("你好") == 2);
        assert(abs(-3) == 3);
        assert(floor(1.9) == 1);
        assert(ceil(1.1) == 2);
        assert(min(3, 1, 2) == 1);
        assert(max(3, 1, 2) == 3);
        assert(sqrt(16) == 4);
    "#);
}
