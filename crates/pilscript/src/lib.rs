//! PILScript 库入口：lexer -> parser -> interpreter。

pub mod ast;
pub mod builtins;
pub mod config;
pub mod foreign;
pub mod interp;
pub mod lexer;
pub mod parser;
pub mod value;

use config::Config;

/// 一站式入口：源码 -> 执行（默认权限全开）。
/// 在大栈线程上运行，允许较深的脚本递归。
pub fn run_source(src: &str) -> Result<(), String> {
    run_source_with(src, Config::default())
}

/// 带能力配置的运行入口（嵌入方使用）。
///
/// 容错保证：
/// - 脚本逻辑错误 / 语法错误 -> 结构化错误信息（带行号）
/// - 无限递归 -> 调用深度上限拦截
/// - 解释器内部 bug 触发的 panic -> catch_unwind 隔离，返回 [E9001] 而不是崩溃进程
pub fn run_source_with(src: &str, config: Config) -> Result<(), String> {
    std::thread::scope(|s| {
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(s, move || run_inner(src, config))
            .expect("spawn eval thread");
        match handle.join() {
            Ok(result) => result,
            Err(panic_payload) => Err(format!(
                "[E9001] 解释器内部错误（已隔离，进程未崩溃）: {}",
                panic_msg(&panic_payload)
            )),
        }
    })
}

fn run_inner(src: &str, config: Config) -> Result<(), String> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let tokens = lexer::lex(src).map_err(|e| format!("[E1001] 词法错误: {}", e))?;
        let stmts = parser::Parser::new(tokens)
            .parse_program()
            .map_err(|e| format!("[E2001] 语法错误: {}", e))?;
        let mut interp = interp::Interpreter::with_config(config);
        interp
            .run_program(&stmts)
            .map_err(|e| format!("[E3001] 运行时错误: {}", e))
    }));
    match result {
        Ok(r) => r,
        Err(panic_payload) => Err(format!(
            "[E9001] 解释器内部错误（已隔离，进程未崩溃）: {}",
            panic_msg(&panic_payload)
        )),
    }
}

/// 从 panic payload 中提取可读信息
fn panic_msg(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "未知 panic".into()
    }
}

/// 供 REPL 使用的增量求值器
pub struct Repl {
    interp: interp::Interpreter,
    buffer: String,
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}

impl Repl {
    pub fn new() -> Self {
        Repl { interp: interp::Interpreter::new(), buffer: String::new() }
    }

    pub fn with_config(config: Config) -> Self {
        Repl { interp: interp::Interpreter::with_config(config), buffer: String::new() }
    }

    /// 是否有未完成的多行输入正在缓冲
    pub fn has_pending(&self) -> bool {
        !self.buffer.is_empty()
    }

    /// 喂入一行，返回要回显的结果（None 表示输入不完整，需要继续输入）
    pub fn feed(&mut self, line: &str) -> Result<Option<String>, String> {
        self.buffer.push_str(line);
        self.buffer.push('\n');
        let src = self.buffer.clone();
        let tokens = match lexer::lex(&src) {
            Ok(t) => t,
            Err(e) => {
                if incomplete(&e) {
                    return Ok(None);
                }
                self.buffer.clear();
                return Err(e);
            }
        };
        let stmts = match parser::Parser::new(tokens).parse_program() {
            Ok(s) => s,
            Err(e) => {
                if incomplete(&e) {
                    return Ok(None);
                }
                self.buffer.clear();
                return Err(e);
            }
        };
        self.buffer.clear();
        let mut out = String::new();
        let globals = self.interp.globals.clone();
        for stmt in &stmts {
            let result = match stmt {
                // 表达式语句：求值一次并回显（非 null）
                ast::Stmt::Expr(e) => match self.interp.eval_expr(e, &globals) {
                    Ok(v) => {
                        if !matches!(v, value::Value::Null) {
                            out.push_str(&v.display());
                            out.push('\n');
                        }
                        Ok(())
                    }
                    Err(er) => Err(er),
                },
                _ => self.interp.exec_stmt(stmt, &globals).map(|_| ()),
            };
            result?;
        }
        Ok(if out.is_empty() { None } else { Some(out.trim_end().to_string()) })
    }
}

/// 错误信息里提到文件结尾 / 未闭合 -> 输入还不完整，继续缓冲
fn incomplete(err: &str) -> bool {
    err.contains("文件结尾") || err.contains("没有闭合") || err.contains("缺少 `}`")
}
