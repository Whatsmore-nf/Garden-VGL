// VGL 解释器 — Rust 版（v0.5 完整实现）
// 对应规范: VGL_语法规范 v0.5.txt
// 用法: vgl <file.vgl>

use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use image::{ImageBuffer, Rgb};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn clamp_u8(v: f64) -> u8 {
    if v < 0.0 {
        0
    } else if v > 255.0 {
        255
    } else {
        v as u8
    }
}

// ============================================================
// 错误类型（v0.5 批次 B：错误定位 §8.2）
// ============================================================

#[derive(Debug)]
struct VglError {
    msg: String,
    pos: Option<usize>, // 字符偏移；None 表示未知位置
}

impl VglError {
    fn new(msg: impl Into<String>, pos: Option<usize>) -> Self {
        VglError {
            msg: msg.into(),
            pos,
        }
    }
}

impl std::fmt::Display for VglError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

type VglResult<T> = Result<T, VglError>;

/// 字符偏移 -> (行, 列)，均 1-based
fn pos_to_linecol(src: &str, pos: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in src.char_indices() {
        if i >= pos {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// 格式化错误信息: filename:line:col: msg\n  <源码行>\n  <caret>
fn format_error(msg: &str, src: &str, pos: Option<usize>, filename: &str) -> String {
    match pos {
        None => format!("{}: {}", filename, msg),
        Some(p) => {
            let (line, col) = pos_to_linecol(src, p);
            let lines: Vec<&str> = src.split('\n').collect();
            let src_line = if line >= 1 && line <= lines.len() {
                lines[line - 1]
            } else {
                ""
            };
            let caret: String = " ".repeat(col.saturating_sub(1)) + "^";
            format!(
                "{}:{}:{}: {}\n  {}\n  {}",
                filename, line, col, msg, src_line, caret
            )
        }
    }
}

// ============================================================
// 词法分析
// ============================================================

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    String(String),
    Color(u8, u8, u8),
    Ident(String),
    Keyword(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    DotDot,
    Op(String),
    Eof,
}

#[derive(Debug, Clone)]
struct TokenWithPos {
    tok: Token,
    pos: usize,
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    fn new(s: &str) -> Self {
        Lexer {
            chars: s.chars().collect(),
            pos: 0,
        }
    }
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        self.pos += 1;
        c
    }
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if !c.is_whitespace() {
                break;
            }
            self.advance();
        }
    }
    fn read_number(&mut self) -> VglResult<f64> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }
        if let Some('.') = self.peek() {
            if self.pos + 1 >= self.chars.len() || self.chars[self.pos + 1] != '.' {
                self.advance();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        s.parse::<f64>()
            .map_err(|_| VglError::new(format!("非法数字 {}", s), Some(start)))
    }
    fn read_ident(&mut self) -> String {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        self.chars[start..self.pos].iter().collect()
    }
    fn read_string(&mut self) -> VglResult<String> {
        let start_pos = self.pos;
        self.advance(); // 跳过开头 "
        let mut result = String::new();
        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance();
                return Ok(result);
            }
            if c == '\\' {
                self.advance();
                if let Some(nxt) = self.peek() {
                    match nxt {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        'r' => result.push('\r'),
                        '\\' => result.push('\\'),
                        '"' => result.push('"'),
                        '0' => result.push('\0'),
                        _ => {
                            result.push('\\');
                            result.push(nxt);
                        }
                    }
                    self.advance();
                }
                continue;
            }
            result.push(c);
            self.advance();
        }
        // EOF 仍未闭合
        Err(VglError::new("未终止的字符串", Some(start_pos)))
    }
    fn read_color(&mut self) -> VglResult<(u8, u8, u8)> {
        let start_pos = self.pos;
        self.advance(); // 跳过 #
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_hexdigit() {
                self.advance();
            } else {
                break;
            }
        }
        let hex: String = self.chars[start..self.pos].iter().collect();
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap();
            Ok((r, g, b))
        } else if hex.len() == 3 {
            let r = u8::from_str_radix(&format!("{}{}", &hex[0..1], &hex[0..1]), 16).unwrap();
            let g = u8::from_str_radix(&format!("{}{}", &hex[1..2], &hex[1..2]), 16).unwrap();
            let b = u8::from_str_radix(&format!("{}{}", &hex[2..3], &hex[2..3]), 16).unwrap();
            Ok((r, g, b))
        } else {
            Err(VglError::new(format!("非法颜色 #{}", hex), Some(start_pos)))
        }
    }
    fn next_token(&mut self) -> VglResult<TokenWithPos> {
        loop {
            self.skip_ws();
            let c = match self.peek() {
                None => return Ok(TokenWithPos { tok: Token::Eof, pos: self.pos }),
                Some(ch) => ch,
            };
            if c == '/' && self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '/' {
                self.pos += 2;
                while let Some(ch) = self.peek() {
                    if ch == '\n' {
                        self.advance();
                        break;
                    }
                    self.advance();
                }
                continue;
            }
            if c == '/' && self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '*' {
                let block_start = self.pos;
                self.pos += 2;
                loop {
                    match self.peek() {
                        None => return Err(VglError::new("未闭合的块注释", Some(block_start))),
                        Some('*') if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '/' => {
                            self.pos += 2;
                            break;
                        }
                        _ => {
                            self.advance();
                        }
                    }
                }
                continue;
            }
            break;
        }
        let tok_pos = self.pos;
        let c = match self.peek() {
            None => return Ok(TokenWithPos { tok: Token::Eof, pos: self.pos }),
            Some(ch) => ch,
        };
        if c == '.' && self.pos + 1 < self.chars.len() {
            if self.chars[self.pos + 1] == '.' {
                self.advance();
                self.advance();
                return Ok(TokenWithPos { tok: Token::DotDot, pos: tok_pos });
            }
            if self.chars[self.pos + 1].is_ascii_digit() {
                return Ok(TokenWithPos { tok: Token::Number(self.read_number()?), pos: tok_pos });
            }
        }
        if c.is_ascii_digit() {
            return Ok(TokenWithPos { tok: Token::Number(self.read_number()?), pos: tok_pos });
        }
        if c == '"' {
            return Ok(TokenWithPos { tok: Token::String(self.read_string()?), pos: tok_pos });
        }
        if c == '#' {
            let (r, g, b) = self.read_color()?;
            return Ok(TokenWithPos { tok: Token::Color(r, g, b), pos: tok_pos });
        }
        if c.is_alphabetic() || c == '_' {
            let ident = self.read_ident();
            let kw = [
                "canvas", "bg", "let", "for", "in", "if", "else", "fn", "return", "pixel",
                "stroke", "render", "while", "break", "and", "or", "not", "seed", "true",
                "false", "continue", "struct", "import", "material", "layer", "field",
            ];
            if kw.contains(&ident.as_str()) {
                return Ok(TokenWithPos { tok: Token::Keyword(ident), pos: tok_pos });
            }
            return Ok(TokenWithPos { tok: Token::Ident(ident), pos: tok_pos });
        }
        let tok = match c {
            '(' => { self.advance(); Token::LParen }
            ')' => { self.advance(); Token::RParen }
            '{' => { self.advance(); Token::LBrace }
            '}' => { self.advance(); Token::RBrace }
            '[' => { self.advance(); Token::LBracket }
            ']' => { self.advance(); Token::RBracket }
            ',' => { self.advance(); Token::Comma }
            ':' => { self.advance(); Token::Colon }
            '.' => { self.advance(); Token::Dot }
            '+' | '-' | '*' | '/' | '=' | '<' | '>' | '!' => {
                self.advance();
                let mut op = c.to_string();
                if let Some(nxt) = self.peek() {
                    if (c == '<' || c == '>' || c == '=' || c == '!') && nxt == '=' {
                        self.advance();
                        op.push('=');
                    }
                }
                Token::Op(op)
            }
            _ => return Err(VglError::new(format!("非法字符 '{}'", c), Some(tok_pos))),
        };
        Ok(TokenWithPos { tok, pos: tok_pos })
    }
    fn tokenize(&mut self) -> VglResult<Vec<TokenWithPos>> {
        let mut toks = Vec::new();
        loop {
            let t = self.next_token()?;
            let is_eof = matches!(t.tok, Token::Eof);
            toks.push(t);
            if is_eof {
                break;
            }
        }
        Ok(toks)
    }
}

// ============================================================
// AST
// ============================================================

