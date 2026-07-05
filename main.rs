use std::collections::HashMap;
use std::env;
use std::fs;
use std::rc::Rc;
use std::cell::RefCell;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use image::{ImageBuffer, Rgb};

fn clamp_u8(v: f64) -> u8 { if v < 0.0 { 0 } else if v > 255.0 { 255 } else { v as u8 } }

// ---------- 词法 ----------
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    String(String),
    Color(u8,u8,u8),
    Ident(String),
    Keyword(String),
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    Comma, Colon, Dot, DotDot,
    Op(String),
    Eof,
}

struct Lexer { chars: Vec<char>, pos: usize }
impl Lexer {
    fn new(s: &str) -> Self { Lexer { chars: s.chars().collect(), pos: 0 } }
    fn peek(&self) -> Option<char> { self.chars.get(self.pos).copied() }
    fn advance(&mut self) -> Option<char> { let c = self.peek(); self.pos += 1; c }
    fn skip_ws(&mut self) {
        while let Some(c)=self.peek() { if !c.is_whitespace() { break } self.advance(); }
    }
    fn read_number(&mut self) -> f64 {
        let start = self.pos;
        while let Some(c)=self.peek() {
            if c.is_ascii_digit() { self.advance(); } else { break; }
        }
        if let Some('.') = self.peek() {
            if self.pos + 1 >= self.chars.len() || self.chars[self.pos + 1] != '.' {
                self.advance();
                while let Some(c)=self.peek() {
                    if c.is_ascii_digit() { self.advance(); } else { break; }
                }
            }
        }
        self.chars[start..self.pos].iter().collect::<String>().parse().unwrap()
    }
    fn read_ident(&mut self) -> String {
        let start = self.pos;
        while let Some(c)=self.peek() {
            if c.is_alphanumeric() || c=='_' { self.advance(); } else { break; }
        }
        self.chars[start..self.pos].iter().collect()
    }
    fn read_string(&mut self) -> String {
        self.advance();
        let mut result = String::new();
        while let Some(c) = self.peek() {
            if c == '"' { break; }
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
                        _ => { result.push('\\'); result.push(nxt); }
                    }
                    self.advance();
                }
                continue;
            }
            result.push(c);
            self.advance();
        }
        self.advance();
        result
    }
    fn read_color(&mut self) -> (u8,u8,u8) {
        self.advance();
        let start = self.pos;
        while let Some(c)=self.peek() { if c.is_ascii_hexdigit() { self.advance(); } else { break; } }
        let hex: String = self.chars[start..self.pos].iter().collect();
        if hex.len()==6 {
            let r=u8::from_str_radix(&hex[0..2],16).unwrap();
            let g=u8::from_str_radix(&hex[2..4],16).unwrap();
            let b=u8::from_str_radix(&hex[4..6],16).unwrap();
            (r,g,b)
        } else if hex.len()==3 {
            let r=u8::from_str_radix(&format!("{}{}",&hex[0..1],&hex[0..1]),16).unwrap();
            let g=u8::from_str_radix(&format!("{}{}",&hex[1..2],&hex[1..2]),16).unwrap();
            let b=u8::from_str_radix(&format!("{}{}",&hex[2..3],&hex[2..3]),16).unwrap();
            (r,g,b)
        } else { panic!("非法颜色 #{}", hex) }
    }
    fn next_token(&mut self) -> Token {
        loop {
            self.skip_ws();
            let c = match self.peek() { None => return Token::Eof, Some(ch) => ch };
            if c == '/' && self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '/' {
                self.pos += 2;
                while let Some(ch) = self.peek() {
                    if ch == '\n' { self.advance(); break; }
                    self.advance();
                }
                continue;
            }
            if c == '/' && self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '*' {
                self.pos += 2;
                loop {
                    match self.peek() {
                        None => panic!("未闭合的块注释"),
                        Some('*') if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '/' => {
                            self.pos += 2;
                            break;
                        },
                        _ => { self.advance(); }
                    }
                }
                continue;
            }
            break;
        }
        let c = match self.peek() { None => return Token::Eof, Some(ch) => ch };
        if c == '.' && self.pos + 1 < self.chars.len() {
            if self.chars[self.pos + 1] == '.' {
                self.advance(); self.advance();
                return Token::DotDot;
            }
            if self.chars[self.pos + 1].is_ascii_digit() {
                return Token::Number(self.read_number());
            }
        }
        if c.is_ascii_digit() {
            return Token::Number(self.read_number());
        }
        if c=='"' { return Token::String(self.read_string()); }
        if c=='#' { let (r,g,b)=self.read_color(); return Token::Color(r,g,b); }
        if c.is_alphabetic() || c=='_' {
            let ident = self.read_ident();
            let kw = ["canvas","bg","let","for","in","if","else","fn","return","pixel","stroke","render",
                      "while","break","and","or","not","seed","true","false","continue","struct","import",
                      "material","layer","field"];
            if kw.contains(&ident.as_str()) { return Token::Keyword(ident); }
            return Token::Ident(ident);
        }
        match c {
            '(' => { self.advance(); Token::LParen },
            ')' => { self.advance(); Token::RParen },
            '{' => { self.advance(); Token::LBrace },
            '}' => { self.advance(); Token::RBrace },
            '[' => { self.advance(); Token::LBracket },
            ']' => { self.advance(); Token::RBracket },
            ',' => { self.advance(); Token::Comma },
            ':' => { self.advance(); Token::Colon },
            '.' => { self.advance(); Token::Dot },
            '+'|'-'|'*'|'/'|'='|'<'|'>'|'!' => {
                self.advance(); let mut op = c.to_string();
                if let Some(nxt)=self.peek() {
                    if (c=='<'||c=='>'||c=='='||c=='!') && nxt=='=' {
                        self.advance(); op.push('=');
                    }
                }
                Token::Op(op)
            },
            _ => panic!("非法字符 '{}'", c)
        }
    }
    fn tokenize(&mut self) -> Vec<Token> {
        let mut toks = Vec::new();
        loop { let t = self.next_token(); let is_eof = matches!(&t, Token::Eof); toks.push(t); if is_eof { break; } }
        toks
    }
}

// ---------- AST ----------
#[derive(Clone, Debug)]
enum Expr {
    Number(f64), String(String), Color(u8,u8,u8), Bool(bool), Ident(String),
    Tuple(Vec<Expr>), Array(Vec<Expr>),
    BinOp(String, Box<Expr>, Box<Expr>), LogicOp(String, Box<Expr>, Box<Expr>), UnaryNot(Box<Expr>),
    Index(Box<Expr>, Box<Expr>), FieldAccess(Box<Expr>, String),
    Call(String, Vec<Expr>, HashMap<String, Expr>),
}

