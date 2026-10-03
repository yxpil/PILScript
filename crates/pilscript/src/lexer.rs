//! 词法分析：把源码切成 Token 流。

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    // 字面量
    Num(f64),
    Str(String),
    Ident(String),
    // 关键字
    Let,
    Function,
    Return,
    If,
    Else,
    While,
    For,
    Break,
    Continue,
    True,
    False,
    Null,
    // 标点与运算符
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Dot,
    Colon,
    Assign,      // =
    Eq,          // ==
    Ne,          // !=
    Lt,          // <
    Le,          // <=
    Gt,          // >
    Ge,          // >=
    Plus,        // +
    Minus,       // -
    Star,        // *
    Slash,       // /
    Percent,     // %
    Bang,        // !
    AndAnd,      // &&
    OrOr,        // ||
    PlusAssign,  // +=
    MinusAssign, // -=
    StarAssign,  // *=
    SlashAssign, // /=
    PercentAssign,
    /// 换行（Go 风格自动分号：仅当前一 token 可能结束语句时产生）
    Newline,
    Eof,
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Tok::Num(n) => return write!(f, "数字 {}", n),
            Tok::Str(s) => return write!(f, "字符串 \"{}\"", s),
            Tok::Ident(s) => return write!(f, "标识符 `{}`", s),
            Tok::Let => "let",
            Tok::Function => "function",
            Tok::Return => "return",
            Tok::If => "if",
            Tok::Else => "else",
            Tok::While => "while",
            Tok::For => "for",
            Tok::Break => "break",
            Tok::Continue => "continue",
            Tok::True => "true",
            Tok::False => "false",
            Tok::Null => "null",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::Comma => ",",
            Tok::Semi => ";",
            Tok::Dot => ".",
            Tok::Colon => ":",
            Tok::Assign => "=",
            Tok::Eq => "==",
            Tok::Ne => "!=",
            Tok::Lt => "<",
            Tok::Le => "<=",
            Tok::Gt => ">",
            Tok::Ge => ">=",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Percent => "%",
            Tok::Bang => "!",
            Tok::AndAnd => "&&",
            Tok::OrOr => "||",
            Tok::PlusAssign => "+=",
            Tok::MinusAssign => "-=",
            Tok::StarAssign => "*=",
            Tok::SlashAssign => "/=",
            Tok::PercentAssign => "%=",
            Tok::Newline => "换行",
            Tok::Eof => "文件结尾",
        };
        write!(f, "`{}`", s)
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
}

pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    let mut tokens: Vec<Token> = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut line = 1usize;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '\n' => {
                line += 1;
                // Go 风格自动分号：只有当上一 token 可能结束一个语句时，
                // 换行才成为语句终止符；行尾是运算符/逗号/开括号等则视为续行。
                if let Some(prev) = tokens.last() {
                    if ends_statement(&prev.tok) && prev.tok != Tok::Newline {
                        tokens.push(Token { tok: Tok::Newline, line });
                    }
                }
                i += 1;
            }
            ' ' | '\t' | '\r' => i += 1,
            '/' if i + 1 < chars.len() && chars[i + 1] == '/' => {
                // 行注释
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                // 块注释
                let start_line = line;
                i += 2;
                loop {
                    if i + 1 >= chars.len() {
                        return Err(format!("第 {} 行：块注释没有闭合", start_line));
                    }
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    if chars[i] == '*' && chars[i + 1] == '/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            '"' => {
                // 字符串
                i += 1;
                let mut s = String::new();
                loop {
                    if i >= chars.len() {
                        return Err(format!("第 {} 行：字符串没有闭合", line));
                    }
                    let ch = chars[i];
                    if ch == '"' {
                        i += 1;
                        break;
                    }
                    if ch == '\\' {
                        i += 1;
                        if i >= chars.len() {
                            return Err(format!("第 {} 行：字符串没有闭合", line));
                        }
                        let esc = chars[i];
                        s.push(match esc {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            '\\' => '\\',
                            '"' => '"',
                            '0' => '\0',
                            other => other,
                        });
                        i += 1;
                    } else {
                        if ch == '\n' {
                            line += 1;
                        }
                        s.push(ch);
                        i += 1;
                    }
                }
                tokens.push(Token { tok: Tok::Str(s), line });
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                // 小数部分
                if i + 1 < chars.len() && chars[i] == '.' && chars[i + 1].is_ascii_digit() {
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                let n: f64 = text
                    .parse()
                    .map_err(|_| format!("第 {} 行：无法解析数字 `{}`", line, text))?;
                tokens.push(Token { tok: Tok::Num(n), line });
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let tok = match word.as_str() {
                    "let" => Tok::Let,
                    "function" => Tok::Function,
                    "return" => Tok::Return,
                    "if" => Tok::If,
                    "else" => Tok::Else,
                    "while" => Tok::While,
                    "for" => Tok::For,
                    "break" => Tok::Break,
                    "continue" => Tok::Continue,
                    "true" => Tok::True,
                    "false" => Tok::False,
                    "null" => Tok::Null,
                    _ => Tok::Ident(word),
                };
                tokens.push(Token { tok, line });
            }
            // 运算符与标点
            '(' => push(&mut tokens, Tok::LParen, line, &mut i),
            ')' => push(&mut tokens, Tok::RParen, line, &mut i),
            '{' => push(&mut tokens, Tok::LBrace, line, &mut i),
            '}' => push(&mut tokens, Tok::RBrace, line, &mut i),
            '[' => push(&mut tokens, Tok::LBracket, line, &mut i),
            ']' => push(&mut tokens, Tok::RBracket, line, &mut i),
            ',' => push(&mut tokens, Tok::Comma, line, &mut i),
            ';' => push(&mut tokens, Tok::Semi, line, &mut i),
            '.' => push(&mut tokens, Tok::Dot, line, &mut i),
            ':' => push(&mut tokens, Tok::Colon, line, &mut i),
            '=' => {
                if peek(&chars, i + 1, '=') {
                    push2(&mut tokens, Tok::Eq, line, &mut i, 2);
                } else {
                    push(&mut tokens, Tok::Assign, line, &mut i);
                }
            }
            '!' => {
                if peek(&chars, i + 1, '=') {
                    push2(&mut tokens, Tok::Ne, line, &mut i, 2);
                } else {
                    push(&mut tokens, Tok::Bang, line, &mut i);
                }
            }
            '<' => {
                if peek(&chars, i + 1, '=') {
                    push2(&mut tokens, Tok::Le, line, &mut i, 2);
                } else {
                    push(&mut tokens, Tok::Lt, line, &mut i);
                }
            }
            '>' => {
                if peek(&chars, i + 1, '=') {
                    push2(&mut tokens, Tok::Ge, line, &mut i, 2);
                } else {
                    push(&mut tokens, Tok::Gt, line, &mut i);
                }
            }
            '+' => {
                if peek(&chars, i + 1, '=') {
                    push2(&mut tokens, Tok::PlusAssign, line, &mut i, 2);
                } else {
                    push(&mut tokens, Tok::Plus, line, &mut i);
                }
            }
            '-' => {
                if peek(&chars, i + 1, '=') {
                    push2(&mut tokens, Tok::MinusAssign, line, &mut i, 2);
                } else {
                    push(&mut tokens, Tok::Minus, line, &mut i);
                }
            }
            '*' => {
                if peek(&chars, i + 1, '=') {
                    push2(&mut tokens, Tok::StarAssign, line, &mut i, 2);
                } else {
                    push(&mut tokens, Tok::Star, line, &mut i);
                }
            }
            '/' => {
                if peek(&chars, i + 1, '=') {
                    push2(&mut tokens, Tok::SlashAssign, line, &mut i, 2);
                } else {
                    push(&mut tokens, Tok::Slash, line, &mut i);
                }
            }
            '%' => {
                if peek(&chars, i + 1, '=') {
                    push2(&mut tokens, Tok::PercentAssign, line, &mut i, 2);
                } else {
                    push(&mut tokens, Tok::Percent, line, &mut i);
                }
            }
            '&' => {
                if peek(&chars, i + 1, '&') {
                    push2(&mut tokens, Tok::AndAnd, line, &mut i, 2);
                } else {
                    return Err(format!("第 {} 行：意外的字符 `&`（想要 `&&`）", line));
                }
            }
            '|' => {
                if peek(&chars, i + 1, '|') {
                    push2(&mut tokens, Tok::OrOr, line, &mut i, 2);
                } else {
                    return Err(format!("第 {} 行：意外的字符 `|`（想要 `||`）", line));
                }
            }
            other => {
                return Err(format!("第 {} 行：无法识别的字符 `{}`", line, other));
            }
        }
    }

    tokens.push(Token { tok: Tok::Eof, line });
    Ok(tokens)
}

fn push(tokens: &mut Vec<Token>, tok: Tok, line: usize, i: &mut usize) {
    tokens.push(Token { tok, line });
    *i += 1;
}

fn push2(tokens: &mut Vec<Token>, tok: Tok, line: usize, i: &mut usize, n: usize) {
    tokens.push(Token { tok, line });
    *i += n;
}

fn peek(chars: &[char], idx: usize, expect: char) -> bool {
    idx < chars.len() && chars[idx] == expect
}

/// 这些 token 之后出现换行，意味着当前语句到此结束
fn ends_statement(t: &Tok) -> bool {
    matches!(
        t,
        Tok::Num(_)
            | Tok::Str(_)
            | Tok::Ident(_)
            | Tok::True
            | Tok::False
            | Tok::Null
            | Tok::RParen
            | Tok::RBracket
            | Tok::RBrace
    )
}