#[derive(Clone, Debug)]
enum Expr {
    Number(f64),
    String(String),
    Color(u8, u8, u8),
    Bool(bool),
    Ident(String),
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    BinOp(String, Box<Expr>, Box<Expr>),
    LogicOp(String, Box<Expr>, Box<Expr>),
    UnaryNot(Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    FieldAccess(Box<Expr>, String),
    Call(String, Vec<Expr>, HashMap<String, Expr>),
}

#[derive(Clone, Debug)]
enum Stmt {
    Canvas(u32, u32),
    Bg(Expr),
    Let(String, Expr),
    Assign(String, Expr),
    For(String, Expr, Expr, Vec<StmtWithPos>, Option<String>), // 最后为 label
    While(Expr, Vec<StmtWithPos>, Option<String>),
    If(Expr, Vec<StmtWithPos>, Option<Vec<StmtWithPos>>),
    Break(Option<String>), // v0.4 带标签 break
    Continue,
    Seed(u64),
    FnDef(String, Vec<String>, Vec<StmtWithPos>),
    Return(Expr),
    Pixel(Expr, Expr, Expr),
    Stroke(HashMap<String, Expr>),
    Render(String),
    StructDef(String, Vec<(String, Expr)>),
    Import(String),
    MaterialDef(String, HashMap<String, Expr>),
    LayerDef(String, Vec<StmtWithPos>),
    FieldDef(String, Vec<String>, Vec<StmtWithPos>),
    IndexAssign(Expr, Expr, Expr),
    FieldAssign(Expr, String, Expr),
    ExprStmt(Expr),
}

/// 为语句附加位置信息（运行时错误定位用）
#[derive(Clone, Debug)]
struct StmtWithPos {
    stmt: Stmt,
    pos: usize,
}

// ============================================================
// 语法分析
// ============================================================

struct Parser {
    tokens: Vec<TokenWithPos>,
    pos: usize,
    loop_depth: i32, // 校验 break 必须在循环体内
}

impl Parser {
    fn new(tokens: Vec<TokenWithPos>) -> Self {
        Parser { tokens, pos: 0, loop_depth: 0 }
    }
    fn peek(&self) -> &Token {
        &self.tokens[self.pos].tok
    }
    fn peek_pos(&self) -> usize {
        self.tokens[self.pos].pos
    }
    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].tok.clone();
        self.pos += 1;
        t
    }
    fn expect(&mut self, tok: &Token) -> VglResult<()> {
        if *self.peek() != *tok {
            return Err(VglError::new(
                format!("期望 {:?}, 得到 {:?}", tok, self.peek()),
                Some(self.peek_pos()),
            ));
        }
        self.advance();
        Ok(())
    }
    fn parse_program(&mut self) -> VglResult<Vec<StmtWithPos>> {
        let mut stmts = Vec::new();
        while !matches!(self.peek(), Token::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> VglResult<StmtWithPos> {
        let start_pos = self.peek_pos();

        // 带标签循环: IDENT ':' for/while ...
        if let Token::Ident(label) = self.peek().clone() {
            if self.pos + 2 < self.tokens.len() {
                if matches!(self.tokens[self.pos + 1].tok, Token::Colon)
                    && matches!(&self.tokens[self.pos + 2].tok, Token::Keyword(k) if k == "for" || k == "while")
                {
                    self.advance(); // IDENT
                    self.advance(); // ':'
                    let mut s = self.parse_stmt()?;
                    // 注入 label
                    match &mut s.stmt {
                        Stmt::For(_, _, _, _, l) | Stmt::While(_, _, l) => {
                            *l = Some(label);
                        }
                        _ => {}
                    }
                    s.pos = start_pos;
                    return Ok(s);
                }
            }
        }

        let stmt = self._parse_stmt_impl()?;
        Ok(StmtWithPos { stmt, pos: start_pos })
    }

    fn _parse_stmt_impl(&mut self) -> VglResult<Stmt> {
        match self.peek().clone() {
            Token::Keyword(kw) => match kw.as_str() {
                "canvas" => {
                    self.advance();
                    let w = match self.advance() {
                        Token::Number(n) => n as u32,
                        _ => return Err(VglError::new("canvas 需要宽度", Some(self.peek_pos()))),
                    };
                    let peeked = self.peek().clone();
                    let h = match &peeked {
                        Token::Ident(s) if s.len() > 1 && s.as_bytes()[0] == b'x' => {
                            self.advance();
                            s[1..].parse::<u32>().map_err(|_| {
                                VglError::new(format!("canvas 高度非法: {}", s), Some(self.peek_pos()))
                            })?
                        }
                        Token::Number(n) => {
                            self.advance();
                            *n as u32
                        }
                        _ => return Err(VglError::new("canvas 需要宽x高", Some(self.peek_pos()))),
                    };
                    Ok(Stmt::Canvas(w, h))
                }
                "bg" => {
                    self.advance();
                    Ok(Stmt::Bg(self.parse_expr()?))
                }
                "let" => {
                    self.advance();
                    let name = match self.advance() {
                        Token::Ident(s) => s,
                        _ => return Err(VglError::new("let 需要标识符", Some(self.peek_pos()))),
                    };
                    self.expect(&Token::Op("=".to_string()))?;
                    Ok(Stmt::Let(name, self.parse_expr()?))
                }
                "for" => {
                    self.advance();
                    let var = match self.advance() {
                        Token::Ident(s) => s,
                        _ => return Err(VglError::new("for 需要迭代变量", Some(self.peek_pos()))),
                    };
                    self.expect(&Token::Keyword("in".to_string()))?;
                    let start = self.parse_expr()?;
                    self.expect(&Token::DotDot)?;
                    let end = self.parse_expr()?;
                    self.expect(&Token::LBrace)?;
                    self.loop_depth += 1;
                    let mut body = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) {
                        body.push(self.parse_stmt()?);
                    }
                    self.loop_depth -= 1;
                    self.expect(&Token::RBrace)?;
                    Ok(Stmt::For(var, start, end, body, None))
                }
                "while" => {
                    self.advance();
                    let cond = self.parse_expr()?;
                    self.expect(&Token::LBrace)?;
                    self.loop_depth += 1;
                    let mut body = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) {
                        body.push(self.parse_stmt()?);
                    }
                    self.loop_depth -= 1;
                    self.expect(&Token::RBrace)?;
                    Ok(Stmt::While(cond, body, None))
                }
                "if" => {
                    self.advance();
                    let cond = self.parse_expr()?;
                    self.expect(&Token::LBrace)?;
                    let mut then_body = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) {
                        then_body.push(self.parse_stmt()?);
                    }
                    self.expect(&Token::RBrace)?;
                    let else_body = if matches!(self.peek(), Token::Keyword(ref k) if k == "else") {
                        self.advance();
                        self.expect(&Token::LBrace)?;
                        let mut b = Vec::new();
                        while !matches!(self.peek(), Token::RBrace) {
                            b.push(self.parse_stmt()?);
                        }
                        self.expect(&Token::RBrace)?;
                        Some(b)
                    } else {
                        None
                    };
                    Ok(Stmt::If(cond, then_body, else_body))
                }
                "break" => {
                    self.advance();
                    if self.loop_depth == 0 {
                        return Err(VglError::new(
                            "break 只能出现在 for/while 循环体内",
                            Some(start_pos_of(self)),
                        ));
                    }
                    // 可选 label: break label
                    let label = if let Token::Ident(s) = self.peek() {
                        let s = s.clone();
                        self.advance();
                        Some(s)
                    } else {
                        None
                    };
                    Ok(Stmt::Break(label))
                }
                "continue" => {
                    self.advance();
                    if self.loop_depth == 0 {
                        return Err(VglError::new(
                            "continue 只能出现在 for/while 循环体内",
                            Some(start_pos_of(self)),
                        ));
                    }
                    Ok(Stmt::Continue)
                }
                "seed" => {
                    self.advance();
                    let n = match self.advance() {
                        Token::Number(v) => v as u64,
                        _ => return Err(VglError::new("seed 需要整数", Some(self.peek_pos()))),
                    };
                    Ok(Stmt::Seed(n))
                }
                "fn" => {
                    self.advance();
                    let name = match self.advance() {
                        Token::Ident(s) => s,
                        _ => return Err(VglError::new("fn 需要名称", Some(self.peek_pos()))),
                    };
                    self.expect(&Token::LParen)?;
                    let params = self.parse_param_list()?;
                    self.expect(&Token::RParen)?;
                    self.expect(&Token::LBrace)?;
                    let body = self.parse_block_body()?;
                    self.expect(&Token::RBrace)?;
                    Ok(Stmt::FnDef(name, params, body))
                }
                "return" => {
                    self.advance();
                    Ok(Stmt::Return(self.parse_expr()?))
                }
                "pixel" => {
                    self.advance();
                    self.expect(&Token::LParen)?;
                    let map = self.parse_kwargs_block()?;
                    self.expect(&Token::RParen)?;
                    let x = map.get("x").cloned().unwrap_or(Expr::Number(0.0));
                    let y = map.get("y").cloned().unwrap_or(Expr::Number(0.0));
                    let rgb = map.get("rgb").cloned().unwrap_or(Expr::Color(0, 0, 0));
                    Ok(Stmt::Pixel(x, y, rgb))
                }
                "stroke" => {
                    self.advance();
                    self.expect(&Token::LBrace)?;
                    let fields = self.parse_kwargs_block()?;
                    self.expect(&Token::RBrace)?;
                    Ok(Stmt::Stroke(fields))
                }
                "render" => {
                    self.advance();
                    let fname = match self.advance() {
                        Token::String(s) => s,
                        _ => return Err(VglError::new("render 需要字符串", Some(self.peek_pos()))),
                    };
                    Ok(Stmt::Render(fname))
                }
                "struct" => {
                    self.advance();
                    let name = match self.advance() {
                        Token::Ident(s) => s,
                        _ => return Err(VglError::new("struct 需要名称", Some(self.peek_pos()))),
                    };
                    self.expect(&Token::LBrace)?;
                    let mut fields = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) {
                        let fname = match self.advance() {
                            Token::Ident(s) => s,
                            _ => return Err(VglError::new("struct 字段需要标识符", Some(self.peek_pos()))),
                        };
                        self.expect(&Token::Colon)?;
                        let default = self.parse_expr()?;
                        fields.push((fname, default));
                        if matches!(self.peek(), Token::Comma) {
                            self.advance();
                        }
                    }
                    self.expect(&Token::RBrace)?;
                    Ok(Stmt::StructDef(name, fields))
                }
                "import" => {
                    self.advance();
                    let path = match self.advance() {
                        Token::String(s) => s,
                        _ => return Err(VglError::new("import 需要字符串路径", Some(self.peek_pos()))),
                    };
                    Ok(Stmt::Import(path))
                }
                "material" => {
                    self.advance();
                    let name = match self.advance() {
                        Token::Ident(s) => s,
                        _ => return Err(VglError::new("material 需要名称", Some(self.peek_pos()))),
                    };
                    self.expect(&Token::LBrace)?;
                    let fields = self.parse_kwargs_block()?;
                    self.expect(&Token::RBrace)?;
                    Ok(Stmt::MaterialDef(name, fields))
                }
                "layer" => {
                    self.advance();
                    let name = match self.advance() {
                        Token::Ident(s) => s,
                        _ => return Err(VglError::new("layer 需要名称", Some(self.peek_pos()))),
                    };
                    self.expect(&Token::LBrace)?;
                    let body = self.parse_block_body()?;
                    self.expect(&Token::RBrace)?;
                    Ok(Stmt::LayerDef(name, body))
                }
                "field" => {
                    self.advance();
                    let name = match self.advance() {
                        Token::Ident(s) => s,
                        _ => return Err(VglError::new("field 需要名称", Some(self.peek_pos()))),
                    };
                    self.expect(&Token::LParen)?;
                    let params = self.parse_param_list()?;
                    self.expect(&Token::RParen)?;
                    self.expect(&Token::LBrace)?;
                    let body = self.parse_block_body()?;
                    self.expect(&Token::RBrace)?;
                    Ok(Stmt::FieldDef(name, params, body))
                }
                _ => self.parse_ident_stmt(),
            },
            _ => self.parse_ident_stmt(),
        }
    }

    fn parse_param_list(&mut self) -> VglResult<Vec<String>> {
        let mut params = Vec::new();
        if !matches!(self.peek(), Token::RParen) {
            match self.advance() {
                Token::Ident(s) => params.push(s),
                _ => return Err(VglError::new("参数需要标识符", Some(self.peek_pos()))),
            }
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                match self.advance() {
                    Token::Ident(s) => params.push(s),
                    _ => return Err(VglError::new("参数需要标识符", Some(self.peek_pos()))),
                }
            }
        }
        Ok(params)
    }

    fn parse_block_body(&mut self) -> VglResult<Vec<StmtWithPos>> {
        let mut body = Vec::new();
        while !matches!(self.peek(), Token::RBrace) {
            body.push(self.parse_stmt()?);
        }
        Ok(body)
    }

    /// 解析 `key: val, key: val, ...` 直到遇到 RParen 或 RBrace
    /// v0.5 批次 C：允许 KEYWORD 作为 key（如 stroke { material: ... }）
    fn parse_kwargs_block(&mut self) -> VglResult<HashMap<String, Expr>> {
        let mut map = HashMap::new();
        while !matches!(self.peek(), Token::RBrace) && !matches!(self.peek(), Token::RParen) {
            let key = match self.peek().clone() {
                Token::Ident(s) => {
                    self.advance();
                    s
                }
                Token::Keyword(s) => {
                    self.advance();
                    s
                }
                _ => return Err(VglError::new("字段名需要标识符", Some(self.peek_pos()))),
            };
            self.expect(&Token::Colon)?;
            let val = self.parse_expr()?;
            map.insert(key, val);
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            }
        }
        Ok(map)
    }

    fn parse_ident_stmt(&mut self) -> VglResult<Stmt> {
        let name = match self.peek().clone() {
            Token::Ident(s) => {
                self.advance();
                s
            }
            _ => return Err(VglError::new("意外的标记", Some(self.peek_pos()))),
        };
        // 赋值
        if let Token::Op(ref op) = self.peek() {
            if op == "=" {
                self.advance();
                return Ok(Stmt::Assign(name, self.parse_expr()?));
            }
        }
        // 索引赋值
        if matches!(self.peek(), Token::LBracket) {
            self.advance();
            let idx = self.parse_expr()?;
            self.expect(&Token::RBracket)?;
            self.expect(&Token::Op("=".to_string()))?;
            let expr = self.parse_expr()?;
            return Ok(Stmt::IndexAssign(Expr::Ident(name), idx, expr));
        }
        // 字段赋值
        if matches!(self.peek(), Token::Dot) {
            self.advance();
            let field = match self.advance() {
                Token::Ident(s) => s,
                _ => return Err(VglError::new("字段名需要标识符", Some(self.peek_pos()))),
            };
            self.expect(&Token::Op("=".to_string()))?;
            let expr = self.parse_expr()?;
            return Ok(Stmt::FieldAssign(Expr::Ident(name), field, expr));
        }
        // 表达式语句（函数调用链）
        let mut expr = Expr::Ident(name.clone());
        expr = self.parse_postfix(expr)?;
        Ok(Stmt::ExprStmt(expr))
    }

    fn parse_postfix(&mut self, mut expr: Expr) -> VglResult<Expr> {
        loop {
            match self.peek().clone() {
                Token::LParen => {
                    self.advance();
                    let (args, kwargs) = self.parse_call_args()?;
                    self.expect(&Token::RParen)?;
                    let name = match expr {
                        Expr::Ident(n) => n,
                        _ => return Err(VglError::new("只有标识符可调用", Some(self.peek_pos()))),
                    };
                    expr = Expr::Call(name, args, kwargs);
                }
                Token::LBracket => {
                    self.advance();
                    let idx = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    expr = Expr::Index(Box::new(expr), Box::new(idx));
                }
                Token::Dot => {
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(s) => s,
                        _ => return Err(VglError::new("字段名需要标识符", Some(self.peek_pos()))),
                    };
                    expr = Expr::FieldAccess(Box::new(expr), field);
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_call_args(&mut self) -> VglResult<(Vec<Expr>, HashMap<String, Expr>)> {
        let mut args = Vec::new();
        let mut kwargs = HashMap::new();
        if matches!(self.peek(), Token::RParen) {
            return Ok((args, kwargs));
        }
        // 检测是否为 kwarg 形式: IDENT ':' 或 KEYWORD ':'
        let is_kwarg = matches!(self.peek(), Token::Ident(_) | Token::Keyword(_))
            && self.pos + 1 < self.tokens.len()
            && matches!(self.tokens[self.pos + 1].tok, Token::Colon);
        if is_kwarg {
            while !matches!(self.peek(), Token::RParen) {
                let key = match self.peek().clone() {
                    Token::Ident(s) | Token::Keyword(s) => {
                        self.advance();
                        s
                    }
                    _ => return Err(VglError::new("参数名需要标识符", Some(self.peek_pos()))),
                };
                self.expect(&Token::Colon)?;
                let val = self.parse_expr()?;
                kwargs.insert(key, val);
                if matches!(self.peek(), Token::Comma) {
                    self.advance();
                }
            }
        } else {
            args.push(self.parse_expr()?);
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                args.push(self.parse_expr()?);
            }
        }
        Ok((args, kwargs))
    }

    fn parse_expr(&mut self) -> VglResult<Expr> {
        self.parse_or()
    }
    fn parse_or(&mut self) -> VglResult<Expr> {
        let mut left = self.parse_and()?;
        while let Token::Keyword(ref kw) = self.peek() {
            if kw == "or" {
                self.advance();
                let right = self.parse_and()?;
                left = Expr::LogicOp("or".into(), Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }
    fn parse_and(&mut self) -> VglResult<Expr> {
        let mut left = self.parse_compare()?;
        while let Token::Keyword(ref kw) = self.peek() {
            if kw == "and" {
                self.advance();
                let right = self.parse_compare()?;
                left = Expr::LogicOp("and".into(), Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }
    fn parse_compare(&mut self) -> VglResult<Expr> {
        let left = self.parse_add()?;
        if let Token::Op(ref op) = self.peek() {
            if ["<", ">", "<=", ">=", "==", "!="].contains(&op.as_str()) {
                let op = self.advance();
                let right = self.parse_add()?;
                if let Token::Op(opstr) = op {
                    return Ok(Expr::BinOp(opstr, Box::new(left), Box::new(right)));
                }
            }
        }
        Ok(left)
    }
    fn parse_add(&mut self) -> VglResult<Expr> {
        let mut left = self.parse_mul()?;
        while let Token::Op(ref op) = self.peek() {
            if op == "+" || op == "-" {
                let op = self.advance();
                let right = self.parse_mul()?;
                if let Token::Op(opstr) = op {
                    left = Expr::BinOp(opstr, Box::new(left), Box::new(right));
                }
            } else {
                break;
            }
        }
        Ok(left)
    }
    fn parse_mul(&mut self) -> VglResult<Expr> {
        let mut left = self.parse_unary()?;
        while let Token::Op(ref op) = self.peek() {
            if op == "*" || op == "/" {
                let op = self.advance();
                let right = self.parse_unary()?;
                if let Token::Op(opstr) = op {
                    left = Expr::BinOp(opstr, Box::new(left), Box::new(right));
                }
            } else {
                break;
            }
        }
        Ok(left)
    }
    fn parse_unary(&mut self) -> VglResult<Expr> {
        if let Token::Op(ref op) = self.peek() {
            if op == "-" {
                self.advance();
                return Ok(Expr::BinOp(
                    "-".into(),
                    Box::new(Expr::Number(0.0)),
                    Box::new(self.parse_unary()?),
                ));
            }
        }
        if let Token::Keyword(ref kw) = self.peek() {
            if kw == "not" {
                self.advance();
                return Ok(Expr::UnaryNot(Box::new(self.parse_unary()?)));
            }
        }
        self.parse_primary()
    }
    fn parse_primary(&mut self) -> VglResult<Expr> {
        let tok = self.advance();
        let expr = match tok {
            Token::Number(n) => Expr::Number(n),
            Token::String(s) => Expr::String(s),
            Token::Color(r, g, b) => Expr::Color(r, g, b),
            Token::Keyword(ref kw) if kw == "true" => Expr::Bool(true),
            Token::Keyword(ref kw) if kw == "false" => Expr::Bool(false),
            Token::Ident(s) => Expr::Ident(s),
            Token::LParen => {
                let inner = self.parse_expr()?;
                if matches!(self.peek(), Token::Comma) {
                    self.advance();
                    let mut elems = vec![inner];
                    while !matches!(self.peek(), Token::RParen) {
                        elems.push(self.parse_expr()?);
                        if matches!(self.peek(), Token::Comma) {
                            self.advance();
                        }
                    }
                    self.expect(&Token::RParen)?;
                    Expr::Tuple(elems)
                } else {
                    self.expect(&Token::RParen)?;
                    inner
                }
            }
            Token::LBracket => {
                let mut elems = Vec::new();
                while !matches!(self.peek(), Token::RBracket) {
                    elems.push(self.parse_expr()?);
                    if matches!(self.peek(), Token::Comma) {
                        self.advance();
                    }
                }
                self.expect(&Token::RBracket)?;
                Expr::Array(elems)
            }
            _ => return Err(VglError::new(format!("意外的标记: {:?}", tok), Some(self.peek_pos()))),
        };
        self.parse_postfix(expr)
    }
}

fn start_pos_of(p: &Parser) -> usize {
    p.peek_pos()
}

// ============================================================
// 运行时环境
// ============================================================

#[derive(Clone, Debug)]
enum Value {
    Number(f64),
    Bool(bool),
    String(String),
    Color(u8, u8, u8),
    Tuple(Vec<Value>),
    Array(Rc<RefCell<Vec<Value>>>),
    Dict(Rc<RefCell<HashMap<String, Value>>>),
    Struct(Rc<RefCell<HashMap<String, Value>>>),
    Path(String, Vec<Value>),
    Closure(String, Vec<String>, Vec<StmtWithPos>, Rc<RefCell<Env>>),
    Material(HashMap<String, Value>),
    Layer(Rc<RefCell<Canvas>>),
    None,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a.to_bits() == b.to_bits(),
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Color(r1, g1, b1), Value::Color(r2, g2, b2)) => r1 == r2 && g1 == g2 && b1 == b2,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::None, Value::None) => true,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::Number(n) => n.to_bits().hash(state),
            Value::Bool(b) => b.hash(state),
            Value::String(s) => s.hash(state),
            Value::Color(r, g, b) => {
                r.hash(state);
                g.hash(state);
                b.hash(state);
            }
            Value::Tuple(t) => t.hash(state),
            _ => 0.hash(state),
        }
    }
}

impl Value {
    fn as_number(&self) -> Option<f64> {
        if let Value::Number(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    fn as_tuple(&self) -> Option<Vec<Value>> {
        if let Value::Tuple(t) = self {
            Some(t.clone())
        } else {
            None
        }
    }
    fn as_string(&self) -> Option<String> {
        if let Value::String(s) = self {
            Some(s.clone())
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
struct Env {
    vars: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    fn new(parent: Option<Rc<RefCell<Env>>>) -> Self {
        Env {
            vars: HashMap::new(),
            parent,
        }
    }
    fn get(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.vars.get(name) {
            return Some(v.clone());
        }
        if let Some(ref p) = self.parent {
            return p.borrow().get(name);
        }
        None
    }
    fn contains(&self, name: &str) -> bool {
        if self.vars.contains_key(name) {
            true
        } else if let Some(ref p) = self.parent {
            p.borrow().contains(name)
        } else {
            false
        }
    }
    fn set(&mut self, name: &str, val: Value) -> Result<(), String> {
        if self.vars.contains_key(name) {
            self.vars.insert(name.to_string(), val);
            Ok(())
        } else if let Some(ref p) = self.parent {
            if p.borrow().contains(name) {
                p.borrow_mut().vars.insert(name.to_string(), val);
                Ok(())
            } else {
                Err(format!("变量 {} 未定义", name))
            }
        } else {
            Err(format!("变量 {} 未定义", name))
        }
    }
}

// ============================================================
// 绘图引擎
// ============================================================

#[derive(Clone, Debug)]
struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    bg: (u8, u8, u8),
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Canvas {
            width: w,
            height: h,
            pixels: vec![0; (w * h * 3) as usize],
            bg: (0, 0, 0),
        }
    }
    fn put_pixel(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8) {
        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
            let idx = (y as u32 * self.width + x as u32) as usize * 3;
            self.pixels[idx] = r.min(255);
            self.pixels[idx + 1] = g.min(255);
            self.pixels[idx + 2] = b.min(255);
        }
    }
    fn put_pixel_aa(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8, alpha: f64) {
        if alpha <= 0.0 || x < 0 || x >= self.width as i32 || y < 0 || y >= self.height as i32 {
            return;
        }
        let idx = (y as u32 * self.width + x as u32) as usize * 3;
        let a = alpha.min(1.0).max(0.0);
        self.pixels[idx] = clamp_u8(self.pixels[idx] as f64 * (1.0 - a) + r as f64 * a);
        self.pixels[idx + 1] = clamp_u8(self.pixels[idx + 1] as f64 * (1.0 - a) + g as f64 * a);
        self.pixels[idx + 2] = clamp_u8(self.pixels[idx + 2] as f64 * (1.0 - a) + b as f64 * a);
    }
    fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, width: f64, r: u8, g: u8, b: u8) {
        if width <= 1.0 {
            self.wu_line(x0, y0, x1, y1, r, g, b);
        } else {
            for (x, y) in self.bresenham_points(x0, y0, x1, y1) {
                self.brush(x, y, width as i32, r, g, b);
            }
        }
    }
    fn bresenham_points(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
        let mut pts = Vec::new();
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut x = x0;
        let mut y = y0;
        loop {
            pts.push((x, y));
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
        pts
    }
    fn brush(&mut self, cx: i32, cy: i32, radius: i32, r: u8, g: u8, b: u8) {
        let rad = radius as f64 / 2.0;
        let r2 = rad * rad;
        let ri = (rad + 1.0) as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f64 <= r2 {
                    self.put_pixel(cx + dx, cy + dy, r, g, b);
                }
            }
        }
    }
    fn wu_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, r: u8, g: u8, b: u8) {
        fn ipart(x: f64) -> i32 { x.floor() as i32 }
        fn fpart(x: f64) -> f64 { x - x.floor() }
        fn rfpart(x: f64) -> f64 { 1.0 - fpart(x) }
        let (mut x0, mut y0, mut x1, mut y1) = (x0, y0, x1, y1);
        let steep = (y1 - y0).abs() > (x1 - x0).abs();
        if steep {
            std::mem::swap(&mut x0, &mut y0);
            std::mem::swap(&mut x1, &mut y1);
        }
        if x0 > x1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }
        let dx = x1 - x0;
        let dy = y1 - y0;
        let grad = if dx != 0 { dy as f64 / dx as f64 } else { 1.0 };
        let xend = (x0 as f64).round() as i32;
        let yend = y0 as f64 + grad * (xend - x0) as f64;
        let xgap = rfpart(x0 as f64 + 0.5);
        let xpxl1 = xend;
        let ypxl1 = ipart(yend);
        if steep {
            self.put_pixel_aa(ypxl1, xpxl1, r, g, b, rfpart(yend) * xgap);
            self.put_pixel_aa(ypxl1 + 1, xpxl1, r, g, b, fpart(yend) * xgap);
        } else {
            self.put_pixel_aa(xpxl1, ypxl1, r, g, b, rfpart(yend) * xgap);
            self.put_pixel_aa(xpxl1, ypxl1 + 1, r, g, b, fpart(yend) * xgap);
        }
        let mut intery = yend + grad;
        let xend2 = (x1 as f64).round() as i32;
        let yend2 = y1 as f64 + grad * (xend2 - x1) as f64;
        let xgap2 = fpart(x1 as f64 + 0.5);
        let xpxl2 = xend2;
        let ypxl2 = ipart(yend2);
        if steep {
            self.put_pixel_aa(ypxl2, xpxl2, r, g, b, rfpart(yend2) * xgap2);
            self.put_pixel_aa(ypxl2 + 1, xpxl2, r, g, b, fpart(yend2) * xgap2);
        } else {
            self.put_pixel_aa(xpxl2, ypxl2, r, g, b, rfpart(yend2) * xgap2);
            self.put_pixel_aa(xpxl2, ypxl2 + 1, r, g, b, fpart(yend2) * xgap2);
        }
        for x in (xpxl1 + 1)..xpxl2 {
            if steep {
                self.put_pixel_aa(ipart(intery), x, r, g, b, rfpart(intery));
                self.put_pixel_aa(ipart(intery) + 1, x, r, g, b, fpart(intery));
            } else {
                self.put_pixel_aa(x, ipart(intery), r, g, b, rfpart(intery));
                self.put_pixel_aa(x, ipart(intery) + 1, r, g, b, fpart(intery));
            }
            intery += grad;
        }
    }
    fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32, width: f64, r: u8, g: u8, b: u8) {
        let mut x = radius;
        let mut y = 0;
        let mut err = 0;
        while x >= y {
            for (px, py) in &[
                (cx + x, cy + y),
                (cx + y, cy + x),
                (cx - y, cy + x),
                (cx - x, cy + y),
                (cx - x, cy - y),
                (cx - y, cy - x),
                (cx + y, cy - x),
                (cx + x, cy - y),
            ] {
                if width <= 1.0 {
                    self.put_pixel(*px, *py, r, g, b);
                } else {
                    self.brush(*px, *py, width as i32, r, g, b);
                }
            }
            y += 1;
            if err <= 0 {
                err += 2 * y + 1;
            }
            if err > 0 {
                x -= 1;
                err -= 2 * x + 1;
            }
        }
    }
    fn sample_bezier3(
        &self,
        p1: (f64, f64),
        p2: (f64, f64),
        p3: (f64, f64),
        p4: (f64, f64),
        n: usize,
    ) -> Vec<(i32, i32)> {
        let mut pts = Vec::new();
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let q0 = (p1.0 + (p2.0 - p1.0) * t, p1.1 + (p2.1 - p1.1) * t);
            let q1 = (p2.0 + (p3.0 - p2.0) * t, p2.1 + (p3.1 - p2.1) * t);
            let q2 = (p3.0 + (p4.0 - p3.0) * t, p3.1 + (p4.1 - p3.1) * t);
            let r0 = (q0.0 + (q1.0 - q0.0) * t, q0.1 + (q1.1 - q0.1) * t);
            let r1 = (q1.0 + (q2.0 - q1.0) * t, q1.1 + (q2.1 - q1.1) * t);
            let point = (r0.0 + (r1.0 - r0.0) * t, r0.1 + (r1.1 - r0.1) * t);
            pts.push((point.0.round() as i32, point.1.round() as i32));
        }
        pts
    }
    /// 二次贝塞尔采样（通过提升为三次）
    fn sample_bezier2(
        &self,
        p1: (f64, f64),
        p2: (f64, f64),
        p3: (f64, f64),
        n: usize,
    ) -> Vec<(i32, i32)> {
        // 二次 -> 三次：c1 = p1 + 2/3*(p2-p1), c2 = p3 + 2/3*(p2-p3)
        let c1 = (p1.0 + (p2.0 - p1.0) * 2.0 / 3.0, p1.1 + (p2.1 - p1.1) * 2.0 / 3.0);
        let c2 = (p3.0 + (p2.0 - p3.0) * 2.0 / 3.0, p3.1 + (p2.1 - p3.1) * 2.0 / 3.0);
        self.sample_bezier3(p1, c1, c2, p3, n)
    }
    fn fill(&mut self, r: u8, g: u8, b: u8) {
        for i in (0..self.pixels.len()).step_by(3) {
            self.pixels[i] = r;
            self.pixels[i + 1] = g;
            self.pixels[i + 2] = b;
        }
    }
}