#[derive(Clone, Debug)]
enum Stmt {
    Canvas(u32,u32),
    Bg(Expr),
    Let(String, Expr),
    Assign(String, Expr),
    For(String, Expr, Expr, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    If(Expr, Vec<Stmt>, Option<Vec<Stmt>>),
    Break,
    Continue,
    Seed(u64),
    FnDef(String, Vec<String>, Vec<Stmt>),
    Return(Expr),
    Pixel(Expr, Expr, Expr),
    Stroke(HashMap<String, Expr>),
    Render(String),
    StructDef(String, Vec<(String, Expr)>),
    Import(String),
    MaterialDef(String, HashMap<String, Expr>),
    LayerDef(String, Vec<Stmt>),
    FieldDef(String, Vec<String>, Vec<Stmt>),
    IndexAssign(Expr, Expr, Expr),
    FieldAssign(Expr, String, Expr),
    ExprStmt(Expr),
}

// ---------- Parser ----------
struct Parser { tokens: Vec<Token>, pos: usize }
impl Parser {
    fn peek(&self) -> &Token { &self.tokens[self.pos] }
    fn advance(&mut self) -> Token { let t = self.peek().clone(); self.pos += 1; t }
    fn expect(&mut self, tok: Token) { if *self.peek() != tok { panic!("期望 {:?}, 得到 {:?}", tok, self.peek()); } self.advance(); }
    fn parse_program(&mut self) -> Vec<Stmt> {
        let mut stmts = Vec::new();
        while !matches!(self.peek(), Token::Eof) { stmts.push(self.parse_stmt()); }
        stmts
    }
    fn parse_stmt(&mut self) -> Stmt {
        match self.peek() {
            Token::Keyword(ref kw) => match kw.as_str() {
                "canvas" => { self.advance();
                    let w = if let Token::Number(n)=self.advance() { n as u32 } else { panic!() };
                    let peeked = self.peek().clone();
                    let h = match &peeked {
                        Token::Ident(s) if s.len() > 1 && s.as_bytes()[0] == b'x' => {
                            self.advance();
                            s[1..].parse::<u32>().unwrap()
                        },
                        Token::Number(n) => { self.advance(); *n as u32 },
                        _ => panic!("canvas 需要宽x高"),
                    };
                    Stmt::Canvas(w,h) },
                "bg" => { self.advance(); Stmt::Bg(self.parse_expr()) },
                "let" => { self.advance();
                    let name = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                    self.expect(Token::Op("=".to_string()));
                    Stmt::Let(name, self.parse_expr()) },
                "for" => { self.advance();
                    let var = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                    self.expect(Token::Keyword("in".to_string()));
                    let start = self.parse_expr();
                    self.expect(Token::DotDot);
                    let end = self.parse_expr();
                    self.expect(Token::LBrace);
                    let mut body = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) { body.push(self.parse_stmt()); }
                    self.expect(Token::RBrace);
                    Stmt::For(var, start, end, body) },
                "while" => { self.advance();
                    let cond = self.parse_expr();
                    self.expect(Token::LBrace);
                    let mut body = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) { body.push(self.parse_stmt()); }
                    self.expect(Token::RBrace);
                    Stmt::While(cond, body) },
                "if" => { self.advance();
                    let cond = self.parse_expr();
                    self.expect(Token::LBrace);
                    let mut then_body = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) { then_body.push(self.parse_stmt()); }
                    self.expect(Token::RBrace);
                    let else_body = if matches!(self.peek(), Token::Keyword(ref k) if k=="else") {
                        self.advance();
                        self.expect(Token::LBrace);
                        let mut b = Vec::new();
                        while !matches!(self.peek(), Token::RBrace) { b.push(self.parse_stmt()); }
                        self.expect(Token::RBrace);
                        Some(b)
                    } else { None };
                    Stmt::If(cond, then_body, else_body) },
                "break" => { self.advance(); Stmt::Break },
                "continue" => { self.advance(); Stmt::Continue },
                "seed" => { self.advance();
                    let n = if let Token::Number(v)=self.advance() { v as u64 } else { panic!() };
                    Stmt::Seed(n) },
                "fn" => { self.advance();
                    let name = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                    self.expect(Token::LParen);
                    let mut params = Vec::new();
                    if !matches!(self.peek(), Token::RParen) {
                        if let Token::Ident(s)=self.advance() { params.push(s); } else { panic!() };
                        while matches!(self.peek(), Token::Comma) {
                            self.advance();
                            if let Token::Ident(s)=self.advance() { params.push(s); } else { panic!() }
                        }
                    }
                    self.expect(Token::RParen);
                    self.expect(Token::LBrace);
                    let mut body = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) { body.push(self.parse_stmt()); }
                    self.expect(Token::RBrace);
                    Stmt::FnDef(name, params, body) },
                "return" => { self.advance(); Stmt::Return(self.parse_expr()) },
                "pixel" => { self.advance();
                    self.expect(Token::LParen);
                    let mut map = HashMap::new();
                    while !matches!(self.peek(), Token::RParen) {
                        let key = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                        self.expect(Token::Colon);
                        let val = self.parse_expr();
                        map.insert(key, val);
                        if matches!(self.peek(), Token::Comma) { self.advance(); }
                    }
                    self.expect(Token::RParen);
                    let x = map.get("x").cloned().unwrap_or(Expr::Number(0.0));
                    let y = map.get("y").cloned().unwrap_or(Expr::Number(0.0));
                    let rgb = map.get("rgb").cloned().unwrap_or(Expr::Color(0,0,0));
                    Stmt::Pixel(x,y,rgb) },
                "stroke" => { self.advance();
                    self.expect(Token::LBrace);
                    let mut fields = HashMap::new();
                    while !matches!(self.peek(), Token::RBrace) {
                        let key = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                        self.expect(Token::Colon);
                        let val = self.parse_expr();
                        fields.insert(key, val);
                        if matches!(self.peek(), Token::Comma) { self.advance(); }
                    }
                    self.expect(Token::RBrace);
                    Stmt::Stroke(fields) },
                "render" => { self.advance();
                    let fname = if let Token::String(s)=self.advance() { s } else { panic!() };
                    Stmt::Render(fname) },
                "struct" => { self.advance();
                    let name = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                    self.expect(Token::LBrace);
                    let mut fields = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) {
                        let fname = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                        self.expect(Token::Colon);
                        let default = self.parse_expr();
                        fields.push((fname, default));
                        if matches!(self.peek(), Token::Comma) { self.advance(); }
                    }
                    self.expect(Token::RBrace);
                    Stmt::StructDef(name, fields) },
                "import" => { self.advance();
                    let path = if let Token::String(s)=self.advance() { s } else { panic!() };
                    Stmt::Import(path) },
                "material" => { self.advance();
                    let name = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                    self.expect(Token::LBrace);
                    let mut fields = HashMap::new();
                    while !matches!(self.peek(), Token::RBrace) {
                        let key = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                        self.expect(Token::Colon);
                        let val = self.parse_expr();
                        fields.insert(key, val);
                        if matches!(self.peek(), Token::Comma) { self.advance(); }
                    }
                    self.expect(Token::RBrace);
                    Stmt::MaterialDef(name, fields) },
                "layer" => { self.advance();
                    let name = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                    self.expect(Token::LBrace);
                    let mut body = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) { body.push(self.parse_stmt()); }
                    self.expect(Token::RBrace);
                    Stmt::LayerDef(name, body) },
                "field" => { self.advance();
                    let name = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                    self.expect(Token::LParen);
                    let mut params = Vec::new();
                    if !matches!(self.peek(), Token::RParen) {
                        if let Token::Ident(s)=self.advance() { params.push(s); } else { panic!() };
                        while matches!(self.peek(), Token::Comma) {
                            self.advance();
                            if let Token::Ident(s)=self.advance() { params.push(s); } else { panic!() }
                        }
                    }
                    self.expect(Token::RParen);
                    self.expect(Token::LBrace);
                    let mut body = Vec::new();
                    while !matches!(self.peek(), Token::RBrace) { body.push(self.parse_stmt()); }
                    self.expect(Token::RBrace);
                    Stmt::FieldDef(name, params, body) },
                _ => {
                    if let Token::Ident(name) = self.peek() {
                        let name = name.clone(); self.advance();
                        if let Token::Op(ref op) = self.peek() {
                            if op == "=" {
                                self.advance();
                                return Stmt::Assign(name, self.parse_expr());
                            }
                        }
                        if matches!(self.peek(), Token::LBracket) {
                            let base = Expr::Ident(name);
                            self.advance();
                            let idx = self.parse_expr();
                            self.expect(Token::RBracket);
                            self.expect(Token::Op("=".to_string()));
                            let expr = self.parse_expr();
                            return Stmt::IndexAssign(base, idx, expr);
                        }
                        if matches!(self.peek(), Token::Dot) {
                            self.advance();
                            let field = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                            self.expect(Token::Op("=".to_string()));
                            let expr = self.parse_expr();
                            return Stmt::FieldAssign(Expr::Ident(name), field, expr);
                        }
                        let mut expr = Expr::Ident(name.clone());
                        loop {
                            match self.peek() {
                                Token::LParen => {
                                    self.advance();
                                    let mut args = Vec::new();
                                    let mut kwargs = HashMap::new();
                                    if !matches!(self.peek(), Token::RParen) {
                                        let is_kwarg = matches!(self.peek(), Token::Ident(_))
                                            && self.pos + 1 < self.tokens.len()
                                            && matches!(&self.tokens[self.pos + 1], Token::Colon);
                                        if is_kwarg {
                                            while !matches!(self.peek(), Token::RParen) {
                                                let key = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                                                self.expect(Token::Colon);
                                                let val = self.parse_expr();
                                                kwargs.insert(key, val);
                                                if matches!(self.peek(), Token::Comma) { self.advance(); }
                                            }
                                        } else {
                                            args.push(self.parse_expr());
                                            while matches!(self.peek(), Token::Comma) {
                                                self.advance();
                                                args.push(self.parse_expr());
                                            }
                                        }
                                    }
                                    self.expect(Token::RParen);
                                    expr = Expr::Call(name.clone(), args, kwargs);
                                },
                                Token::LBracket => {
                                    self.advance();
                                    let idx = self.parse_expr();
                                    self.expect(Token::RBracket);
                                    expr = Expr::Index(Box::new(expr), Box::new(idx));
                                },
                                Token::Dot => {
                                    self.advance();
                                    let field = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                                    expr = Expr::FieldAccess(Box::new(expr), field);
                                },
                                _ => break,
                            }
                        }
                        return Stmt::ExprStmt(expr);
                    }
                    Stmt::ExprStmt(self.parse_expr())
                }
            },
            _ => {
                if let Token::Ident(name) = self.peek() {
                    let name = name.clone(); self.advance();
                    if let Token::Op(ref op) = self.peek() {
                        if op == "=" {
                            self.advance();
                            return Stmt::Assign(name, self.parse_expr());
                        }
                    }
                    if matches!(self.peek(), Token::LBracket) {
                        let base = Expr::Ident(name);
                        self.advance();
                        let idx = self.parse_expr();
                        self.expect(Token::RBracket);
                        self.expect(Token::Op("=".to_string()));
                        let expr = self.parse_expr();
                        return Stmt::IndexAssign(base, idx, expr);
                    }
                    if matches!(self.peek(), Token::Dot) {
                        self.advance();
                        let field = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                        self.expect(Token::Op("=".to_string()));
                        let expr = self.parse_expr();
                        return Stmt::FieldAssign(Expr::Ident(name), field, expr);
                    }
                    let mut expr = Expr::Ident(name.clone());
                    loop {
                        match self.peek() {
                            Token::LParen => {
                                self.advance();
                                let mut args = Vec::new();
                                let mut kwargs = HashMap::new();
                                if !matches!(self.peek(), Token::RParen) {
                                    let is_kwarg = matches!(self.peek(), Token::Ident(_))
                                        && self.pos + 1 < self.tokens.len()
                                        && matches!(&self.tokens[self.pos + 1], Token::Colon);
                                    if is_kwarg {
                                        while !matches!(self.peek(), Token::RParen) {
                                            let key = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                                            self.expect(Token::Colon);
                                            let val = self.parse_expr();
                                            kwargs.insert(key, val);
                                            if matches!(self.peek(), Token::Comma) { self.advance(); }
                                        }
                                    } else {
                                        args.push(self.parse_expr());
                                        while matches!(self.peek(), Token::Comma) {
                                            self.advance();
                                            args.push(self.parse_expr());
                                        }
                                    }
                                }
                                self.expect(Token::RParen);
                                expr = Expr::Call(name.clone(), args, kwargs);
                            },
                            Token::LBracket => {
                                self.advance();
                                let idx = self.parse_expr();
                                self.expect(Token::RBracket);
                                expr = Expr::Index(Box::new(expr), Box::new(idx));
                            },
                            Token::Dot => {
                                self.advance();
                                let field = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                                expr = Expr::FieldAccess(Box::new(expr), field);
                            },
                            _ => break,
                        }
                    }
                    return Stmt::ExprStmt(expr);
                }
                Stmt::ExprStmt(self.parse_expr())
            }
        }
    }

    fn parse_expr(&mut self) -> Expr { self.parse_or() }
    fn parse_or(&mut self) -> Expr {
        let mut left = self.parse_and();
        while let Token::Keyword(ref kw) = self.peek() { if kw=="or" { self.advance(); let right = self.parse_and(); left = Expr::LogicOp("or".into(), Box::new(left), Box::new(right)); } else { break; } }
        left
    }
    fn parse_and(&mut self) -> Expr {
        let mut left = self.parse_compare();
        while let Token::Keyword(ref kw) = self.peek() { if kw=="and" { self.advance(); let right = self.parse_compare(); left = Expr::LogicOp("and".into(), Box::new(left), Box::new(right)); } else { break; } }
        left
    }
    fn parse_compare(&mut self) -> Expr {
        let left = self.parse_add();
        if let Token::Op(ref op) = self.peek() {
            if ["<",">","<=",">=","==","!="].contains(&op.as_str()) {
                let op = self.advance();
                let right = self.parse_add();
                if let Token::Op(opstr) = op { return Expr::BinOp(opstr, Box::new(left), Box::new(right)); }
            }
        }
        left
    }
    fn parse_add(&mut self) -> Expr {
        let mut left = self.parse_mul();
        while let Token::Op(ref op) = self.peek() { if op=="+"||op=="-" { let op = self.advance(); let right = self.parse_mul(); if let Token::Op(opstr)=op { left = Expr::BinOp(opstr, Box::new(left), Box::new(right)); } } else { break; } }
        left
    }
    fn parse_mul(&mut self) -> Expr {
        let mut left = self.parse_unary();
        while let Token::Op(ref op) = self.peek() { if op=="*"||op=="/" { let op = self.advance(); let right = self.parse_unary(); if let Token::Op(opstr)=op { left = Expr::BinOp(opstr, Box::new(left), Box::new(right)); } } else { break; } }
        left
    }
    fn parse_unary(&mut self) -> Expr {
        if let Token::Op(ref op) = self.peek() { if op=="-" { self.advance(); return Expr::BinOp("-".into(), Box::new(Expr::Number(0.0)), Box::new(self.parse_unary())); } }
        if let Token::Keyword(ref kw) = self.peek() { if kw=="not" { self.advance(); return Expr::UnaryNot(Box::new(self.parse_unary())); } }
        self.parse_primary()
    }
    fn parse_primary(&mut self) -> Expr {
        let tok = self.advance();
        let expr = match tok {
            Token::Number(n) => Expr::Number(n),
            Token::String(s) => Expr::String(s),
            Token::Color(r,g,b) => Expr::Color(r,g,b),
            Token::Keyword(ref kw) if kw=="true" => Expr::Bool(true),
            Token::Keyword(ref kw) if kw=="false" => Expr::Bool(false),
            Token::Ident(s) => Expr::Ident(s),
            Token::LParen => {
                let inner = self.parse_expr();
                if matches!(self.peek(), Token::Comma) {
                    self.advance(); let mut elems = vec![inner];
                    while !matches!(self.peek(), Token::RParen) {
                        elems.push(self.parse_expr());
                        if matches!(self.peek(), Token::Comma) { self.advance(); }
                    }
                    self.expect(Token::RParen); Expr::Tuple(elems)
                } else { self.expect(Token::RParen); inner }
            },
            Token::LBracket => {
                let mut elems = Vec::new();
                while !matches!(self.peek(), Token::RBracket) {
                    elems.push(self.parse_expr());
                    if matches!(self.peek(), Token::Comma) { self.advance(); }
                }
                self.expect(Token::RBracket); Expr::Array(elems)
            },
            _ => panic!("意外的标记: {:?}", tok),
        };
        let mut expr = expr;
        loop {
            match self.peek() {
                Token::LBracket => {
                    self.advance(); let idx = self.parse_expr(); self.expect(Token::RBracket);
                    expr = Expr::Index(Box::new(expr), Box::new(idx));
                },
                Token::Dot => {
                    self.advance(); let field = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                    expr = Expr::FieldAccess(Box::new(expr), field);
                },
                Token::LParen => {
                    if let Expr::Ident(name) = expr {
                        self.advance(); let mut args = Vec::new(); let mut kwargs = HashMap::new();
                        if !matches!(self.peek(), Token::RParen) {
                            let is_kwarg = matches!(self.peek(), Token::Ident(_))
                                && self.pos + 1 < self.tokens.len()
                                && matches!(&self.tokens[self.pos + 1], Token::Colon);
                            if is_kwarg {
                                while !matches!(self.peek(), Token::RParen) {
                                    let key = if let Token::Ident(s)=self.advance() { s } else { panic!() };
                                    self.expect(Token::Colon);
                                    let val = self.parse_expr();
                                    kwargs.insert(key, val);
                                    if matches!(self.peek(), Token::Comma) { self.advance(); }
                                }
                            } else {
                                args.push(self.parse_expr());
                                while matches!(self.peek(), Token::Comma) {
                                    self.advance();
                                    args.push(self.parse_expr());
                                }
                            }
                        }
                        self.expect(Token::RParen);
                        expr = Expr::Call(name, args, kwargs);
                    } else { panic!("只有标识符可调用"); }
                },
                _ => break,
            }
        }
        expr
    }
}

