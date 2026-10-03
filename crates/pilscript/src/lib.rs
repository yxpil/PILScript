//! PILScript 库入口：lexer -> parser -> interpreter。

pub mod ast;
pub mod builtins;
pub mod foreign;
pub mod interp;
pub mod lexer;
pub mod parser;
pub mod value;

/// 一站式入口：源码 -> 执行。
/// 在大栈线程上运行，允许较深的脚本递归。
pub fn run_source(src: &str) -> Result<(), String> {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn_scoped(s, move || run_source_inner(src))
            .expect("spawn eval thread")
            .join()
            .expect("eval thread panicked")
    })
}

fn run_source_inner(src: &str) -> Result<(), String> {
    let tokens = lexer::lex(src).map_err(|e| format!("词法错误: {}", e))?;
    let stmts = parser::Parser::new(tokens)
        .parse_program()
        .map_err(|e| e.to_string())?;
    let mut interp = interp::Interpreter::new();
    interp.run_program(&stmts).map_err(|e| format!("运行时错误: {}", e))
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
