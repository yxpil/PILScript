use pilscript::config::Config;
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

/// CLI 选项
struct Opts {
    command: Option<String>,
    file: Option<String>,
    config: Config,
    json: bool,
}

fn parse_opts() -> Opts {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut opts = Opts { command: None, file: None, config: Config::default(), json: false };
    for arg in args {
        match arg.as_str() {
            "--no-ffi" => opts.config.allow_ffi = false,
            "--no-fs" => opts.config.allow_fs = false,
            "--json" => opts.json = true,
            "--version" | "-V" => opts.command = Some("version".into()),
            "--help" | "-h" => opts.command = Some("help".into()),
            other => {
                if opts.command.is_none() {
                    opts.command = Some(other.into());
                } else if opts.file.is_none() {
                    opts.file = Some(other.into());
                }
            }
        }
    }
    opts
}

fn main() {
    let opts = parse_opts();
    let exit_code = match opts.command.as_deref() {
        Some("run") => match &opts.file {
            Some(path) => run_file(path, &opts),
            None => {
                eprintln!("错误: `run` 需要一个脚本文件参数\n");
                print_help();
                2
            }
        },
        Some("repl") => repl(&opts),
        Some("version") => {
            println!("pilscript {}", VERSION);
            0
        }
        Some("help") | None => {
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
    println!();
    println!("权限开关（生产环境建议按需收紧）:");
    println!("  --no-ffi                   禁用 dlopen / 跨语言调用");
    println!("  --no-fs                    禁用文件读写（read_file/write_file 等）");
    println!();
    println!("其他:");
    println!("  --json                     错误以 JSON 告警格式输出（供日志系统采集）");
}

/// 结构化告警：从错误信息中提取行号，打印源码上下文
fn report_error(err: &str, source: Option<&str>, json: bool) {
    if json {
        // 机器可读告警：供日志/监控系统采集
        let code = err
            .split(']')
            .next()
            .and_then(|s| s.strip_prefix('['))
            .unwrap_or("E9000");
        let line = extract_line(err).unwrap_or(0);
        let msg = strip_code_and_line(err);
        let esc = msg.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
        eprintln!(
            "{{\"level\":\"error\",\"code\":\"{}\",\"line\":{},\"message\":\"{}\"}}",
            code, line, esc
        );
        return;
    }
    eprintln!("{}", err);
    if let (Some(src), Some(line)) = (source, extract_line(err)) {
        let src_line = src.lines().nth(line - 1).unwrap_or("");
        eprintln!("  {:>4} | {}", line, src_line.trim_start());
        eprintln!("       | {}", "^".repeat(src_line.trim().len().max(1)));
    }
}

fn extract_line(err: &str) -> Option<usize> {
    // 匹配 "第 X 行"
    const PAT: &str = "第 ";
    let idx = err.find(PAT)?;
    let rest = &err[idx + PAT.len()..];
    let end = rest.find(' ')?;
    rest[..end].parse().ok()
}

fn strip_code_and_line(err: &str) -> String {
    let msg = match err.find("] ") {
        Some(i) => &err[i + 2..],
        None => err,
    };
    match extract_line(msg) {
        Some(line) => {
            let with_full = format!("第 {} 行：", line);
            let with_half = format!("第 {} 行:", line);
            msg.replace(&with_full, "").replace(&with_half, "")
        }
        None => msg.to_string(),
    }
}

fn run_file(path: &str, opts: &Opts) -> i32 {
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("错误: 无法读取文件 {}: {}", path, e);
            return 2;
        }
    };
    match pilscript::run_source_with(&src, opts.config) {
        Ok(()) => 0,
        Err(e) => {
            report_error(&e, Some(&src), opts.json);
            1
        }
    }
}

fn repl(opts: &Opts) -> i32 {
    println!("{}PILScript {} — 输入表达式直接求值，:quit 退出", BANNER, VERSION);
    if !opts.config.allow_ffi {
        println!("(权限: FFI 已禁用)");
    }
    if !opts.config.allow_fs {
        println!("(权限: 文件系统已禁用)");
    }
    let stdin = std::io::stdin();
    let mut rl = Repl::with_config(opts.config);
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