// ---------- 运行时环境 ----------
#[derive(Clone, Debug)]
enum Value {
    Number(f64),
    Bool(bool),
    String(String),
    Color(u8,u8,u8),
    Tuple(Vec<Value>),
    Array(Rc<RefCell<Vec<Value>>>),
    Dict(Rc<RefCell<HashMap<String, Value>>>),
    Struct(Rc<RefCell<HashMap<String, Value>>>),
    Path(String, Vec<Value>),
    Closure(String, Vec<String>, Vec<Stmt>, Rc<RefCell<Env>>),
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
            (Value::Color(r1,g1,b1), Value::Color(r2,g2,b2)) => r1==r2 && g1==g2 && b1==b2,
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
            Value::Color(r,g,b) => { r.hash(state); g.hash(state); b.hash(state); }
            Value::Tuple(t) => t.hash(state),
            _ => 0.hash(state),
        }
    }
}

#[derive(Clone, Debug)]
struct Env {
    vars: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Env>>>,
}
impl Env {
    fn new(parent: Option<Rc<RefCell<Env>>>) -> Self { Env { vars: HashMap::new(), parent } }
    fn get(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.vars.get(name) { return Some(v.clone()); }
        if let Some(ref p) = self.parent { return p.borrow().get(name); }
        None
    }
    fn contains(&self, name: &str) -> bool {
        if self.vars.contains_key(name) { true }
        else if let Some(ref p) = self.parent { p.borrow().contains(name) }
        else { false }
    }
    fn set(&mut self, name: &str, val: Value) {
        if self.vars.contains_key(name) {
            self.vars.insert(name.to_string(), val);
        } else if let Some(ref p) = self.parent {
            if p.borrow().contains(name) {
                p.borrow_mut().vars.insert(name.to_string(), val);
            } else {
                panic!("变量 {} 未定义", name);
            }
        } else {
            panic!("变量 {} 未定义", name);
        }
    }
}

