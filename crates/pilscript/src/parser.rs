//! 语法分析：递归下降解析，Token 流 -> AST。

use crate::ast::*;
use crate::lexer::{Tok, Token};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

pub type ParseResult<T> = Result<T, String>;

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse_program(mut self) -> ParseResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        while !self.check(&Tok::Eof) {
            if self.eat(&Tok::Newline) {
                continue;
            }
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    // ---- 基础工具 ----

    fn skip_newlines(&mut self) {
        while self.eat(&Tok::Newline) {}
    }

    /// 语句结束：分号、换行（Go 风格自动分号）皆可
    fn expect_stmt_end(&mut self) -> ParseResult<()> {
        if self.check(&Tok::Semi) || self.check(&Tok::Newline) || self.check(&Tok::Eof) {
            self.advance();
            Ok(())
        } else {
            Err(format!(
                "第 {} 行：语法错误，期望语句结束（`;`），但得到 {}",
                self.line(),
                self.peek()
            ))
        }
    }

    fn peek(&self) -> &Tok {
        &self.tokens[self.pos].tok
    }

    fn line(&self) -> usize {
        self.tokens[self.pos].line
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn check(&self, tok: &Tok) -> bool {
        self.peek() == tok
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.check(tok) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, tok: &Tok) -> ParseResult<()> {
        if self.check(tok) {
            self.advance();
            Ok(())
        } else {
            Err(format!(
                "第 {} 行：语法错误，期望 {}，但得到 {}",
                self.line(),
                tok,
                self.peek()
            ))
        }
    }

    fn expect_ident(&mut self) -> ParseResult<String> {
        if let Tok::Ident(name) = self.peek().clone() {
            self.advance();
            Ok(name)
        } else {
            Err(format!(
                "第 {} 行：语法错误，期望标识符，但得到 {}",
                self.line(),
                self.peek()
            ))
        }
    }

    // ---- 语句 ----

    fn parse_stmt(&mut self) -> ParseResult<Stmt> {
        self.skip_newlines();
        match self.peek().clone() {
            Tok::Let => self.parse_let(),
            Tok::Function if matches!(&self.tokens[self.pos + 1].tok, Tok::Ident(_)) => {
                self.parse_fn_decl()
            }
            Tok::Return => {
                self.advance();
                let value = if self.check(&Tok::Semi) || self.check(&Tok::Newline) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect_stmt_end()?;
                Ok(Stmt::Return(value))
            }
            Tok::If => self.parse_if(),
            Tok::While => self.parse_while(),
            Tok::For => self.parse_for(),
            Tok::Break => {
                self.advance();
                self.expect_stmt_end()?;
                Ok(Stmt::Break)
            }
            Tok::Continue => {
                self.advance();
                self.expect_stmt_end()?;
                Ok(Stmt::Continue)
            }
            Tok::LBrace => {
                let body = self.parse_block()?;
                Ok(Stmt::If { cond: Expr::Bool(true), then_body: body, else_body: None })
            }
            _ => {
                let expr = self.parse_expr()?;
                self.expect_stmt_end()?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_let(&mut self) -> ParseResult<Stmt> {
        self.expect(&Tok::Let)?;
        let name = self.expect_ident()?;
        let value = if self.eat(&Tok::Assign) {
            self.parse_expr()?
        } else {
            Expr::Null
        };
        self.expect_stmt_end()?;
        Ok(Stmt::Let { name, value })
    }

    fn parse_fn_decl(&mut self) -> ParseResult<Stmt> {
        self.expect(&Tok::Function)?;
        let name = self.expect_ident()?;
        let (params, body) = self.parse_fn_rest()?;
        Ok(Stmt::FnDecl { name, params, body })
    }

    fn parse_fn_rest(&mut self) -> ParseResult<(Vec<String>, Vec<Stmt>)> {
        self.expect(&Tok::LParen)?;
        let mut params = Vec::new();
        if !self.check(&Tok::RParen) {
            loop {
                params.push(self.expect_ident()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        self.expect(&Tok::RParen)?;
        let body = self.parse_block()?;
        Ok((params, body))
    }

    fn parse_if(&mut self) -> ParseResult<Stmt> {
        self.expect(&Tok::If)?;
        self.expect(&Tok::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Tok::RParen)?;
        let then_body = self.parse_block()?;
        let else_body = if self.eat(&Tok::Else) {
            if self.check(&Tok::If) {
                Some(vec![self.parse_if()?])
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        Ok(Stmt::If { cond, then_body, else_body })
    }

    fn parse_while(&mut self) -> ParseResult<Stmt> {
        self.expect(&Tok::While)?;
        self.expect(&Tok::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Tok::RParen)?;
        let body = self.parse_block()?;
        Ok(Stmt::While { cond, body })
    }

    fn parse_for(&mut self) -> ParseResult<Stmt> {
        self.expect(&Tok::For)?;
        self.expect(&Tok::LParen)?;
        // for (let i = 0; i < n; i += 1)
        let init = if self.eat(&Tok::Semi) {
            None
        } else {
            let s = self.parse_for_head_stmt()?;
            self.expect(&Tok::Semi)?;
            Some(Box::new(s))
        };
        let cond = if self.check(&Tok::Semi) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&Tok::Semi)?;
        let step = if self.check(&Tok::RParen) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&Tok::RParen)?;
        let body = self.parse_block()?;
        Ok(Stmt::For { init, cond, step, body })
    }

    /// for 头部只允许 let 声明或表达式语句（不吃分号，由 parse_for 处理）
    fn parse_for_head_stmt(&mut self) -> ParseResult<Stmt> {
        if self.check(&Tok::Let) {
            self.expect(&Tok::Let)?;
            let name = self.expect_ident()?;
            self.expect(&Tok::Assign)?;
            let value = self.parse_expr()?;
            Ok(Stmt::Let { name, value })
        } else {
            let expr = self.parse_expr()?;
            Ok(Stmt::Expr(expr))
        }
    }

    fn parse_block(&mut self) -> ParseResult<Vec<Stmt>> {
        self.skip_newlines();
        self.expect(&Tok::LBrace)?;
        let mut stmts = Vec::new();
        while !self.check(&Tok::RBrace) {
            if self.check(&Tok::Eof) {
                return Err(format!("第 {} 行：语法错误，缺少 `}}`（到达文件结尾）", self.line()));
            }
            if self.eat(&Tok::Newline) {
                continue;
            }
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&Tok::RBrace)?;
        Ok(stmts)
    }

    // ---- 表达式（优先级从低到高）----

    pub fn parse_expr(&mut self) -> ParseResult<Expr> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> ParseResult<Expr> {
        let left = self.parse_logical_or()?;
        // 赋值目标只允许 Ident / Index / Member
        let is_assign = matches!(
            self.peek(),
            Tok::Assign | Tok::PlusAssign | Tok::MinusAssign | Tok::StarAssign
                | Tok::SlashAssign | Tok::PercentAssign
        );
        if is_assign {
            match &left {
                Expr::Ident(_) | Expr::Index { .. } | Expr::Member { .. } => {}
                _ => return Err(format!("第 {} 行：无效的赋值目标", self.line())),
            }
            let op = self.advance().tok;
            let value = self.parse_assignment()?; // 右结合
            let value = match op {
                Tok::Assign => value,
                Tok::PlusAssign => Expr::Binary { op: BinOp::Add, left: Box::new(left.clone()), right: Box::new(value) },
                Tok::MinusAssign => Expr::Binary { op: BinOp::Sub, left: Box::new(left.clone()), right: Box::new(value) },
                Tok::StarAssign => Expr::Binary { op: BinOp::Mul, left: Box::new(left.clone()), right: Box::new(value) },
                Tok::SlashAssign => Expr::Binary { op: BinOp::Div, left: Box::new(left.clone()), right: Box::new(value) },
                Tok::PercentAssign => Expr::Binary { op: BinOp::Mod, left: Box::new(left.clone()), right: Box::new(value) },
                _ => unreachable!(),
            };
            Ok(Expr::Assign { target: Box::new(left), value: Box::new(value) })
        } else {
            Ok(left)
        }
    }

    fn parse_logical_or(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_logical_and()?;
        while self.eat(&Tok::OrOr) {
            let right = self.parse_logical_and()?;
            left = Expr::Logical { op: LogicalOp::Or, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_equality()?;
        while self.eat(&Tok::AndAnd) {
            let right = self.parse_equality()?;
            left = Expr::Logical { op: LogicalOp::And, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                Tok::Eq => BinOp::Eq,
                Tok::Ne => BinOp::Ne,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.peek() {
                Tok::Lt => BinOp::Lt,
                Tok::Le => BinOp::Le,
                Tok::Gt => BinOp::Gt,
                Tok::Ge => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_term()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_factor()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_factor()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> ParseResult<Expr> {
        if self.eat(&Tok::Minus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary { op: UnaryOp::Neg, expr: Box::new(expr) });
        }
        if self.eat(&Tok::Bang) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary { op: UnaryOp::Not, expr: Box::new(expr) });
        }
        self.parse_postfix()
    }

    /// 后缀：调用、索引、成员访问
    fn parse_postfix(&mut self) -> ParseResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.eat(&Tok::LParen) {
                let mut args = Vec::new();
                self.skip_newlines();
                if !self.check(&Tok::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        self.skip_newlines();
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&Tok::RParen)?;
                expr = Expr::Call { callee: Box::new(expr), args, line: self.line() };
            } else if self.eat(&Tok::LBracket) {
                let index = self.parse_expr()?;
                self.expect(&Tok::RBracket)?;
                expr = Expr::Index { obj: Box::new(expr), index: Box::new(index) };
            } else if self.eat(&Tok::Dot) {
                let name = self.expect_ident()?;
                expr = Expr::Member { obj: Box::new(expr), name };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Expr> {
        match self.peek().clone() {
            Tok::Num(n) => {
                self.advance();
                Ok(Expr::Num(n))
            }
            Tok::Str(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Tok::True => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Tok::False => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            Tok::Null => {
                self.advance();
                Ok(Expr::Null)
            }
            Tok::Ident(name) => {
                self.advance();
                Ok(Expr::Ident(name))
            }
            Tok::LParen => {
                self.advance();
                let e = self.parse_expr()?;
                self.expect(&Tok::RParen)?;
                Ok(e)
            }
            Tok::LBracket => {
                self.advance();
                let mut items = Vec::new();
                self.skip_newlines();
                if !self.check(&Tok::RBracket) {
                    loop {
                        items.push(self.parse_expr()?);
                        self.skip_newlines();
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&Tok::RBracket)?;
                Ok(Expr::Array(items))
            }
            Tok::LBrace => {
                self.advance();
                let mut pairs = Vec::new();
                self.skip_newlines();
                if !self.check(&Tok::RBrace) {
                    loop {
                        // key: 标识符或字符串
                        let key = match self.peek().clone() {
                            Tok::Ident(k) => {
                                self.advance();
                                k
                            }
                            Tok::Str(k) => {
                                self.advance();
                                k
                            }
                            other => {
                                return Err(format!(
                                    "第 {} 行：语法错误，对象键只能是标识符或字符串，但得到 {}",
                                    self.line(),
                                    other
                                ))
                            }
                        };
                        self.expect(&Tok::Colon)?;
                        let value = self.parse_expr()?;
                        pairs.push((key, value));
                        self.skip_newlines();
                        if !self.eat(&Tok::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&Tok::RBrace)?;
                Ok(Expr::Object(pairs))
            }
            Tok::Function => {
                self.advance();
                let (params, body) = self.parse_fn_rest()?;
                Ok(Expr::Fn { params, body })
            }
            other => Err(format!(
                "第 {} 行：语法错误，意外的 {}",
                self.line(),
                other
            )),
        }
    }
}
