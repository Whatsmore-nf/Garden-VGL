// ============================================================
// VGL v2.0 — 语法分析器（递归下降）
// ============================================================

use crate::ast::*;
use crate::error::{VglError, VglResult};
use crate::lexer::Tok;

use std::cell::RefCell;
use std::rc::Rc;

pub struct Parser {
    toks: Vec<(Tok, usize)>,
    idx: usize,
}

impl Parser {
    pub fn new(toks: Vec<(Tok, usize)>) -> Self {
        Parser { toks, idx: 0 }
    }

    fn peek(&self) -> &Tok {
        &self.toks[self.idx.min(self.toks.len() - 1)].0
    }

    fn peek_pos(&self) -> usize {
        self.toks[self.idx.min(self.toks.len() - 1)].1
    }

    fn bump(&mut self) -> (Tok, usize) {
        let t = self.toks[self.idx.min(self.toks.len() - 1)].clone();
        if self.idx < self.toks.len() - 1 {
            self.idx += 1;
        }
        t
    }

    fn eat_punct(&mut self, p: &str) -> bool {
        if matches!(self.peek(), Tok::Punct(x) if *x == p) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_punct(&mut self, p: &str) -> VglResult<usize> {
        let pos = self.peek_pos();
        if self.eat_punct(p) {
            Ok(pos)
        } else {
            Err(VglError::new(format!("期望 '{}'，得到 {}", p, self.describe(self.peek())), pos))
        }
    }

    fn eat_kw(&mut self, k: &str) -> bool {
        if matches!(self.peek(), Tok::Kw(x) if *x == k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_kw(&mut self, k: &str) -> VglResult<usize> {
        let pos = self.peek_pos();
        if self.eat_kw(k) {
            Ok(pos)
        } else {
            Err(VglError::new(format!("期望关键字 '{}'，得到 {}", k, self.describe(self.peek())), pos))
        }
    }

    fn expect_ident(&mut self) -> VglResult<(String, usize)> {
        let pos = self.peek_pos();
        match self.peek().clone() {
            Tok::Ident(name) => {
                self.bump();
                Ok((name, pos))
            }
            other => Err(VglError::new(format!("期望标识符，得到 {}", self.describe(&other), ), pos)),
        }
    }

    fn describe(&self, t: &Tok) -> String {
        match t {
            Tok::Ident(s) => format!("标识符 '{}'", s),
            Tok::Num(v) => format!("数字 {}", v),
            Tok::Str(s) => format!("字符串 \"{}\"", s),
            Tok::Kw(k) => format!("关键字 '{}'", k),
            Tok::Punct(p) => format!("'{}'", p),
            Tok::Eof => "文件结尾".to_string(),
        }
    }

    // ---------- 语句 ----------

    pub fn parse_program(&mut self) -> VglResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        loop {
            match self.peek() {
                Tok::Eof => break,
                Tok::Punct(";") => {
                    self.bump();
                }
                _ => stmts.push(self.parse_stmt()?),
            }
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> VglResult<Stmt> {
        let pos = self.peek_pos();
        match self.peek().clone() {
            Tok::Kw("use") => {
                self.bump();
                let p = self.peek_pos();
                match self.bump().0 {
                    Tok::Str(s) => Ok(Stmt::Use(s, p)),
                    other => Err(VglError::new(
                        format!("use 后应为字符串路径，得到 {}", self.describe(&other)),
                        p,
                    )),
                }
            }
            Tok::Kw("canvas") => {
                self.bump();
                let w = self.expect_num()?;
                let xp = self.peek_pos();
                match self.bump().0 {
                    Tok::Ident(s) if s == "x" => {}
                    other => {
                        return Err(VglError::new(
                            format!("canvas 语法: canvas 800x600（得到 {}）", self.describe(&other)),
                            xp,
                        ))
                    }
                }
                let h = self.expect_num()?;
                Ok(Stmt::Canvas { w, h, pos })
            }
            Tok::Kw("seed") => {
                self.bump();
                let e = self.parse_expr()?;
                Ok(Stmt::Seed(e, pos))
            }
            Tok::Kw("render") => {
                self.bump();
                let e = self.parse_expr()?;
                Ok(Stmt::Render(e, pos))
            }
            Tok::Kw("let") => {
                self.bump();
                let (name, _) = self.expect_ident()?;
                self.expect_punct("=")?;
                let e = self.parse_expr()?;
                Ok(Stmt::Let { name, expr: e, pos })
            }
            Tok::Kw("fn") => Ok(Stmt::FnDef(self.parse_fndef()?)),
            Tok::Kw("if") => self.parse_if(),
            Tok::Kw("for") => self.parse_for(),
            Tok::Kw("while") => {
                self.bump();
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Stmt::While(cond, body, pos))
            }
            Tok::Kw("return") => {
                self.bump();
                // return 后不跟表达式的情况（下一个 token 是 } 或 ; 或语句开头）
                let e = if self.starts_expr() { Some(self.parse_expr()?) } else { None };
                Ok(Stmt::Return(e, pos))
            }
            Tok::Kw("break") => {
                self.bump();
                Ok(Stmt::Break(pos))
            }
            Tok::Kw("continue") => {
                self.bump();
                Ok(Stmt::Continue(pos))
            }
            Tok::Ident(name) if name == "group" && self.group_ahead() => self.parse_group(),
            _ => Ok(Stmt::Expr(self.parse_expr()?)),
        }
    }

    /// 判断当前位置是否为 `group (` 或 `group {`
    fn group_ahead(&self) -> bool {
        matches!(self.toks.get(self.idx + 1).map(|t| &t.0), Some(Tok::Punct("(")) | Some(Tok::Punct("{")))
    }

    fn parse_group(&mut self) -> VglResult<Stmt> {
        let pos = self.peek_pos();
        self.bump(); // group
        let mut named = Vec::new();
        if self.eat_punct("(") {
            loop {
                if self.eat_punct(")") {
                    break;
                }
                let (name, npos) = self.expect_ident()?;
                self.expect_punct(":")?;
                let e = self.parse_expr()?;
                named.push((name, e, npos));
                if !self.eat_punct(",") {
                    self.expect_punct(")")?;
                    break;
                }
            }
        }
        let body = self.parse_block()?;
        Ok(Stmt::Group { named, body, pos })
    }

    fn parse_fndef(&mut self) -> VglResult<Rc<FnDef>> {
        let pos = self.peek_pos();
        self.bump(); // fn
        let (name, _) = self.expect_ident()?;
        self.expect_punct("(")?;
        let mut params = Vec::new();
        loop {
            if self.eat_punct(")") {
                break;
            }
            let (pname, _) = self.expect_ident()?;
            let default = if self.eat_punct("=") { Some(self.parse_expr()?) } else { None };
            params.push((pname, default));
            if !self.eat_punct(",") {
                self.expect_punct(")")?;
                break;
            }
        }
        let body = self.parse_block()?;
        Ok(Rc::new(FnDef {
            name,
            params,
            body,
            env: Rc::new(RefCell::new(Env::new(None))), // 占位，解释器填充
            pos,
        }))
    }

    fn parse_if(&mut self) -> VglResult<Stmt> {
        let pos = self.peek_pos();
        self.bump(); // if
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        let mut branches = vec![(cond, body)];
        let mut else_body = None;
        loop {
            if self.eat_kw("else") {
                if self.eat_kw("if") {
                    let c = self.parse_expr()?;
                    let b = self.parse_block()?;
                    branches.push((c, b));
                } else {
                    else_body = Some(self.parse_block()?);
                    break;
                }
            } else {
                break;
            }
        }
        Ok(Stmt::If { branches, else_body, pos })
    }

    fn parse_for(&mut self) -> VglResult<Stmt> {
        let pos = self.peek_pos();
        self.bump(); // for
        let (var, _) = self.expect_ident()?;
        self.expect_kw("in")?;
        let first = self.parse_expr()?;
        if self.eat_punct("..") {
            let end = self.parse_expr()?;
            // 可选: step 表达式（上下文关键字）
            let step = if matches!(self.peek(), Tok::Ident(s) if s == "step") {
                self.bump();
                Some(self.parse_expr()?)
            } else {
                None
            };
            let body = self.parse_block()?;
            Ok(Stmt::ForRange { var, start: first, end, step, body, pos })
        } else {
            let body = self.parse_block()?;
            Ok(Stmt::ForIn { var, arr: first, body, pos })
        }
    }

    fn parse_block(&mut self) -> VglResult<Vec<Stmt>> {
        self.expect_punct("{")?;
        let mut stmts = Vec::new();
        loop {
            match self.peek() {
                Tok::Punct("}") => {
                    self.bump();
                    break;
                }
                Tok::Eof => return Err(VglError::new("未闭合的代码块 '{'", self.peek_pos())),
                Tok::Punct(";") => {
                    self.bump();
                }
                _ => stmts.push(self.parse_stmt()?),
            }
        }
        Ok(stmts)
    }

    fn expect_num(&mut self) -> VglResult<f64> {
        let pos = self.peek_pos();
        match self.bump().0 {
            Tok::Num(v) => Ok(v),
            other => Err(VglError::new(format!("期望数字，得到 {}", self.describe(&other)), pos)),
        }
    }

    /// 判断当前 token 是否能作为表达式的开头
    fn starts_expr(&self) -> bool {
        !matches!(
            self.peek(),
            Tok::Punct("}") | Tok::Punct(";") | Tok::Eof | Tok::Kw("let") | Tok::Kw("fn")
                | Tok::Kw("if") | Tok::Kw("for") | Tok::Kw("while") | Tok::Kw("return")
                | Tok::Kw("break") | Tok::Kw("continue") | Tok::Kw("use") | Tok::Kw("canvas")
                | Tok::Kw("seed") | Tok::Kw("render") | Tok::Kw("else")
        )
    }

    // ---------- 表达式（优先级爬升） ----------

    pub fn parse_expr(&mut self) -> VglResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> VglResult<Expr> {
        let mut lhs = self.parse_and()?;
        while self.eat_kw2("or") {
            let pos = lhs.pos();
            let rhs = self.parse_and()?;
            lhs = Expr::Binary { op: "or", lhs: Box::new(lhs), rhs: Box::new(rhs), pos };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> VglResult<Expr> {
        let mut lhs = self.parse_cmp()?;
        while self.eat_kw2("and") {
            let pos = lhs.pos();
            let rhs = self.parse_cmp()?;
            lhs = Expr::Binary { op: "and", lhs: Box::new(lhs), rhs: Box::new(rhs), pos };
        }
        Ok(lhs)
    }

    /// and/or 不是保留关键字，作为上下文标识符处理
    fn eat_kw2(&mut self, name: &str) -> bool {
        if let Tok::Ident(s) = self.peek() {
            if s == name {
                self.bump();
                return true;
            }
        }
        false
    }

    fn parse_cmp(&mut self) -> VglResult<Expr> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek() {
                Tok::Punct("==") => "==",
                Tok::Punct("!=") => "!=",
                Tok::Punct("<") => "<",
                Tok::Punct("<=") => "<=",
                Tok::Punct(">") => ">",
                Tok::Punct(">=") => ">=",
                _ => break,
            };
            self.bump();
            let pos = lhs.pos();
            let rhs = self.parse_add()?;
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), pos };
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> VglResult<Expr> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Tok::Punct("+") => "+",
                Tok::Punct("-") => "-",
                _ => break,
            };
            self.bump();
            let pos = lhs.pos();
            let rhs = self.parse_mul()?;
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), pos };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> VglResult<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Tok::Punct("*") => "*",
                Tok::Punct("/") => "/",
                Tok::Punct("%") => "%",
                _ => break,
            };
            self.bump();
            let pos = lhs.pos();
            let rhs = self.parse_unary()?;
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), pos };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> VglResult<Expr> {
        let pos = self.peek_pos();
        match self.peek() {
            Tok::Punct("-") => {
                self.bump();
                let e = self.parse_unary()?;
                Ok(Expr::Unary { op: "-", expr: Box::new(e), pos })
            }
            Tok::Punct("!") => {
                self.bump();
                let e = self.parse_unary()?;
                Ok(Expr::Unary { op: "!", expr: Box::new(e), pos })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> VglResult<Expr> {
        let mut e = self.parse_primary()?;
        while self.eat_punct("[") {
            let pos = e.pos();
            let idx = self.parse_expr()?;
            self.expect_punct("]")?;
            e = Expr::Index { obj: Box::new(e), idx: Box::new(idx), pos };
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> VglResult<Expr> {
        let (tok, pos) = self.bump();
        match tok {
            Tok::Num(v) => Ok(Expr::Num(v, pos)),
            Tok::Str(s) => Ok(Expr::Str(s, pos)),
            Tok::Kw("true") => Ok(Expr::Bool(true, pos)),
            Tok::Kw("false") => Ok(Expr::Bool(false, pos)),
            Tok::Kw("none") => Ok(Expr::NoneLit(pos)),
            Tok::Ident(name) => {
                if self.eat_punct("(") {
                    let (args, named) = self.parse_call_args()?;
                    Ok(Expr::Call { name, args, named, pos })
                } else {
                    Ok(Expr::Ident(name, pos))
                }
            }
            Tok::Punct("(") => {
                let e = self.parse_expr()?;
                self.expect_punct(")")?;
                Ok(e)
            }
            Tok::Punct("[") => {
                let mut items = Vec::new();
                loop {
                    if self.eat_punct("]") {
                        break;
                    }
                    items.push(self.parse_expr()?);
                    if !self.eat_punct(",") {
                        self.expect_punct("]")?;
                        break;
                    }
                }
                Ok(Expr::ArrLit(items, pos))
            }
            other => Err(VglError::new(format!("意外的 {}", self.describe(&other)), pos)),
        }
    }

    /// 解析调用参数: 位置参数与 name: value 命名参数混排
    fn parse_call_args(&mut self) -> VglResult<(Vec<Expr>, Vec<(String, Expr, usize)>)> {
        let mut args = Vec::new();
        let mut named = Vec::new();
        loop {
            if self.eat_punct(")") {
                break;
            }
            // 命名参数: Ident ':' expr
            let is_named = matches!(self.peek(), Tok::Ident(_))
                && matches!(self.toks.get(self.idx + 1).map(|t| &t.0), Some(Tok::Punct(":")));
            if is_named {
                let (name, npos) = self.expect_ident()?;
                self.expect_punct(":")?;
                let e = self.parse_expr()?;
                named.push((name, e, npos));
            } else {
                args.push(self.parse_expr()?);
            }
            if !self.eat_punct(",") {
                self.expect_punct(")")?;
                break;
            }
        }
        Ok((args, named))
    }
}