// ---------- 绘图引擎 ----------
#[derive(Clone, Debug)]
struct Canvas {
    width: u32, height: u32, pixels: Vec<u8>, bg: (u8,u8,u8),
}
impl Canvas {
    fn new(w: u32, h: u32) -> Self { Canvas { width: w, height: h, pixels: vec![0; (w*h*3) as usize], bg: (0,0,0) } }
    fn put_pixel(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8) {
        if x>=0 && x<self.width as i32 && y>=0 && y<self.height as i32 {
            let idx = (y as u32 * self.width + x as u32) as usize * 3;
            self.pixels[idx]=r.min(255); self.pixels[idx+1]=g.min(255); self.pixels[idx+2]=b.min(255);
        }
    }
    fn put_pixel_aa(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8, alpha: f64) {
        if alpha<=0.0 || x<0 || x>=self.width as i32 || y<0 || y>=self.height as i32 { return; }
        let idx = (y as u32 * self.width + x as u32) as usize * 3;
        let a = alpha.min(1.0).max(0.0);
        self.pixels[idx] = clamp_u8(self.pixels[idx] as f64 * (1.0-a) + r as f64 * a);
        self.pixels[idx+1] = clamp_u8(self.pixels[idx+1] as f64 * (1.0-a) + g as f64 * a);
        self.pixels[idx+2] = clamp_u8(self.pixels[idx+2] as f64 * (1.0-a) + b as f64 * a);
    }
    fn draw_line(&mut self, x0:i32,y0:i32,x1:i32,y1:i32,width:f64,r:u8,g:u8,b:u8) {
        if width <= 1.0 { self.wu_line(x0,y0,x1,y1,r,g,b); }
        else { for (x,y) in self.bresenham_points(x0,y0,x1,y1) { self.brush(x,y,width as i32,r,g,b); } }
    }
    fn bresenham_points(&self, x0:i32,y0:i32,x1:i32,y1:i32) -> Vec<(i32,i32)> {
        let mut pts = Vec::new();
        let dx=(x1-x0).abs(); let dy=-(y1-y0).abs();
        let sx=if x0<x1 {1} else {-1}; let sy=if y0<y1 {1} else {-1};
        let mut err=dx+dy; let mut x=x0; let mut y=y0;
        loop { pts.push((x,y)); if x==x1 && y==y1 { break; } let e2=2*err; if e2>=dy { err+=dy; x+=sx; } if e2<=dx { err+=dx; y+=sy; } }
        pts
    }
    fn brush(&mut self, cx:i32, cy:i32, radius:i32, r:u8, g:u8, b:u8) {
        let rad = radius as f64 / 2.0; let r2 = rad*rad; let ri = (rad+1.0) as i32;
        for dy in -ri..=ri { for dx in -ri..=ri { if dx*dx+dy*dy <= r2 as i32 { self.put_pixel(cx+dx, cy+dy, r,g,b); } } }
    }
    fn wu_line(&mut self, x0:i32,y0:i32,x1:i32,y1:i32,r:u8,g:u8,b:u8) {
        fn ipart(x:f64)->i32 { x.floor() as i32 } fn fpart(x:f64)->f64 { x-x.floor() } fn rfpart(x:f64)->f64 { 1.0-fpart(x) }
        let (mut x0,mut y0,mut x1,mut y1) = (x0,y0,x1,y1);
        let steep = (y1-y0).abs() > (x1-x0).abs();
        if steep { std::mem::swap(&mut x0,&mut y0); std::mem::swap(&mut x1,&mut y1); }
        if x0 > x1 { std::mem::swap(&mut x0,&mut x1); std::mem::swap(&mut y0,&mut y1); }
        let dx = x1 - x0; let dy = y1 - y0; let grad = if dx != 0 { dy as f64 / dx as f64 } else { 1.0 };
        let xend = (x0 as f64).round() as i32; let yend = y0 as f64 + grad * (xend - x0) as f64; let xgap = rfpart(x0 as f64 + 0.5);
        let xpxl1 = xend; let ypxl1 = ipart(yend);
        if steep { self.put_pixel_aa(ypxl1, xpxl1, r,g,b, rfpart(yend)*xgap); self.put_pixel_aa(ypxl1+1, xpxl1, r,g,b, fpart(yend)*xgap); }
        else { self.put_pixel_aa(xpxl1, ypxl1, r,g,b, rfpart(yend)*xgap); self.put_pixel_aa(xpxl1, ypxl1+1, r,g,b, fpart(yend)*xgap); }
        let mut intery = yend + grad;
        let xend2 = (x1 as f64).round() as i32; let yend2 = y1 as f64 + grad * (xend2 - x1) as f64; let xgap2 = fpart(x1 as f64 + 0.5);
        let xpxl2 = xend2; let ypxl2 = ipart(yend2);
        if steep { self.put_pixel_aa(ypxl2, xpxl2, r,g,b, rfpart(yend2)*xgap2); self.put_pixel_aa(ypxl2+1, xpxl2, r,g,b, fpart(yend2)*xgap2); }
        else { self.put_pixel_aa(xpxl2, ypxl2, r,g,b, rfpart(yend2)*xgap2); self.put_pixel_aa(xpxl2, ypxl2+1, r,g,b, fpart(yend2)*xgap2); }
        for x in (xpxl1+1)..xpxl2 {
            if steep { self.put_pixel_aa(ipart(intery), x, r,g,b, rfpart(intery)); self.put_pixel_aa(ipart(intery)+1, x, r,g,b, fpart(intery)); }
            else { self.put_pixel_aa(x, ipart(intery), r,g,b, rfpart(intery)); self.put_pixel_aa(x, ipart(intery)+1, r,g,b, fpart(intery)); }
            intery += grad;
        }
    }
    fn draw_circle(&mut self, cx:i32, cy:i32, radius:i32, width:f64, r:u8, g:u8, b:u8) {
        let mut x=radius; let mut y=0; let mut err=0;
        while x >= y {
            for (px,py) in &[(cx+x,cy+y),(cx+y,cy+x),(cx-y,cy+x),(cx-x,cy+y),(cx-x,cy-y),(cx-y,cy-x),(cx+y,cy-x),(cx+x,cy-y)] {
                if width <= 1.0 { self.put_pixel(*px,*py,r,g,b); } else { self.brush(*px,*py,width as i32,r,g,b); }
            }
            y += 1; if err <= 0 { err += 2*y + 1; } if err > 0 { x -= 1; err -= 2*x + 1; }
        }
    }
    fn sample_bezier3(&self, p1:(f64,f64),p2:(f64,f64),p3:(f64,f64),p4:(f64,f64),n:usize) -> Vec<(i32,i32)> {
        let mut pts = Vec::new();
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let q0 = (p1.0 + (p2.0-p1.0)*t, p1.1 + (p2.1-p1.1)*t);
            let q1 = (p2.0 + (p3.0-p2.0)*t, p2.1 + (p3.1-p2.1)*t);
            let q2 = (p3.0 + (p4.0-p3.0)*t, p3.1 + (p4.1-p3.1)*t);
            let r0 = (q0.0 + (q1.0-q0.0)*t, q0.1 + (q1.1-q0.1)*t);
            let r1 = (q1.0 + (q2.0-q1.0)*t, q1.1 + (q2.1-q1.1)*t);
            let point = (r0.0 + (r1.0-r0.0)*t, r0.1 + (r1.1-r0.1)*t);
            pts.push((point.0.round() as i32, point.1.round() as i32));
        }
        pts
    }
    fn fill(&mut self, r:u8,g:u8,b:u8) {
        for i in (0..self.pixels.len()).step_by(3) { self.pixels[i]=r; self.pixels[i+1]=g; self.pixels[i+2]=b; }
    }
}

// ---------- 噪声实现 ----------
const PERM: [usize; 512] = {
    let mut p = [0; 512];
    let mut i = 0;
    while i < 256 { p[i] = i; p[i+256] = i; i += 1; }
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
    while j < 512 { p[j] = shuffle[j % 256]; j += 1; }
    p
};