// ============================================================
// 噪声实现
// ============================================================

const PERM: [usize; 512] = {
    let mut p = [0; 512];
    let mut i = 0;
    while i < 256 {
        p[i] = i;
        p[i + 256] = i;
        i += 1;
    }
    let shuffle = [151,160,137,91,90,15,131,13,201,95,96,53,194,233,7,225,
                   140,36,103,30,69,142,8,99,37,240,21,10,23,190,6,148,
                   247,120,234,75,0,26,197,62,94,252,219,203,117,35,11,32,
                   57,177,33,88,237,149,56,87,174,20,125,136,171,168,68,175,
                   74,165,71,134,139,48,27,166,77,146,158,231,83,111,229,122,
                   60,211,133,230,220,105,92,41,55,46,245,40,244,102,143,54,
                   65,25,63,161,1,216,80,73,209,76,132,187,208,89,18,169,
                   200,196,135,130,116,188,159,86,164,100,109,198,173,186,3,64,
                   52,217,226,250,124,123,5,202,38,147,118,126,255,82,85,212,
                   207,206,59,227,47,16,58,17,182,189,28,42,223,183,170,213,
                   119,248,152,2,44,154,163,70,221,153,101,155,167,43,172,9,
                   129,22,39,253,19,98,108,110,79,113,224,232,178,185,112,104,
                   218,246,97,228,251,34,242,193,238,210,144,12,191,179,162,241,
                   81,51,145,235,249,14,239,107,49,192,214,31,181,199,106,157,
                   184,84,204,176,115,121,50,45,127,4,150,254,138,236,205,93,
                   222,114,67,29,24,72,243,141,128,195,78,66,215,61,156,180];
    let mut j = 0;
    while j < 512 {
        p[j] = shuffle[j % 256];
        j += 1;
    }
    p
};

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}
fn grad(hash: usize, x: f64, y: f64) -> f64 {
    let h = hash & 7;
    let u = if h < 4 { x } else { y };
    let v = if h < 4 { y } else { x };
    (if (h & 1) == 0 { u } else { -u }) + (if (h & 2) == 0 { v } else { -v })
}
fn perlin(x: f64, y: f64) -> f64 {
    let xi = x.floor() as i32 & 255;
    let yi = y.floor() as i32 & 255;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);
    let p = &PERM;
    let aaa = p[p[xi as usize] + yi as usize];
    let aba = p[p[xi as usize] + yi as usize + 1];
    let baa = p[p[xi as usize + 1] + yi as usize];
    let bba = p[p[xi as usize + 1] + yi as usize + 1];
    let x1 = lerp(grad(aaa, xf, yf), grad(baa, xf - 1.0, yf), u);
    let x2 = lerp(grad(aba, xf, yf - 1.0), grad(bba, xf - 1.0, yf - 1.0), u);
    lerp(x1, x2, v)
}

