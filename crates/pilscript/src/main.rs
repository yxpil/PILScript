use pilscript::Repl;
use std::env;
use std::fs;
use std::io::{BufRead, Write};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BANNER: &str = r#"
  ____  _ _       _____      _ _
 |  _ \(_) |     |  ___|    (_) |
 | |_) | | |     | |__ _   _ _| | ___
 |  __/| | |     |  __| | | | | |/ _ \
 |_|   |_|_|     |_|  | |_| | | |  __/
                      \___/|_|_|\___|
"#;

fn main() {
    let args: Vec<String> = env::args().collect();
    let exit_code = match args.get(1).map(|s| s.as_str()) {
        Some("run") if args.len() >= 3 => run_file(&args[2]),
        Some("repl") => repl(),
        Some("--version") | Some("-V") => {
            println!("pilscript {}", VERSION);
            0
        }
        Some("--help") | Some("-h") | None => {
            print_help();
            0
        }
        Some(other) => {
            eprintln!("未知命令 `{}`\n", other);
            print_help();
            2
        }
    };
    std::process::exit(exit_code);
}

fn print_help() {
    println!("PILScript {} — 极简 JS 风格脚本语言（Rust 实现，支持 C-ABI FFI）", VERSION);
    println!();
    println!("用法:");
    println!("  pilscript run <文件.pil>   运行脚本");
    println!("  pilscript repl             交互式解释器");
    println!("  pilscript --version        版本信息");
}

fn run_file(path: &str) -> i32 {
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("错误: 无法读取文件 {}: {}", path, e);
            return 2;
        }
    };
    match pilscript::run_source(&src) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

fn repl() -> i32 {
    println!("{}PILScript {} — 输入表达式直接求值，:quit 退出", BANNER, VERSION);
    let stdin = std::io::stdin();
    let mut rl = Repl::new();
    loop {
        let prompt = if rl.has_pending() { "  ... " } else { "pil> " };
        print!("{}", prompt);
        std::io::stdout().flush().ok();
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("读取输入失败: {}", e);
                return 1;
            }
        }
        let line = line.trim_end();
        if line == ":quit" || line == ":q" {
            break;
        }
        if line.is_empty() {
            continue;
        }
        match rl.feed(line) {
            Ok(Some(out)) => println!("{}", out),
            Ok(None) => {} // 输入未完成，继续缓冲
            Err(e) => eprintln!("错误: {}", e),
        }
    }
    0
}