fn fade(t: f64) -> f64 { t*t*t*(t*(t*6.0-15.0)+10.0) }
fn lerp(a:f64, b:f64, t:f64) -> f64 { a + t*(b-a) }
fn grad(hash: usize, x:f64, y:f64) -> f64 {
    let h = hash & 7;
    let u = if h < 4 { x } else { y };
    let v = if h < 4 { y } else { x };
    (if (h & 1) == 0 { u } else { -u }) + (if (h & 2) == 0 { v } else { -v })
}
fn perlin(x:f64, y:f64) -> f64 {
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
    let x1 = lerp(grad(aaa, xf, yf), grad(baa, xf-1.0, yf), u);
    let x2 = lerp(grad(aba, xf, yf-1.0), grad(bba, xf-1.0, yf-1.0), u);
    lerp(x1, x2, v)
}
fn worley(x:f64, y:f64) -> f64 {
    let cell_size = 32.0;
    let cx = (x / cell_size).floor() as i32;
    let cy = (y / cell_size).floor() as i32;
    let mut min_dist = 1e9;
    for dx in -1..=1 {
        for dy in -1..=1 {
            let ncx = cx + dx;
            let ncy = cy + dy;
            let mut h = (ncx * 374761393 + ncy * 668265263) as u64;
            h = (h ^ (h >> 13)) * 1274126177;
            h = h ^ (h >> 16);
            let px = ncx as f64 * cell_size + (h % cell_size as u64) as f64;
            let py = ncy as f64 * cell_size + ((h >> 8) % cell_size as u64) as f64;
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if d < min_dist { min_dist = d; }
        }
    }
    min_dist
}
fn fbm(x:f64, y:f64, octaves:i32) -> f64 {
    let mut total = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut norm = 0.0;
    let oct = octaves.max(1).min(8);
    for _ in 0..oct {
        total += perlin(x*freq, y*freq) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    total / norm
}

// ---------- 解释器 ----------
struct Interpreter {
    canvas: Option<Canvas>,
    builtins: HashMap<String, fn(Vec<Value>) -> Value>,
    layers: HashMap<String, Value>,
    struct_defs: HashMap<String, (Vec<String>, Vec<Value>)>,
    imported: Vec<String>,
    rng: StdRng,
    current_dir: String,
}

impl Interpreter {
    fn new() -> Self {
        let mut interp = Interpreter {
            canvas: None,
            builtins: HashMap::new(),
            layers: HashMap::new(),
            struct_defs: HashMap::new(),
            imported: Vec::new(),
            rng: StdRng::from_entropy(),
            current_dir: ".".to_string(),
        };
        let builtins: Vec<(&str, fn(Vec<Value>) -> Value)> = vec![
            ("rand", |args| {
                let a = args[0].as_number().unwrap();
                let b = args[1].as_number().unwrap();
                Value::Number(rand::thread_rng().gen_range(a.min(b)..a.max(b)))
            }),
            ("int", |args| { let n=args[0].as_number().unwrap(); Value::Number(n.floor()) }),
            ("abs", |args| { let n=args[0].as_number().unwrap(); Value::Number(n.abs()) }),
            ("floor", |args| { let n=args[0].as_number().unwrap(); Value::Number(n.floor()) }),
            ("ceil", |args| { let n=args[0].as_number().unwrap(); Value::Number(n.ceil()) }),
            ("sin", |args| { let n=args[0].as_number().unwrap(); Value::Number(n.sin()) }),
            ("cos", |args| { let n=args[0].as_number().unwrap(); Value::Number(n.cos()) }),
            ("sqrt", |args| { let n=args[0].as_number().unwrap(); Value::Number(n.sqrt()) }),
            ("pow", |args| { let a=args[0].as_number().unwrap(); let b=args[1].as_number().unwrap(); Value::Number(a.powf(b)) }),
            ("min", |args| { let a=args[0].as_number().unwrap(); let b=args[1].as_number().unwrap(); Value::Number(a.min(b)) }),
            ("max", |args| { let a=args[0].as_number().unwrap(); let b=args[1].as_number().unwrap(); Value::Number(a.max(b)) }),
            ("bool", |args| { match &args[0] {
                Value::Number(n) => Value::Bool(*n != 0.0),
                Value::Bool(b) => Value::Bool(*b),
                _ => Value::Bool(true)
            }}),
            ("len", |args| {
                let n = match &args[0] {
                    Value::Tuple(t) => t.len(),
                    Value::Array(a) => a.borrow().len(),
                    Value::Dict(d) => d.borrow().len(),
                    Value::String(s) => s.len(),
                    _ => 0,
                };
                Value::Number(n as f64)
            }),
            ("push", |args| {
                if let Value::Array(arr) = &args[0] {
                    arr.borrow_mut().push(args[1].clone());
                }
                Value::None
            }),
            ("pop", |args| {
                if let Value::Array(arr) = &args[0] {
                    arr.borrow_mut().pop().unwrap_or(Value::None)
                } else { Value::None }
            }),
            ("array", |args| {
                if args.is_empty() {
                    Value::Array(Rc::new(RefCell::new(Vec::new())))
                } else if args.len() == 1 {
                    match &args[0] {
                        Value::Number(n) => Value::Array(Rc::new(RefCell::new(vec![Value::None; *n as usize]))),
                        _ => Value::Array(Rc::new(RefCell::new(args.clone())))
                    }
                } else {
                    Value::Array(Rc::new(RefCell::new(args.clone())))
                }
            }),
            ("dict", |args| {
                let mut d: HashMap<String, Value> = HashMap::new();
                for i in (0..args.len()).step_by(2) {
                    if i+1 < args.len() {
                        let key = args[i].as_string().unwrap_or_else(|| format!("{:?}", args[i]));
                        d.insert(key, args[i+1].clone());
                    }
                }
                Value::Dict(Rc::new(RefCell::new(d)))
            }),
            ("keys", |args| {
                if let Value::Dict(d) = &args[0] {
                    let keys: Vec<Value> = d.borrow().keys().map(|k| Value::String(k.clone())).collect();
                    Value::Array(Rc::new(RefCell::new(keys)))
                } else { Value::Array(Rc::new(RefCell::new(Vec::new()))) }
            }),
            ("values", |args| {
                if let Value::Dict(d) = &args[0] {
                    let vals: Vec<Value> = d.borrow().values().cloned().collect();
                    Value::Array(Rc::new(RefCell::new(vals)))
                } else { Value::Array(Rc::new(RefCell::new(Vec::new()))) }
            }),
            ("has", |args| {
                if let Value::Dict(d) = &args[0] {
                    let key = args[1].as_string().unwrap_or_default();
                    Value::Bool(d.borrow().contains_key(&key))
                } else { Value::Bool(false) }
            }),
            ("line", |args| {
                let p1 = args[0].as_tuple().unwrap();
                let p2 = args[1].as_tuple().unwrap();
                Value::Path("line".into(), vec![Value::Tuple(p1), Value::Tuple(p2)])
            }),
            ("circle", |args| {
                let cx = args[0].as_number().unwrap() as i32;
                let cy = args[1].as_number().unwrap() as i32;
                let r = args[2].as_number().unwrap() as i32;
                Value::Path("circle".into(), vec![Value::Number(cx as f64), Value::Number(cy as f64), Value::Number(r as f64)])
            }),
            ("bezier", |args| {
                let p1 = args[0].as_tuple().unwrap();
                let p2 = args[1].as_tuple().unwrap();
                let p3 = args[2].as_tuple().unwrap();
                let p4 = args[3].as_tuple().unwrap();
                Value::Path("bezier".into(), vec![Value::Tuple(p1), Value::Tuple(p2), Value::Tuple(p3), Value::Tuple(p4)])
            }),
            ("qbezier", |args| {
                let p1 = args[0].as_tuple().unwrap();
                let p2 = args[1].as_tuple().unwrap();
                let p3 = args[2].as_tuple().unwrap();
                Value::Path("qbezier".into(), vec![Value::Tuple(p1), Value::Tuple(p2), Value::Tuple(p3)])
            }),
            ("path", |args| {
                if let Value::Array(arr) = &args[0] {
                    let pts = arr.borrow().clone();
                    Value::Path("polyline".into(), pts)
                } else { Value::None }
            }),
            ("dot", |args| {
                let a = args[0].as_tuple().unwrap();
                let b = args[1].as_tuple().unwrap();
                let sum = a.iter().zip(b.iter()).map(|(x,y)| x.as_number().unwrap() * y.as_number().unwrap()).sum();
                Value::Number(sum)
            }),
            ("length", |args| {
                let p1 = args[0].as_tuple().unwrap();
                let p2 = args[1].as_tuple().unwrap();
                let d = (p1[0].as_number().unwrap() - p2[0].as_number().unwrap()).powi(2) +
                        (p1[1].as_number().unwrap() - p2[1].as_number().unwrap()).powi(2);
                Value::Number(d.sqrt())
            }),
            ("perlin", |args| {
                let x=args[0].as_number().unwrap();
                let y=args[1].as_number().unwrap();
                Value::Number(perlin(x,y))
            }),
            ("worley", |args| {
                let x=args[0].as_number().unwrap();
                let y=args[1].as_number().unwrap();
                Value::Number(worley(x,y))
            }),
            ("fbm", |args| {
                let x=args[0].as_number().unwrap();
                let y=args[1].as_number().unwrap();
                let o = args.get(2).map(|v| v.as_number().unwrap() as i32).unwrap_or(4);
                Value::Number(fbm(x,y,o))
            }),
        ];
        for (name, f) in builtins { interp.builtins.insert(name.to_string(), f); }
        interp
    }

    fn eval(&mut self, expr: &Expr, env: Rc<RefCell<Env>>) -> Value {
        match expr {
            Expr::Number(n) => Value::Number(*n),
            Expr::String(s) => Value::String(s.clone()),
            Expr::Color(r,g,b) => Value::Color(*r,*g,*b),
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Ident(name) => {
                if let Some(v) = env.borrow().get(name) { v }
                else if name=="true" { Value::Bool(true) }
                else if name=="false" { Value::Bool(false) }
                else { panic!("未定义变量: {}", name) }
            },
            Expr::Tuple(el) => Value::Tuple(el.iter().map(|e| self.eval(e, env.clone())).collect()),
            Expr::Array(el) => {
                let vals = el.iter().map(|e| self.eval(e, env.clone())).collect();
                Value::Array(Rc::new(RefCell::new(vals)))
            },
            Expr::BinOp(op, l, r) => {
                let lv = self.eval(l, env.clone());
                let rv = self.eval(r, env.clone());
                match (&lv, &rv) {
                    (Value::Number(a), Value::Number(b)) => {
                        let res = match op.as_str() {
                            "+" => a+b, "-" => a-b, "*" => a*b, "/" => a/b,
                            "<" => return Value::Bool(a < b),
                            ">" => return Value::Bool(a > b),
                            "<=" => return Value::Bool(a <= b),
                            ">=" => return Value::Bool(a >= b),
                            "==" => return Value::Bool(a == b),
                            "!=" => return Value::Bool(a != b),
                            _ => panic!("未知运算符"),
                        };
                        Value::Number(res)
                    },
                    (Value::Tuple(a), Value::Tuple(b)) => {
                        if a.len() != b.len() { panic!("元组长度不匹配"); }
                        let res = match op.as_str() {
                            "+" => a.iter().zip(b.iter()).map(|(x,y)| Value::Number(x.as_number().unwrap() + y.as_number().unwrap())).collect(),
                            "-" => a.iter().zip(b.iter()).map(|(x,y)| Value::Number(x.as_number().unwrap() - y.as_number().unwrap())).collect(),
                            _ => panic!("只支持 +/- 元组广播"),
                        };
                        Value::Tuple(res)
                    },
                    (Value::Tuple(t), Value::Number(n)) if op=="*" => {
                        let res = t.iter().map(|v| Value::Number(v.as_number().unwrap() * n)).collect();
                        Value::Tuple(res)
                    },
                    (Value::Number(n), Value::Tuple(t)) if op=="*" => {
                        let res = t.iter().map(|v| Value::Number(v.as_number().unwrap() * n)).collect();
                        Value::Tuple(res)
                    },
                    (Value::Tuple(t), Value::Number(n)) if op=="/" => {
                        let res = t.iter().map(|v| Value::Number(v.as_number().unwrap() / n)).collect();
                        Value::Tuple(res)
                    },
                    _ => panic!("类型不匹配: {:?} {:?} {:?}", lv, op, rv),
                }
            },
            Expr::LogicOp(op, l, r) => {
                let lv = self.eval(l, env.clone());
                let lb = match lv { Value::Bool(b) => b, _ => panic!("逻辑运算需要 bool") };
                if op=="and" {
                    if !lb { return Value::Bool(false); }
                    let rv = self.eval(r, env.clone());
                    match rv { Value::Bool(b) => Value::Bool(b), _ => panic!() }
                } else {
                    if lb { return Value::Bool(true); }
                    let rv = self.eval(r, env.clone());
                    match rv { Value::Bool(b) => Value::Bool(b), _ => panic!() }
                }
            },
            Expr::UnaryNot(e) => {
                let v = self.eval(e, env.clone());
                match v { Value::Bool(b) => Value::Bool(!b), _ => panic!("not 作用于非 bool") }
            },
            Expr::Call(name, args, kwargs) => {
                if name == "compose" {
                    let layer_name = self.eval(&args[0], env.clone()).as_string().unwrap();
                    let blend = if args.len() > 1 { self.eval(&args[1], env.clone()).as_string().unwrap() } else { "over".to_string() };
                    self.compose_layer(&layer_name, &blend);
                    return Value::None;
                }
                if name == "fill" {
                    let field_name = self.eval(&args[0], env.clone()).as_string().unwrap();
                    self.fill_field(&field_name, env.clone());
                    return Value::None;
                }
                if self.struct_defs.contains_key(name) {
                    return self.construct_struct(name, args, kwargs, env.clone());
                }
                let arg_vals: Vec<Value> = args.iter().map(|a| self.eval(a, env.clone())).collect();
                if self.builtins.contains_key(name) {
                    return self.builtins[name](arg_vals);
                }
                if let Some(Value::Closure(_, params, body, closure_env)) = env.borrow().get(name) {
                    let new_env = Rc::new(RefCell::new(Env::new(Some(closure_env.clone()))));
                    for (i, p) in params.iter().enumerate() {
                        if i < args.len() {
                            new_env.borrow_mut().vars.insert(p.clone(), arg_vals[i].clone());
                        }
                    }
                    for (k, v) in kwargs {
                        new_env.borrow_mut().vars.insert(k.clone(), self.eval(v, env.clone()));
                    }
                    let result = self.execute_block(&body, new_env);
                    match result {
                        Ok(_) => Value::None,
                        Err(Control::Return(v)) => v,
                        _ => Value::None,
                    }
                } else {
                    panic!("未定义函数: {}", name);
                }
            },
            Expr::Index(base, idx) => {
                let base_val = self.eval(base, env.clone());
                let idx_val = self.eval(idx, env.clone());
                match base_val {
                    Value::Tuple(t) => {
                        let i = idx_val.as_number().unwrap() as usize;
                        if i < t.len() { t[i].clone() } else { panic!("索引越界") }
                    },
                    Value::Array(arr) => {
                        let i = idx_val.as_number().unwrap() as usize;
                        let arr_ref = arr.borrow();
                        if i < arr_ref.len() { arr_ref[i].clone() } else { panic!("索引越界") }
                    },
                    Value::Dict(d) => {
                        let key = idx_val.as_string().unwrap();
                        let d_ref = d.borrow();
                        d_ref.get(&key).cloned().unwrap_or_else(|| panic!("键不存在"))
                    },
                    _ => panic!("不支持索引"),
                }
            },
            Expr::FieldAccess(obj, field) => {
                let obj_val = self.eval(obj, env.clone());
                if let Value::Struct(s) = obj_val {
                    let s_ref = s.borrow();
                    s_ref.get(field).cloned().unwrap_or_else(|| panic!("字段不存在"))
                } else {
                    panic!("不是结构体")
                }
            },
        }
    }

    fn exec(&mut self, stmt: &Stmt, env: Rc<RefCell<Env>>) -> Result<(), Control> {
        match stmt {
            Stmt::Canvas(w,h) => {
                self.canvas = Some(Canvas::new(*w,*h));
                Ok(())
            },
            Stmt::Bg(expr) => {
                let val = self.eval(expr, env.clone());
                let (r,g,b) = match val {
                    Value::Color(r,g,b) => (r,g,b),
                    Value::Tuple(t) => {
                        if t.len()>=3 {
                            (clamp_u8(t[0].as_number().unwrap()),
                             clamp_u8(t[1].as_number().unwrap()),
                             clamp_u8(t[2].as_number().unwrap()))
                        } else { panic!("bg 需要颜色") }
                    },
                    _ => panic!("bg 需要颜色"),
                };
                if let Some(canvas) = &mut self.canvas {
                    canvas.fill(r,g,b);
                    canvas.bg = (r,g,b);
                }
                Ok(())
            },
            Stmt::Let(name, expr) => {
                let val = self.eval(expr, env.clone());
                env.borrow_mut().vars.insert(name.clone(), val);
                Ok(())
            },
            Stmt::Assign(name, expr) => {
                let val = self.eval(expr, env.clone());
                env.borrow_mut().set(name, val);
                Ok(())
            },
            Stmt::For(var, start, end, body) => {
                let start_val = self.eval(start, env.clone());
                let end_val = self.eval(end, env.clone());
                let mut i = start_val.as_number().unwrap();
                let end = end_val.as_number().unwrap();
                while i < end {
                    let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    block_env.borrow_mut().vars.insert(var.clone(), Value::Number(i));
                    match self.execute_block(body, block_env) {
                        Ok(_) => {},
                        Err(Control::Break) => break,
                        Err(Control::Continue) => {},
                        Err(Control::Return(v)) => return Err(Control::Return(v)),
                    }
                    i += 1.0;
                }
                Ok(())
            },
            Stmt::While(cond, body) => {
                while {
                    let cond_val = self.eval(cond, env.clone());
                    match cond_val {
                        Value::Bool(b) => b,
                        Value::Number(n) => n != 0.0,
                        _ => panic!("条件需要 bool"),
                    }
                } {
                    let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    match self.execute_block(body, block_env) {
                        Ok(_) => {},
                        Err(Control::Break) => break,
                        Err(Control::Continue) => {},
                        Err(Control::Return(v)) => return Err(Control::Return(v)),
                    }
                }
                Ok(())
            },
            Stmt::If(cond, then_body, else_body) => {
                let cond_val = self.eval(cond, env.clone());
                let b = match cond_val {
                    Value::Bool(b) => b,
                    Value::Number(n) => n != 0.0,
                    _ => panic!("条件需要 bool"),
                };
                if b {
                    let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    self.execute_block(then_body, block_env)
                } else if let Some(else_body) = else_body {
                    let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    self.execute_block(else_body, block_env)
                } else {
                    Ok(())
                }
            },
            Stmt::Break => Err(Control::Break),
            Stmt::Continue => Err(Control::Continue),
            Stmt::Seed(n) => {
                self.rng = StdRng::seed_from_u64(*n);
                Ok(())
            },
            Stmt::FnDef(name, params, body) => {
                let closure = Value::Closure(name.clone(), params.clone(), body.clone(), env.clone());
                env.borrow_mut().vars.insert(name.clone(), closure);
                Ok(())
            },
            Stmt::Return(expr) => {
                let val = self.eval(expr, env.clone());
                Err(Control::Return(val))
            },
            Stmt::Pixel(x_expr, y_expr, rgb_expr) => {
                let x = self.eval(x_expr, env.clone()).as_number().unwrap() as i32;
                let y = self.eval(y_expr, env.clone()).as_number().unwrap() as i32;
                let rgb_val = self.eval(rgb_expr, env.clone());
                let (r,g,b) = match rgb_val {
                    Value::Color(r,g,b) => (r,g,b),
                    Value::Tuple(t) => {
                        (clamp_u8(t[0].as_number().unwrap()),
                         clamp_u8(t[1].as_number().unwrap()),
                         clamp_u8(t[2].as_number().unwrap()))
                    },
                    _ => panic!("rgb 需要颜色或三元组"),
                };
                if let Some(canvas) = &mut self.canvas {
                    canvas.put_pixel(x,y,r,g,b);
                }
                Ok(())
            },
            Stmt::Stroke(fields) => {
                let path_val = self.eval(fields.get("path").unwrap_or(&Expr::Ident("none".into())), env.clone());
                let width = self.eval(fields.get("width").unwrap_or(&Expr::Number(1.0)), env.clone()).as_number().unwrap();
                let color_val = if let Some(mat_expr) = fields.get("material") {
                    let mat_val = self.eval(mat_expr, env.clone());
                    if let Value::Material(mat_map) = mat_val {
                        let mut color = mat_map.get("color").cloned().unwrap_or(Value::Color(0,0,0));
                        if let Some(noise) = mat_map.get("noise") {
                            let nv = noise.as_number().unwrap_or(0.0);
                            if nv != 0.0 {
                                let nx = width * 10.0;
                                let ny = 0.0;
                                let noise_val = perlin(nx, ny) * nv;
                                if let Value::Color(r,g,b) = color {
                                    let r2 = clamp_u8(r as f64 + noise_val * 255.0);
                                    let g2 = clamp_u8(g as f64 + noise_val * 255.0);
                                    let b2 = clamp_u8(b as f64 + noise_val * 255.0);
                                    color = Value::Color(r2,g2,b2);
                                }
                            }
                        }
                        color
                    } else { panic!("material 必须是材质类型") }
                } else {
                    self.eval(fields.get("color").unwrap_or(&Expr::Color(0,0,0)), env.clone())
                };
                let (r,g,b) = match color_val {
                    Value::Color(r,g,b) => (r,g,b),
                    Value::Tuple(t) => {
                        (clamp_u8(t[0].as_number().unwrap()),
                         clamp_u8(t[1].as_number().unwrap()),
                         clamp_u8(t[2].as_number().unwrap()))
                    },
                    _ => panic!("color 错误"),
                };
                if let Some(canvas) = &mut self.canvas {
                    match path_val {
                        Value::Path(tag, args) => {
                            match tag.as_str() {
                                "line" => {
                                    let p1 = args[0].as_tuple().unwrap();
                                    let p2 = args[1].as_tuple().unwrap();
                                    canvas.draw_line(p1[0].as_number().unwrap() as i32,
                                                     p1[1].as_number().unwrap() as i32,
                                                     p2[0].as_number().unwrap() as i32,
                                                     p2[1].as_number().unwrap() as i32,
                                                     width, r,g,b);
                                },
                                "circle" => {
                                    let cx = args[0].as_number().unwrap() as i32;
                                    let cy = args[1].as_number().unwrap() as i32;
                                    let rad = args[2].as_number().unwrap() as i32;
                                    canvas.draw_circle(cx,cy,rad,width,r,g,b);
                                },
                                "bezier" => {
                                    let p1 = args[0].as_tuple().unwrap();
                                    let p2 = args[1].as_tuple().unwrap();
                                    let p3 = args[2].as_tuple().unwrap();
                                    let p4 = args[3].as_tuple().unwrap();
                                    let pts = canvas.sample_bezier3(
                                        (p1[0].as_number().unwrap(), p1[1].as_number().unwrap()),
                                        (p2[0].as_number().unwrap(), p2[1].as_number().unwrap()),
                                        (p3[0].as_number().unwrap(), p3[1].as_number().unwrap()),
                                        (p4[0].as_number().unwrap(), p4[1].as_number().unwrap()),
                                        32
                                    );
                                    for i in 0..pts.len()-1 {
                                        canvas.draw_line(pts[i].0, pts[i].1, pts[i+1].0, pts[i+1].1, width, r,g,b);
                                    }
                                },
                                "qbezier" => {
                                    let p1 = args[0].as_tuple().unwrap();
                                    let p2 = args[1].as_tuple().unwrap();
                                    let p3 = args[2].as_tuple().unwrap();
                                    let pts = canvas.sample_bezier3(
                                        (p1[0].as_number().unwrap(), p1[1].as_number().unwrap()),
                                        (p2[0].as_number().unwrap(), p2[1].as_number().unwrap()),
                                        (p3[0].as_number().unwrap(), p3[1].as_number().unwrap()),
                                        (p3[0].as_number().unwrap(), p3[1].as_number().unwrap()),
                                        32
                                    );
                                    for i in 0..pts.len()-1 {
                                        canvas.draw_line(pts[i].0, pts[i].1, pts[i+1].0, pts[i+1].1, width, r,g,b);
                                    }
                                },
                                "polyline" => {
                                    if args.len() > 0 {
                                        for i in 0..args.len()-1 {
                                            let p1 = args[i].as_tuple().unwrap();
                                            let p2 = args[i+1].as_tuple().unwrap();
                                            canvas.draw_line(p1[0].as_number().unwrap() as i32,
                                                             p1[1].as_number().unwrap() as i32,
                                                             p2[0].as_number().unwrap() as i32,
                                                             p2[1].as_number().unwrap() as i32,
                                                             width, r,g,b);
                                        }
                                    }
                                },
                                _ => {},
                            }
                        },
                        _ => panic!("path 不是路径"),
                    }
                }
                Ok(())
            },
            Stmt::Render(fname) => {
                if let Some(canvas) = &self.canvas {
                    let img = ImageBuffer::<Rgb<u8>, _>::from_vec(canvas.width, canvas.height, canvas.pixels.clone()).unwrap();
                    img.save(fname).unwrap();
                    println!("已渲染: {}", fname);
                }
                Ok(())
            },
            Stmt::StructDef(name, fields) => {
                let mut def_names = Vec::new();
                let mut def_vals = Vec::new();
                for (fname, expr) in fields {
                    def_names.push(fname.clone());
                    def_vals.push(self.eval(expr, env.clone()));
                }
                self.struct_defs.insert(name.clone(), (def_names, def_vals));
                Ok(())
            },
            Stmt::Import(path) => {
                let full_path = if path.starts_with('/') || path.starts_with('.') {
                    path.clone()
                } else {
                    format!("{}/{}", self.current_dir, path)
                };
                if self.imported.contains(&full_path) { return Ok(()); }
                self.imported.push(full_path.clone());
                let src = fs::read_to_string(&full_path).unwrap_or_else(|_| panic!("无法导入模块: {}", path));
                let mut lexer = Lexer::new(&src);
                let tokens = lexer.tokenize();
                let mut parser = Parser { tokens, pos: 0 };
                let ast = parser.parse_program();
                let old_dir = self.current_dir.clone();
                self.current_dir = std::path::Path::new(&full_path).parent().unwrap().to_str().unwrap().to_string();
                for s in ast {
                    self.exec(&s, env.clone())?;
                }
                self.current_dir = old_dir;
                Ok(())
            },
            Stmt::MaterialDef(name, fields) => {
                let mut map = HashMap::new();
                for (k, v) in fields {
                    map.insert(k.clone(), self.eval(v, env.clone()));
                }
                let mat = Value::Material(map);
                env.borrow_mut().vars.insert(name.clone(), mat);
                Ok(())
            },
            Stmt::LayerDef(name, body) => {
                if let Some(canvas) = &self.canvas {
                    let mut layer_canvas = Canvas::new(canvas.width, canvas.height);
                    layer_canvas.fill(canvas.bg.0, canvas.bg.1, canvas.bg.2);
                    let old_canvas = std::mem::replace(&mut self.canvas, Some(layer_canvas));
                    let block_env = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    let _ = self.execute_block(body, block_env);
                    let new_canvas = self.canvas.take().unwrap();
                    self.layers.insert(name.clone(), Value::Layer(Rc::new(RefCell::new(new_canvas))));
                    self.canvas = old_canvas;
                }
                Ok(())
            },
            Stmt::FieldDef(name, params, body) => {
                let closure = Value::Closure(name.clone(), params.clone(), body.clone(), env.clone());
                env.borrow_mut().vars.insert(name.clone(), closure);
                Ok(())
            },
            Stmt::IndexAssign(base, idx, expr) => {
                let base_val = self.eval(base, env.clone());
                let idx_val = self.eval(idx, env.clone());
                let val = self.eval(expr, env.clone());
                match base_val {
                    Value::Array(arr) => {
                        let i = idx_val.as_number().unwrap() as usize;
                        let mut arr_ref = arr.borrow_mut();
                        if i < arr_ref.len() {
                            arr_ref[i] = val;
                        } else { panic!("索引越界"); }
                    },
                    Value::Dict(d) => {
                        let key = idx_val.as_string().unwrap();
                        let mut d_ref = d.borrow_mut();
                        d_ref.insert(key, val);
                    },
                    Value::Struct(s) => {
                        let field = idx_val.as_string().unwrap();
                        let mut s_ref = s.borrow_mut();
                        s_ref.insert(field, val);
                    },
                    _ => panic!("索引赋值不支持该类型"),
                }
                Ok(())
            },
            Stmt::FieldAssign(obj, field, expr) => {
                let obj_val = self.eval(obj, env.clone());
                let val = self.eval(expr, env.clone());
                if let Value::Struct(s) = obj_val {
                    let mut s_ref = s.borrow_mut();
                    if s_ref.contains_key(field) {
                        s_ref.insert(field.clone(), val);
                    } else { panic!("字段不存在"); }
                } else { panic!("不是结构体"); }
                Ok(())
            },
            Stmt::ExprStmt(expr) => { self.eval(expr, env.clone()); Ok(()) },
        }
    }

    fn execute_block(&mut self, body: &[Stmt], env: Rc<RefCell<Env>>) -> Result<(), Control> {
        for stmt in body {
            self.exec(stmt, env.clone())?;
        }
        Ok(())
    }

    fn execute_block_return(&mut self, body: &[Stmt], env: Rc<RefCell<Env>>) -> Value {
        match self.execute_block(body, env) {
            Ok(_) => Value::None,
            Err(Control::Return(v)) => v,
            _ => Value::None,
        }
    }

    fn construct_struct(&mut self, name: &str, args: &[Expr], kwargs: &HashMap<String, Expr>, env: Rc<RefCell<Env>>) -> Value {
        let (field_names, default_vals) = self.struct_defs.get(name).unwrap().clone();
        let mut fields = HashMap::new();
        for (fname, dval) in field_names.iter().zip(default_vals.iter()) {
            fields.insert(fname.clone(), dval.clone());
        }
        for (i, arg_expr) in args.iter().enumerate() {
            if i < field_names.len() {
                let val = self.eval(arg_expr, env.clone());
                fields.insert(field_names[i].clone(), val);
            } else { panic!("参数过多"); }
        }
        for (k, v_expr) in kwargs {
            if fields.contains_key(k) {
                let val = self.eval(v_expr, env.clone());
                fields.insert(k.clone(), val);
            } else { panic!("未知字段: {}", k); }
        }
        Value::Struct(Rc::new(RefCell::new(fields)))
    }

    fn compose_layer(&mut self, name: &str, blend: &str) {
        if let Some(Value::Layer(layer_canvas)) = self.layers.get(name) {
            let layer = layer_canvas.borrow();
            let lw = layer.width;
            let lh = layer.height;
            let layer_data = &layer.pixels;
            if let Some(canvas) = &mut self.canvas {
                if canvas.width != lw || canvas.height != lh {
                    panic!("图层尺寸不匹配");
                }
                for y in 0..lh {
                    for x in 0..lw {
                        let idx = (y * lw + x) as usize * 3;
                        let mr = canvas.pixels[idx] as f64;
                        let mg = canvas.pixels[idx+1] as f64;
                        let mb = canvas.pixels[idx+2] as f64;
                        let lr = layer_data[idx] as f64;
                        let lg = layer_data[idx+1] as f64;
                        let lb = layer_data[idx+2] as f64;
                        let (nr, ng, nb) = match blend {
                            "add" => ( (mr + lr).min(255.0), (mg + lg).min(255.0), (mb + lb).min(255.0) ),
                            "mul" => ( (mr * lr / 255.0), (mg * lg / 255.0), (mb * lb / 255.0) ),
                            "screen" => ( 255.0 - (255.0-mr)*(255.0-lr)/255.0,
                                          255.0 - (255.0-mg)*(255.0-lg)/255.0,
                                          255.0 - (255.0-mb)*(255.0-lb)/255.0 ),
                            _ => {
                                let alpha = (lr + lg + lb) / (3.0 * 255.0);
                                ( mr*(1.0-alpha) + lr*alpha,
                                  mg*(1.0-alpha) + lg*alpha,
                                  mb*(1.0-alpha) + lb*alpha )
                            }
                        };
                        canvas.pixels[idx] = clamp_u8(nr);
                        canvas.pixels[idx+1] = clamp_u8(ng);
                        canvas.pixels[idx+2] = clamp_u8(nb);
                    }
                }
            }
        } else { panic!("未找到图层: {}", name); }
    }

    fn fill_field(&mut self, name: &str, env: Rc<RefCell<Env>>) {
        let closure = env.borrow().get(name);
        let (params, body, def_env) = match closure {
            Some(Value::Closure(_, p, b, e)) => (p, b, e),
            _ => panic!("未找到颜色场: {}", name),
        };
        let (w, h) = if let Some(c) = &self.canvas {
            (c.width, c.height)
        } else {
            return;
        };
        for y in 0..h {
            for x in 0..w {
                let call_env = Rc::new(RefCell::new(Env::new(Some(def_env.clone()))));
                if params.len() > 0 { call_env.borrow_mut().vars.insert(params[0].clone(), Value::Number(x as f64)); }
                if params.len() > 1 { call_env.borrow_mut().vars.insert(params[1].clone(), Value::Number(y as f64)); }
                let result = self.execute_block_return(&body, call_env);
                if let Some((r, g, b)) = match result {
                    Value::Color(r,g,b) => Some((r,g,b)),
                    Value::Tuple(t) if t.len() >= 3 => {
                        Some((clamp_u8(t[0].as_number().unwrap()),
                              clamp_u8(t[1].as_number().unwrap()),
                              clamp_u8(t[2].as_number().unwrap())))
                    },
                    _ => None,
                } {
                    if let Some(canvas) = &mut self.canvas {
                        canvas.put_pixel(x as i32, y as i32, r, g, b);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
enum Control {
    Break,
    Continue,
    Return(Value),
}

impl Value {
    fn as_number(&self) -> Option<f64> { if let Value::Number(n) = self { Some(*n) } else { None } }
    fn as_bool(&self) -> Option<bool> { if let Value::Bool(b) = self { Some(*b) } else { None } }
    fn as_tuple(&self) -> Option<Vec<Value>> { if let Value::Tuple(t) = self { Some(t.clone()) } else { None } }
    fn as_string(&self) -> Option<String> { if let Value::String(s) = self { Some(s.clone()) } else { None } }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { println!("用法: vgl_rs <file.vgl>"); return; }
    let src = fs::read_to_string(&args[1]).expect("无法读取文件");
    let mut lexer = Lexer::new(&src);
    let tokens = lexer.tokenize();
    let mut parser = Parser { tokens, pos: 0 };
    let ast = parser.parse_program();
    let mut interp = Interpreter::new();
    let path = std::path::Path::new(&args[1]);
    interp.current_dir = path.parent().unwrap().to_str().unwrap().to_string();
    let global_env = Rc::new(RefCell::new(Env::new(None)));
    for stmt in ast {
        if let Err(control) = interp.exec(&stmt, global_env.clone()) {
            match control {
                Control::Break | Control::Continue => panic!("控制信号泄漏到顶层"),
                Control::Return(v) => println!("顶层 return 值: {:?}", v),
            }
        }
    }
}