/// v0.5 修复：使用 i64 避免溢出
fn worley(x: f64, y: f64) -> f64 {
    let cell_size = 32.0;
    let cx = (x / cell_size).floor() as i64;
    let cy = (y / cell_size).floor() as i64;
    let mut min_dist = 1e9;
    for dx in -1..=1 {
        for dy in -1..=1 {
            let ncx = cx + dx;
            let ncy = cy + dy;
            // 用 i64 计算避免 i32 溢出
            let mut h: u64 = (ncx.wrapping_mul(374761393) + ncy.wrapping_mul(668265263)) as u64;
            h = (h ^ (h >> 13)).wrapping_mul(1274126177);
            h = h ^ (h >> 16);
            let px = ncx as f64 * cell_size + (h % cell_size as u64) as f64;
            let py = ncy as f64 * cell_size + ((h >> 8) % cell_size as u64) as f64;
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if d < min_dist {
                min_dist = d;
            }
        }
    }
    min_dist
}

fn fbm(x: f64, y: f64, octaves: i32) -> f64 {
    let mut total = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut norm = 0.0;
    let oct = octaves.max(1).min(8);
    for _ in 0..oct {
        total += perlin(x * freq, y * freq) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    total / norm
}

// ============================================================
// 控制流信号
// ============================================================

#[derive(Debug)]
enum Control {
    Normal,                   // 正常执行，无控制流
    Break(Option<String>), // v0.4 带标签 break
    Continue,
    Return(Value),
}

type ExecResult = Result<Control, VglError>;

// ============================================================
// 解释器
// ============================================================

struct Interpreter {
    canvas: Option<Canvas>,
    layers: HashMap<String, Value>,
    struct_defs: HashMap<String, (Vec<String>, Vec<Value>)>,
    imported: Vec<String>,
    rng: Rc<RefCell<StdRng>>,
    current_dir: String,
    current_filename: String,
    current_src: String,
    current_pos: Option<usize>,
}

impl Interpreter {
    fn new() -> Self {
        Interpreter {
            canvas: None,
            layers: HashMap::new(),
            struct_defs: HashMap::new(),
            imported: Vec::new(),
            rng: Rc::new(RefCell::new(StdRng::from_entropy())),
            current_dir: ".".to_string(),
            current_filename: "".to_string(),
            current_src: "".to_string(),
            current_pos: None,
        }
    }

    fn eval(&mut self, expr: &Expr, env: Rc<RefCell<Env>>) -> VglResult<Value> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::String(s) => Ok(Value::String(s.clone())),
            Expr::Color(r, g, b) => Ok(Value::Color(*r, *g, *b)),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Ident(name) => {
                if let Some(v) = env.borrow().get(name) {
                    Ok(v)
                } else {
                    Err(VglError::new(format!("未定义变量: {}", name), self.current_pos))
                }
            }
            Expr::Tuple(el) => {
                let mut vals = Vec::new();
                for e in el {
                    vals.push(self.eval(e, env.clone())?);
                }
                Ok(Value::Tuple(vals))
            }
            Expr::Array(el) => {
                let mut vals = Vec::new();
                for e in el {
                    vals.push(self.eval(e, env.clone())?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(vals))))
            }
            Expr::BinOp(op, l, r) => {
                let lv = self.eval(l, env.clone())?;
                let rv = self.eval(r, env.clone())?;
                match (&lv, &rv) {
                    (Value::Number(a), Value::Number(b)) => {
                        let res = match op.as_str() {
                            "+" => a + b,
                            "-" => a - b,
                            "*" => a * b,
                            "/" => a / b,
                            "<" => return Ok(Value::Bool(a < b)),
                            ">" => return Ok(Value::Bool(a > b)),
                            "<=" => return Ok(Value::Bool(a <= b)),
                            ">=" => return Ok(Value::Bool(a >= b)),
                            "==" => return Ok(Value::Bool(a == b)),
                            "!=" => return Ok(Value::Bool(a != b)),
                            _ => return Err(VglError::new(format!("未知运算符 {}", op), self.current_pos)),
                        };
                        Ok(Value::Number(res))
                    }
                    (Value::Tuple(a), Value::Tuple(b)) => {
                        if a.len() != b.len() {
                            return Err(VglError::new("元组长度不匹配", self.current_pos));
                        }
                        let res = match op.as_str() {
                            "+" => a.iter().zip(b.iter()).map(|(x, y)| {
                                Value::Number(x.as_number().unwrap_or(0.0) + y.as_number().unwrap_or(0.0))
                            }).collect(),
                            "-" => a.iter().zip(b.iter()).map(|(x, y)| {
                                Value::Number(x.as_number().unwrap_or(0.0) - y.as_number().unwrap_or(0.0))
                            }).collect(),
                            _ => return Err(VglError::new("元组只支持 +/-", self.current_pos)),
                        };
                        Ok(Value::Tuple(res))
                    }
                    (Value::Tuple(t), Value::Number(n)) if op == "*" => {
                        let res = t.iter().map(|v| Value::Number(v.as_number().unwrap_or(0.0) * n)).collect();
                        Ok(Value::Tuple(res))
                    }
                    (Value::Number(n), Value::Tuple(t)) if op == "*" => {
                        let res = t.iter().map(|v| Value::Number(v.as_number().unwrap_or(0.0) * n)).collect();
                        Ok(Value::Tuple(res))
                    }
                    (Value::Tuple(t), Value::Number(n)) if op == "/" => {
                        let res = t.iter().map(|v| Value::Number(v.as_number().unwrap_or(0.0) / n)).collect();
                        Ok(Value::Tuple(res))
                    }
                    _ => Err(VglError::new(format!("类型不匹配: {:?} {} {:?}", lv, op, rv), self.current_pos)),
                }
            }
            Expr::LogicOp(op, l, r) => {
                let lv = self.eval(l, env.clone())?;
                let lb = match lv {
                    Value::Bool(b) => b,
                    Value::Number(n) => n != 0.0,
                    _ => return Err(VglError::new("逻辑运算需要 bool", self.current_pos)),
                };
                if op == "and" {
                    if !lb {
                        return Ok(Value::Bool(false));
                    }
                    let rv = self.eval(r, env.clone())?;
                    match rv {
                        Value::Bool(b) => Ok(Value::Bool(b)),
                        Value::Number(n) => Ok(Value::Bool(n != 0.0)),
                        _ => Err(VglError::new("逻辑运算需要 bool", self.current_pos)),
                    }
                } else {
                    if lb {
                        return Ok(Value::Bool(true));
                    }
                    let rv = self.eval(r, env.clone())?;
                    match rv {
                        Value::Bool(b) => Ok(Value::Bool(b)),
                        Value::Number(n) => Ok(Value::Bool(n != 0.0)),
                        _ => Err(VglError::new("逻辑运算需要 bool", self.current_pos)),
                    }
                }
            }
            Expr::UnaryNot(e) => {
                let v = self.eval(e, env.clone())?;
                match v {
                    Value::Bool(b) => Ok(Value::Bool(!b)),
                    Value::Number(n) => Ok(Value::Bool(n == 0.0)),
                    _ => Err(VglError::new("not 作用于非 bool", self.current_pos)),
                }
            }
            Expr::Call(name, args, kwargs) => self.eval_call(name, args, kwargs, env),
            Expr::Index(base, idx) => {
                let base_val = self.eval(base, env.clone())?;
                let idx_val = self.eval(idx, env.clone())?;
                match base_val {
                    Value::Tuple(t) => {
                        let i = idx_val.as_number().unwrap_or(0.0) as usize;
                        if i < t.len() {
                            Ok(t[i].clone())
                        } else {
                            Err(VglError::new("索引越界", self.current_pos))
                        }
                    }
                    Value::Array(arr) => {
                        let i = idx_val.as_number().unwrap_or(0.0) as usize;
                        let arr_ref = arr.borrow();
                        if i < arr_ref.len() {
                            Ok(arr_ref[i].clone())
                        } else {
                            Err(VglError::new("索引越界", self.current_pos))
                        }
                    }
                    Value::Dict(d) => {
                        let key = idx_val.as_string().unwrap_or_default();
                        let d_ref = d.borrow();
                        d_ref.get(&key).cloned().map(Ok).unwrap_or_else(|| {
                            Err(VglError::new(format!("键不存在: {}", key), self.current_pos))
                        })
                    }
                    _ => Err(VglError::new("不支持索引", self.current_pos)),
                }
            }
            Expr::FieldAccess(obj, field) => {
                let obj_val = self.eval(obj, env.clone())?;
                if let Value::Struct(s) = obj_val {
                    let s_ref = s.borrow();
                    s_ref.get(field).cloned().map(Ok).unwrap_or_else(|| {
                        Err(VglError::new(format!("字段不存在: {}", field), self.current_pos))
                    })
                } else {
                    Err(VglError::new("不是结构体", self.current_pos))
                }
            }
        }
    }

    fn eval_call(
        &mut self,
        name: &str,
        args: &[Expr],
        kwargs: &HashMap<String, Expr>,
        env: Rc<RefCell<Env>>,
    ) -> VglResult<Value> {
        // compose / fill 内建（需要 self）
        if name == "compose" {
            let layer_name = self.eval(&args[0], env.clone())?.as_string().unwrap_or_default();
            let blend = if args.len() > 1 {
                self.eval(&args[1], env.clone())?.as_string().unwrap_or_else(|| "over".to_string())
            } else {
                "over".to_string()
            };
            self.compose_layer(&layer_name, &blend)?;
            return Ok(Value::None);
        }
        if name == "fill" {
            let field_name = self.eval(&args[0], env.clone())?.as_string().unwrap_or_default();
            self.fill_field(&field_name, env)?;
            return Ok(Value::None);
        }
        // struct 构造
        if self.struct_defs.contains_key(name) {
            return self.construct_struct(name, args, kwargs, env);
        }
        // 求值参数
        let mut arg_vals = Vec::new();
        for a in args {
            arg_vals.push(self.eval(a, env.clone())?);
        }
        // 内建函数
        if let Some(v) = self.call_builtin(name, &arg_vals)? {
            return Ok(v);
        }
        // 用户函数 / 闭包
        if let Some(Value::Closure(_, params, body, closure_env)) = env.borrow().get(name) {
            let new_env = Rc::new(RefCell::new(Env::new(Some(closure_env.clone()))));
            for (i, p) in params.iter().enumerate() {
                if i < args.len() {
                    new_env.borrow_mut().vars.insert(p.clone(), arg_vals[i].clone());
                }
            }
            for (k, v) in kwargs {
                new_env.borrow_mut().vars.insert(k.clone(), self.eval(v, env.clone())?);
            }
            match self.execute_block(&body, new_env)? {
                Control::Return(v) => Ok(v),
                _ => Ok(Value::None),
            }
        } else {
            Err(VglError::new(format!("未定义函数: {}", name), self.current_pos))
        }
    }

    /// 调用内建函数。返回 Ok(Some(v)) 表示已处理，Ok(None) 表示非内建。
    /// v0.5 修复：rand 使用 self.rng 而非 thread_rng，使 seed 生效
    fn call_builtin(&mut self, name: &str, args: &[Value]) -> VglResult<Option<Value>> {
        macro_rules! num {
            ($i:expr) => {
                args.get($i).and_then(|v| v.as_number()).unwrap_or(0.0)
            };
        }
        let v = match name {
            "rand" => {
                let a = num!(0);
                let b = num!(1);
                let lo = a.min(b);
                let hi = a.max(b);
                Value::Number(self.rng.borrow_mut().gen_range(lo..hi))
            }
            "int" => Value::Number(num!(0).floor()),
            "abs" => Value::Number(num!(0).abs()),
            "floor" => Value::Number(num!(0).floor()),
            "ceil" => Value::Number(num!(0).ceil()),
            "sin" => Value::Number(num!(0).sin()),
            "cos" => Value::Number(num!(0).cos()),
            "sqrt" => Value::Number(num!(0).sqrt()),
            "pow" => Value::Number(num!(0).powf(num!(1))),
            "min" => Value::Number(num!(0).min(num!(1))),
            "max" => Value::Number(num!(0).max(num!(1))),
            "bool" => match &args.get(0) {
                Some(Value::Number(n)) => Value::Bool(*n != 0.0),
                Some(Value::Bool(b)) => Value::Bool(*b),
                _ => Value::Bool(true),
            },
            "len" => {
                let n = match &args.get(0) {
                    Some(Value::Tuple(t)) => t.len(),
                    Some(Value::Array(a)) => a.borrow().len(),
                    Some(Value::Dict(d)) => d.borrow().len(),
                    Some(Value::String(s)) => s.len(),
                    _ => 0,
                };
                Value::Number(n as f64)
            }
            "push" => {
                if let Some(Value::Array(arr)) = args.get(0) {
                    if let Some(v) = args.get(1) {
                        arr.borrow_mut().push(v.clone());
                    }
                }
                Value::None
            }
            "pop" => {
                if let Some(Value::Array(arr)) = args.get(0) {
                    arr.borrow_mut().pop().unwrap_or(Value::None)
                } else {
                    Value::None
                }
            }
            "array" => {
                if args.is_empty() {
                    Value::Array(Rc::new(RefCell::new(Vec::new())))
                } else if args.len() == 1 {
                    match &args[0] {
                        Value::Number(n) => Value::Array(Rc::new(RefCell::new(vec![Value::None; *n as usize]))),
                        _ => Value::Array(Rc::new(RefCell::new(args.to_vec()))),
                    }
                } else {
                    Value::Array(Rc::new(RefCell::new(args.to_vec())))
                }
            }
            "dict" => {
                let mut d: HashMap<String, Value> = HashMap::new();
                let mut i = 0;
                while i + 1 < args.len() {
                    let key = args[i].as_string().unwrap_or_else(|| format!("{:?}", args[i]));
                    d.insert(key, args[i + 1].clone());
                    i += 2;
                }
                Value::Dict(Rc::new(RefCell::new(d)))
            }
            "keys" => {
                if let Some(Value::Dict(d)) = args.get(0) {
                    let keys: Vec<Value> = d.borrow().keys().map(|k| Value::String(k.clone())).collect();
                    Value::Array(Rc::new(RefCell::new(keys)))
                } else {
                    Value::Array(Rc::new(RefCell::new(Vec::new())))
                }
            }
            "values" => {
                if let Some(Value::Dict(d)) = args.get(0) {
                    let vals: Vec<Value> = d.borrow().values().cloned().collect();
                    Value::Array(Rc::new(RefCell::new(vals)))
                } else {
                    Value::Array(Rc::new(RefCell::new(Vec::new())))
                }
            }
            "has" => {
                if let Some(Value::Dict(d)) = args.get(0) {
                    let key = args.get(1).and_then(|v| v.as_string()).unwrap_or_default();
                    Value::Bool(d.borrow().contains_key(&key))
                } else {
                    Value::Bool(false)
                }
            }
            "line" => {
                let p1 = args.get(0).and_then(|v| v.as_tuple()).unwrap_or_default();
                let p2 = args.get(1).and_then(|v| v.as_tuple()).unwrap_or_default();
                Value::Path("line".into(), vec![Value::Tuple(p1), Value::Tuple(p2)])
            }
            "circle" => {
                let cx = num!(0) as i32;
                let cy = num!(1) as i32;
                let r = num!(2) as i32;
                Value::Path("circle".into(), vec![Value::Number(cx as f64), Value::Number(cy as f64), Value::Number(r as f64)])
            }
            "bezier" => {
                let p1 = args.get(0).and_then(|v| v.as_tuple()).unwrap_or_default();
                let p2 = args.get(1).and_then(|v| v.as_tuple()).unwrap_or_default();
                let p3 = args.get(2).and_then(|v| v.as_tuple()).unwrap_or_default();
                let p4 = args.get(3).and_then(|v| v.as_tuple()).unwrap_or_default();
                Value::Path("bezier".into(), vec![Value::Tuple(p1), Value::Tuple(p2), Value::Tuple(p3), Value::Tuple(p4)])
            }
            "qbezier" => {
                let p1 = args.get(0).and_then(|v| v.as_tuple()).unwrap_or_default();
                let p2 = args.get(1).and_then(|v| v.as_tuple()).unwrap_or_default();
                let p3 = args.get(2).and_then(|v| v.as_tuple()).unwrap_or_default();
                Value::Path("qbezier".into(), vec![Value::Tuple(p1), Value::Tuple(p2), Value::Tuple(p3)])
            }
            "path" => {
                if let Some(Value::Array(arr)) = args.get(0) {
                    let pts = arr.borrow().clone();
                    Value::Path("polyline".into(), pts)
                } else {
                    Value::None
                }
            }
            "dot" => {
                let a = args.get(0).and_then(|v| v.as_tuple()).unwrap_or_default();
                let b = args.get(1).and_then(|v| v.as_tuple()).unwrap_or_default();
                let sum: f64 = a.iter().zip(b.iter())
                    .map(|(x, y)| x.as_number().unwrap_or(0.0) * y.as_number().unwrap_or(0.0))
                    .sum();
                Value::Number(sum)
            }
            "length" => {
                let p1 = args.get(0).and_then(|v| v.as_tuple()).unwrap_or_default();
                let p2 = args.get(1).and_then(|v| v.as_tuple()).unwrap_or_default();
                let d = if p1.len() >= 2 && p2.len() >= 2 {
                    let dx = p1[0].as_number().unwrap_or(0.0) - p2[0].as_number().unwrap_or(0.0);
                    let dy = p1[1].as_number().unwrap_or(0.0) - p2[1].as_number().unwrap_or(0.0);
                    (dx * dx + dy * dy).sqrt()
                } else {
                    0.0
                };
                Value::Number(d)
            }
            "perlin" => Value::Number(perlin(num!(0), num!(1))),
            "worley" => Value::Number(worley(num!(0), num!(1))),
            "fbm" => {
                let o = args.get(2).and_then(|v| v.as_number()).map(|n| n as i32).unwrap_or(4);
                Value::Number(fbm(num!(0), num!(1), o))
            }
            _ => return Ok(None),
        };
        Ok(Some(v))
    }

    fn exec(&mut self, sp: &StmtWithPos, env: Rc<RefCell<Env>>) -> ExecResult {
        // 更新当前语句位置（运行时错误定位）
        self.current_pos = Some(sp.pos);
        let stmt = &sp.stmt;
        match stmt {
            Stmt::Canvas(w, h) => {
                self.canvas = Some(Canvas::new(*w, *h));
                Ok(Control::Normal)
            }
            Stmt::Bg(expr) => {
                let val = self.eval(expr, env.clone())?;
                let (r, g, b) = match val {
                    Value::Color(r, g, b) => (r, g, b),
                    Value::Tuple(t) => {
                        if t.len() >= 3 {
                            (
                                clamp_u8(t[0].as_number().unwrap_or(0.0)),
                                clamp_u8(t[1].as_number().unwrap_or(0.0)),
                                clamp_u8(t[2].as_number().unwrap_or(0.0)),
                            )
                        } else {
                            return Err(VglError::new("bg 需要三元组", self.current_pos));
                        }
                    }
                    _ => {
                        let _ = self.eval_error("bg 需要颜色")?;
                        unreachable!()
                    }
                };
                if let Some(canvas) = &mut self.canvas {
                    canvas.fill(r, g, b);
                    canvas.bg = (r, g, b);
                }
                Ok(Control::Normal)
            }
            Stmt::Let(name, expr) => {
                let val = self.eval(expr, env.clone())?;
                env.borrow_mut().vars.insert(name.clone(), val);
                Ok(Control::Normal)
            }
            Stmt::Assign(name, expr) => {
                let val = self.eval(expr, env.clone())?;
                if let Err(e) = env.borrow_mut().set(name, val) {
                    self.eval_error(&e)?;
                }
                Ok(Control::Normal)
            }
            Stmt::For(var, start, end, body, label) => {
                let start_val = self.eval(start, env.clone())?;
                let end_val = self.eval(end, env.clone())?;
                let mut i = start_val.as_number().unwrap_or(0.0);
                let end = end_val.as_number().unwrap_or(0.0);
                while i < end {
                    let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    block_env.borrow_mut().vars.insert(var.clone(), Value::Number(i));
                    match self.execute_block(body, block_env)? {
                        Control::Normal => {}
                        Control::Continue => {}
                        Control::Break(None) => break,
                        Control::Break(Some(l)) => {
                            // 匹配 label：若匹配则终止本循环，否则向上传播
                            if label.as_deref() == Some(l.as_str()) {
                                break;
                            } else {
                                return Ok(Control::Break(Some(l)));
                            }
                        }
                        Control::Return(v) => return Ok(Control::Return(v)),
                    }
                    i += 1.0;
                }
                Ok(Control::Normal)
            }
            Stmt::While(cond, body, label) => {
                loop {
                    let cond_val = self.eval(cond, env.clone())?;
                    let b = match cond_val {
                        Value::Bool(b) => b,
                        Value::Number(n) => n != 0.0,
                        _ => false,
                    };
                    if !b {
                        break;
                    }
                    let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    match self.execute_block(body, block_env)? {
                        Control::Normal => {}
                        Control::Continue => {}
                        Control::Break(None) => break,
                        Control::Break(Some(l)) => {
                            if label.as_deref() == Some(l.as_str()) {
                                break;
                            } else {
                                return Ok(Control::Break(Some(l)));
                            }
                        }
                        Control::Return(v) => return Ok(Control::Return(v)),
                    }
                }
                Ok(Control::Normal)
            }
            Stmt::If(cond, then_body, else_body) => {
                let cond_val = self.eval(cond, env.clone())?;
                let b = match cond_val {
                    Value::Bool(b) => b,
                    Value::Number(n) => n != 0.0,
                    _ => false,
                };
                if b {
                    let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    self.execute_block(then_body, block_env)
                } else if let Some(eb) = else_body {
                    let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    self.execute_block(eb, block_env)
                } else {
                    Ok(Control::Normal)
                }
            }
            Stmt::Break(_) => {
                // break 在 parse 期已校验 loop_depth
                let label = if let Stmt::Break(l) = stmt { l.clone() } else { None };
                Ok(Control::Break(label))
            }
            Stmt::Continue => Ok(Control::Continue),
            Stmt::Seed(n) => {
                *self.rng.borrow_mut() = StdRng::seed_from_u64(*n);
                Ok(Control::Normal)
            }
            Stmt::FnDef(name, params, body) => {
                let closure = Value::Closure(name.clone(), params.clone(), body.clone(), env.clone());
                env.borrow_mut().vars.insert(name.clone(), closure);
                Ok(Control::Normal)
            }
            Stmt::Return(expr) => {
                let val = self.eval(expr, env.clone())?;
                Ok(Control::Return(val))
            }
            Stmt::Pixel(x_expr, y_expr, rgb_expr) => {
                let x = self.eval(x_expr, env.clone())?.as_number().unwrap_or(0.0) as i32;
                let y = self.eval(y_expr, env.clone())?.as_number().unwrap_or(0.0) as i32;
                let rgb_val = self.eval(rgb_expr, env.clone())?;
                let (r, g, b) = match rgb_val {
                    Value::Color(r, g, b) => (r, g, b),
                    Value::Tuple(t) => (
                        clamp_u8(t.get(0).and_then(|v| v.as_number()).unwrap_or(0.0)),
                        clamp_u8(t.get(1).and_then(|v| v.as_number()).unwrap_or(0.0)),
                        clamp_u8(t.get(2).and_then(|v| v.as_number()).unwrap_or(0.0)),
                    ),
                    _ => {
                        let _ = self.eval_error("rgb 需要颜色或三元组")?;
                        unreachable!()
                    }
                };
                if let Some(canvas) = &mut self.canvas {
                    canvas.put_pixel(x, y, r, g, b);
                }
                Ok(Control::Normal)
            }
            Stmt::Stroke(fields) => self.exec_stroke(fields, env),
            Stmt::Render(fname) => {
                if let Some(canvas) = &self.canvas {
                    // 自动创建输出目录
                    if let Some(parent) = Path::new(fname).parent() {
                        if !parent.as_os_str().is_empty() {
                            let _ = fs::create_dir_all(parent);
                        }
                    }
                    let img = ImageBuffer::<Rgb<u8>, _>::from_vec(
                        canvas.width,
                        canvas.height,
                        canvas.pixels.clone(),
                    )
                    .ok_or_else(|| VglError::new("渲染缓冲区创建失败", self.current_pos))?;
                    img.save(fname).map_err(|e| {
                        eprintln!("渲染失败: {}", e);
                        VglError::new(format!("渲染失败: {}", e), self.current_pos)
                    })?;
                    println!("已渲染: {} ({}x{})", fname, canvas.width, canvas.height);
                }
                Ok(Control::Normal)
            }
            Stmt::StructDef(name, fields) => {
                let mut def_names = Vec::new();
                let mut def_vals = Vec::new();
                for (fname, expr) in fields {
                    def_names.push(fname.clone());
                    def_vals.push(self.eval(expr, env.clone())?);
                }
                self.struct_defs.insert(name.clone(), (def_names, def_vals));
                Ok(Control::Normal)
            }
            Stmt::Import(path) => self.do_import(path, env),
            Stmt::MaterialDef(name, fields) => {
                let mut map = HashMap::new();
                for (k, v) in fields {
                    map.insert(k.clone(), self.eval(v, env.clone())?);
                }
                let mat = Value::Material(map);
                env.borrow_mut().vars.insert(name.clone(), mat);
                Ok(Control::Normal)
            }
            Stmt::LayerDef(name, body) => self.exec_layer(name, body, env),
            Stmt::FieldDef(name, params, body) => {
                let closure = Value::Closure(name.clone(), params.clone(), body.clone(), env.clone());
                env.borrow_mut().vars.insert(name.clone(), closure);
                Ok(Control::Normal)
            }
            Stmt::IndexAssign(base, idx, expr) => {
                let base_val = self.eval(base, env.clone())?;
                let idx_val = self.eval(idx, env.clone())?;
                let val = self.eval(expr, env.clone())?;
                match base_val {
                    Value::Array(arr) => {
                        let i = idx_val.as_number().unwrap_or(0.0) as usize;
                        let mut arr_ref = arr.borrow_mut();
                        if i < arr_ref.len() {
                            arr_ref[i] = val;
                            Ok(Control::Normal)
                        } else {
                            self.eval_error("索引越界")
                        }
                    }
                    Value::Dict(d) => {
                        let key = idx_val.as_string().unwrap_or_default();
                        d.borrow_mut().insert(key, val);
                        Ok(Control::Normal)
                    }
                    Value::Struct(s) => {
                        let field = idx_val.as_string().unwrap_or_default();
                        s.borrow_mut().insert(field, val);
                        Ok(Control::Normal)
                    }
                    _ => self.eval_error("索引赋值不支持该类型"),
                }
            }
            Stmt::FieldAssign(obj, field, expr) => {
                let obj_val = self.eval(obj, env.clone())?;
                let val = self.eval(expr, env.clone())?;
                if let Value::Struct(s) = obj_val {
                    let mut s_ref = s.borrow_mut();
                    s_ref.insert(field.clone(), val);
                    Ok(Control::Normal)
                } else {
                    self.eval_error("字段赋值目标不是结构体")
                }
            }
            Stmt::ExprStmt(expr) => {
                self.eval(expr, env.clone())?;
                Ok(Control::Normal)
            }
        }
    }

    /// 辅助：产生 VglError 并打印后退出。返回类型为 ExecResult 以便在 exec 中用 ? 传播。
    /// 实际上它直接 std::process::exit，返回值仅为满足类型检查。
    fn eval_error(&self, msg: &str) -> Result<Control, VglError> {
        let full = format_error(
            msg,
            &self.current_src,
            self.current_pos,
            &self.current_filename,
        );
        eprintln!("VGL 错误: {}", full);
        std::process::exit(1);
    }

    fn exec_stroke(&mut self, fields: &HashMap<String, Expr>, env: Rc<RefCell<Env>>) -> ExecResult {
        // v0.4 块作用域：stroke 块创建子 Environment
        let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
        let path_val = self.eval(
            fields.get("path").unwrap_or(&Expr::Ident("none".into())),
            block_env.clone(),
        )?;
        let width = self
            .eval(fields.get("width").unwrap_or(&Expr::Number(1.0)), block_env.clone())?
            .as_number()
            .unwrap_or(1.0);
        // v0.5 批次 C：material 字段优先（覆盖 color），支持 noise 扰动
        let color_val = if let Some(mat_expr) = fields.get("material") {
            let mat_val = self.eval(mat_expr, block_env.clone())?;
            if let Value::Material(mat_map) = mat_val {
                let mut color = mat_map.get("color").cloned().unwrap_or(Value::Color(0, 0, 0));
                if let Some(noise) = mat_map.get("noise") {
                    let nv = noise.as_number().unwrap_or(0.0);
                    if nv != 0.0 {
                        let noise_val = perlin(width * 10.0, 0.0) * nv;
                        if let Value::Color(r, g, b) = color {
                            color = Value::Color(
                                clamp_u8(r as f64 + noise_val * 255.0),
                                clamp_u8(g as f64 + noise_val * 255.0),
                                clamp_u8(b as f64 + noise_val * 255.0),
                            );
                        }
                    }
                }
                color
            } else {
                let _ = self.eval_error("material 必须是材质类型")?;
                unreachable!()
            }
        } else {
            self.eval(fields.get("color").unwrap_or(&Expr::Color(0, 0, 0)), block_env.clone())?
        };
        let (r, g, b) = match color_val {
            Value::Color(r, g, b) => (r, g, b),
            Value::Tuple(t) => (
                clamp_u8(t.get(0).and_then(|v| v.as_number()).unwrap_or(0.0)),
                clamp_u8(t.get(1).and_then(|v| v.as_number()).unwrap_or(0.0)),
                clamp_u8(t.get(2).and_then(|v| v.as_number()).unwrap_or(0.0)),
            ),
            _ => {
                let _ = self.eval_error("color 错误")?;
                unreachable!()
            }
        };
        // v0.5 批次 B：samples 字段支持
        let samples = fields
            .get("samples")
            .map(|e| self.eval(e, block_env.clone()).map(|v| v.as_number().unwrap_or(0.0) as i32))
            .transpose()?
            .unwrap_or(0);
        if let Some(canvas) = &mut self.canvas {
            match path_val {
                Value::Path(tag, args) => match tag.as_str() {
                    "line" => {
                        let p1 = args.get(0).and_then(|v| v.as_tuple()).unwrap_or_default();
                        let p2 = args.get(1).and_then(|v| v.as_tuple()).unwrap_or_default();
                        canvas.draw_line(
                            p1.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                            p1.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                            p2.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                            p2.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                            width, r, g, b,
                        );
                    }
                    "circle" => {
                        let cx = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
                        let cy = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
                        let rad = args.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
                        canvas.draw_circle(cx, cy, rad, width, r, g, b);
                    }
                    "bezier" => {
                        let p1 = args.get(0).and_then(|v| v.as_tuple()).unwrap_or_default();
                        let p2 = args.get(1).and_then(|v| v.as_tuple()).unwrap_or_default();
                        let p3 = args.get(2).and_then(|v| v.as_tuple()).unwrap_or_default();
                        let p4 = args.get(3).and_then(|v| v.as_tuple()).unwrap_or_default();
                        let n = if samples > 0 { samples as usize } else { 64 };
                        let pts = canvas.sample_bezier3(
                            (p1.get(0).and_then(|v| v.as_number()).unwrap_or(0.0),
                             p1.get(1).and_then(|v| v.as_number()).unwrap_or(0.0)),
                            (p2.get(0).and_then(|v| v.as_number()).unwrap_or(0.0),
                             p2.get(1).and_then(|v| v.as_number()).unwrap_or(0.0)),
                            (p3.get(0).and_then(|v| v.as_number()).unwrap_or(0.0),
                             p3.get(1).and_then(|v| v.as_number()).unwrap_or(0.0)),
                            (p4.get(0).and_then(|v| v.as_number()).unwrap_or(0.0),
                             p4.get(1).and_then(|v| v.as_number()).unwrap_or(0.0)),
                            n,
                        );
                        for i in 0..pts.len().saturating_sub(1) {
                            canvas.draw_line(pts[i].0, pts[i].1, pts[i + 1].0, pts[i + 1].1, width, r, g, b);
                        }
                    }
                    "qbezier" => {
                        let p1 = args.get(0).and_then(|v| v.as_tuple()).unwrap_or_default();
                        let p2 = args.get(1).and_then(|v| v.as_tuple()).unwrap_or_default();
                        let p3 = args.get(2).and_then(|v| v.as_tuple()).unwrap_or_default();
                        let n = if samples > 0 { samples as usize } else { 32 };
                        let pts = canvas.sample_bezier2(
                            (p1.get(0).and_then(|v| v.as_number()).unwrap_or(0.0),
                             p1.get(1).and_then(|v| v.as_number()).unwrap_or(0.0)),
                            (p2.get(0).and_then(|v| v.as_number()).unwrap_or(0.0),
                             p2.get(1).and_then(|v| v.as_number()).unwrap_or(0.0)),
                            (p3.get(0).and_then(|v| v.as_number()).unwrap_or(0.0),
                             p3.get(1).and_then(|v| v.as_number()).unwrap_or(0.0)),
                            n,
                        );
                        for i in 0..pts.len().saturating_sub(1) {
                            canvas.draw_line(pts[i].0, pts[i].1, pts[i + 1].0, pts[i + 1].1, width, r, g, b);
                        }
                    }
                    "polyline" => {
                        if args.len() > 1 {
                            for i in 0..args.len() - 1 {
                                let p1 = args[i].as_tuple().unwrap_or_default();
                                let p2 = args[i + 1].as_tuple().unwrap_or_default();
                                canvas.draw_line(
                                    p1.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                    p1.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                    p2.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                    p2.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                    width, r, g, b,
                                );
                            }
                        }
                    }
                    _ => {}
                },
                _ => {
                    let _ = self.eval_error("path 不是路径")?;
                    unreachable!()
                }
            }
        }
        Ok(Control::Normal)
    }

    fn do_import(&mut self, path: &str, env: Rc<RefCell<Env>>) -> ExecResult {
        let full_path = if path.starts_with('/') || (path.starts_with('.') && path.len() > 1 && path.as_bytes()[1] == b'/') {
            path.to_string()
        } else {
            format!("{}/{}", self.current_dir, path)
        };
        let full_abs = match fs::canonicalize(&full_path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => full_path.clone(),
        };
        if self.imported.contains(&full_abs) {
            return Ok(Control::Normal);
        }
        self.imported.push(full_abs.clone());
        let src = match fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(e) => {
                let _ = self.eval_error(&format!("无法导入模块 {}: {}", path, e))?;
                unreachable!()
            }
        };
        // 切换文件上下文（错误定位 + 嵌套 import 路径解析）
        let old_dir = self.current_dir.clone();
        let old_fn = self.current_filename.clone();
        let old_src = self.current_src.clone();
        let old_pos = self.current_pos;
        self.current_dir = Path::new(&full_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        self.current_filename = full_path.clone();
        self.current_src = src.clone();
        let result = (|| -> VglResult<()> {
            let mut lexer = Lexer::new(&src);
            let tokens = lexer.tokenize()?;
            let mut parser = Parser::new(tokens);
            let ast = parser.parse_program()?;
            for s in ast {
                self.exec(&s, env.clone()).map_err(|_| {
                    VglError::new("import 子文件执行失败", Some(0))
                })?;
            }
            Ok(())
        })();
        match result {
            Ok(_) => {
                // 正常完成，恢复主文件上下文
                self.current_dir = old_dir;
                self.current_filename = old_fn;
                self.current_src = old_src;
                self.current_pos = old_pos;
                Ok(Control::Normal)
            }
            Err(e) => {
                // 异常时保留子文件上下文（错误定位用）
                let full = format_error(&e.msg, &self.current_src, e.pos, &self.current_filename);
                eprintln!("VGL 错误: {}", full);
                std::process::exit(1);
            }
        }
    }

    fn exec_layer(&mut self, name: &str, body: &[StmtWithPos], env: Rc<RefCell<Env>>) -> ExecResult {
        if let Some(canvas) = &self.canvas {
            let mut layer_canvas = Canvas::new(canvas.width, canvas.height);
            layer_canvas.fill(canvas.bg.0, canvas.bg.1, canvas.bg.2);
            let old_canvas = std::mem::replace(&mut self.canvas, Some(layer_canvas));
            let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
            let _ = self.execute_block(body, block_env);
            let new_canvas = self.canvas.take().unwrap();
            self.layers
                .insert(name.to_string(), Value::Layer(Rc::new(RefCell::new(new_canvas))));
            self.canvas = old_canvas;
        }
        Ok(Control::Normal)
    }

    fn execute_block(&mut self, body: &[StmtWithPos], env: Rc<RefCell<Env>>) -> ExecResult {
        for stmt in body {
            match self.exec(stmt, env.clone())? {
                Control::Normal => {},
                c => return Ok(c),
            }
        }
        Ok(Control::Normal)
    }

    fn construct_struct(
        &mut self,
        name: &str,
        args: &[Expr],
        kwargs: &HashMap<String, Expr>,
        env: Rc<RefCell<Env>>,
    ) -> VglResult<Value> {
        let (field_names, default_vals) = self.struct_defs.get(name).cloned().unwrap_or_default();
        let mut fields = HashMap::new();
        for (fname, dval) in field_names.iter().zip(default_vals.iter()) {
            fields.insert(fname.clone(), dval.clone());
        }
        for (i, arg_expr) in args.iter().enumerate() {
            if i < field_names.len() {
                let val = self.eval(arg_expr, env.clone())?;
                fields.insert(field_names[i].clone(), val);
            } else {
                return Err(VglError::new("参数过多", self.current_pos));
            }
        }
        for (k, v_expr) in kwargs {
            if fields.contains_key(k) {
                let val = self.eval(v_expr, env.clone())?;
                fields.insert(k.clone(), val);
            } else {
                return Err(VglError::new(format!("未知字段: {}", k), self.current_pos));
            }
        }
        Ok(Value::Struct(Rc::new(RefCell::new(fields))))
    }

    fn compose_layer(&mut self, name: &str, blend: &str) -> VglResult<()> {
        let layer_rc = match self.layers.get(name) {
            Some(Value::Layer(lc)) => lc.clone(),
            _ => return Err(VglError::new(format!("未找到图层: {}", name), self.current_pos)),
        };
        let layer = layer_rc.borrow();
        let lw = layer.width;
        let lh = layer.height;
        if let Some(canvas) = &mut self.canvas {
            if canvas.width != lw || canvas.height != lh {
                return Err(VglError::new("图层尺寸不匹配", self.current_pos));
            }
            for y in 0..lh {
                for x in 0..lw {
                    let idx = (y * lw + x) as usize * 3;
                    let mr = canvas.pixels[idx] as f64;
                    let mg = canvas.pixels[idx + 1] as f64;
                    let mb = canvas.pixels[idx + 2] as f64;
                    let lr = layer.pixels[idx] as f64;
                    let lg = layer.pixels[idx + 1] as f64;
                    let lb = layer.pixels[idx + 2] as f64;
                    let (nr, ng, nb) = match blend {
                        "add" => (
                            (mr + lr).min(255.0),
                            (mg + lg).min(255.0),
                            (mb + lb).min(255.0),
                        ),
                        "mul" => (mr * lr / 255.0, mg * lg / 255.0, mb * lb / 255.0),
                        "screen" => (
                            255.0 - (255.0 - mr) * (255.0 - lr) / 255.0,
                            255.0 - (255.0 - mg) * (255.0 - lg) / 255.0,
                            255.0 - (255.0 - mb) * (255.0 - lb) / 255.0,
                        ),
                        _ => {
                            let alpha = (lr + lg + lb) / (3.0 * 255.0);
                            (
                                mr * (1.0 - alpha) + lr * alpha,
                                mg * (1.0 - alpha) + lg * alpha,
                                mb * (1.0 - alpha) + lb * alpha,
                            )
                        }
                    };
                    canvas.pixels[idx] = clamp_u8(nr);
                    canvas.pixels[idx + 1] = clamp_u8(ng);
                    canvas.pixels[idx + 2] = clamp_u8(nb);
                }
            }
        }
        Ok(())
    }

    fn fill_field(&mut self, name: &str, env: Rc<RefCell<Env>>) -> VglResult<()> {
        let closure = env.borrow().get(name);
        let (params, body, def_env) = match closure {
            Some(Value::Closure(_, p, b, e)) => (p, b, e),
            _ => return Err(VglError::new(format!("未找到颜色场: {}", name), self.current_pos)),
        };
        let (w, h) = match &self.canvas {
            Some(c) => (c.width, c.height),
            None => return Ok(()),
        };
        for y in 0..h {
            for x in 0..w {
                let call_env = Rc::new(RefCell::new(Env::new(Some(def_env.clone()))));
                if !params.is_empty() {
                    call_env.borrow_mut().vars.insert(params[0].clone(), Value::Number(x as f64));
                }
                if params.len() > 1 {
                    call_env.borrow_mut().vars.insert(params[1].clone(), Value::Number(y as f64));
                }
                let result = match self.execute_block(&body, call_env) {
                    Ok(Control::Return(v)) => v,
                    Ok(_) => Value::None,
                    Err(_) => Value::None,
                };
                let color = match result {
                    Value::Color(r, g, b) => Some((r, g, b)),
                    Value::Tuple(ref t) if t.len() >= 3 => Some((
                        clamp_u8(t.get(0).and_then(|v| v.as_number()).unwrap_or(0.0)),
                        clamp_u8(t.get(1).and_then(|v| v.as_number()).unwrap_or(0.0)),
                        clamp_u8(t.get(2).and_then(|v| v.as_number()).unwrap_or(0.0)),
                    )),
                    _ => None,
                };
                if let Some((r, g, b)) = color {
                    if let Some(canvas) = &mut self.canvas {
                        canvas.put_pixel(x as i32, y as i32, r, g, b);
                    }
                }
            }
        }
        Ok(())
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("用法: vgl <file.vgl>");
        std::process::exit(1);
    }
    let filename = &args[1];
    let src = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("无法读取文件 {}: {}", filename, e);
            std::process::exit(1);
        }
    };
    let mut interp = Interpreter::new();
    interp.current_dir = Path::new(filename)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    interp.current_filename = filename.clone();
    interp.current_src = src.clone();
    if let Ok(abs) = fs::canonicalize(filename) {
        interp.imported.push(abs.to_string_lossy().to_string());
    }

    let result: VglResult<()> = (|| {
        let mut lexer = Lexer::new(&src);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let ast = parser.parse_program()?;
        let global_env = Rc::new(RefCell::new(Env::new(None)));
        for sp in &ast {
            match interp.exec(sp, global_env.clone()) {
                Ok(Control::Normal) => {}
                Ok(Control::Return(_)) => {}
                Ok(Control::Break(_)) | Ok(Control::Continue) => {
                    eprintln!("{}: 控制流信号泄漏到顶层（不应发生）", filename);
                    std::process::exit(1);
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    })();

    if let Err(e) = result {
        let full = format_error(&e.msg, &interp.current_src, e.pos, &interp.current_filename);
        eprintln!("VGL 错误: {}", full);
        std::process::exit(1);
    }
}
