// ============================================================
// 语法分析
// ============================================================

use std::collections::HashMap;

use crate::ast::{Expr, Stmt, StmtWithPos};
use crate::error::{VglError, VglResult};
use crate::lexer::{Token, TokenWithPos};

pub struct Parser {
    pub tokens: Vec<TokenWithPos>,
    pub pos: usize,
    pub loop_depth: i32, // 校验 break 必须在循环体内
}

impl Parser {
    pub fn new(tokens: Vec<TokenWithPos>) -> Self {
        Parser { tokens, pos: 0, loop_depth: 0 }
    }
    pub fn peek(&self) -> &Token {
        &self.tokens[self.pos].tok
    }
    pub fn peek_pos(&self) -> usize {
        self.tokens[self.pos].pos
    }
    pub fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].tok.clone();
        self.pos += 1;
        t
    }
    pub fn expect(&mut self, tok: &Token) -> VglResult<()> {
        if *self.peek() != *tok {
            return Err(VglError::new(
                format!("期望 {:?}, 得到 {:?}", tok, self.peek()),
                Some(self.peek_pos()),
            ));
        }
        self.advance();
        Ok(())
    }
    pub fn parse_program(&mut self) -> VglResult<Vec<StmtWithPos>> {
        let mut stmts = Vec::new();
        while !matches!(self.peek(), Token::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    pub fn parse_stmt(&mut self) -> VglResult<StmtWithPos> {
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

    pub fn _parse_stmt_impl(&mut self) -> VglResult<Stmt> {
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

    pub fn parse_param_list(&mut self) -> VglResult<Vec<String>> {
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

    pub fn parse_block_body(&mut self) -> VglResult<Vec<StmtWithPos>> {
        let mut body = Vec::new();
        while !matches!(self.peek(), Token::RBrace) {
            body.push(self.parse_stmt()?);
        }
        Ok(body)
    }

    /// 解析 `key: val, key: val, ...` 直到遇到 RParen 或 RBrace
    /// v0.5 批次 C：允许 KEYWORD 作为 key（如 stroke { material: ... }）
    pub fn parse_kwargs_block(&mut self) -> VglResult<HashMap<String, Expr>> {
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

    pub fn parse_ident_stmt(&mut self) -> VglResult<Stmt> {
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

    pub fn parse_postfix(&mut self, mut expr: Expr) -> VglResult<Expr> {
        loop {
            match self.peek().clone() {
                Token::LParen => {
                    let op_pos = self.peek_pos();
                    self.advance();
                    let (args, kwargs) = self.parse_call_args()?;
                    self.expect(&Token::RParen)?;
                    let name = match expr {
                        Expr::Ident(n) => n,
                        _ => return Err(VglError::new("只有标识符可调用", Some(self.peek_pos()))),
                    };
                    expr = Expr::Call(name, args, kwargs, op_pos);
                }
                Token::LBracket => {
                    let op_pos = self.peek_pos();
                    self.advance();
                    let idx = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    expr = Expr::Index(Box::new(expr), Box::new(idx), op_pos);
                }
                Token::Dot => {
                    let op_pos = self.peek_pos();
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(s) => s,
                        _ => return Err(VglError::new("字段名需要标识符", Some(self.peek_pos()))),
                    };
                    expr = Expr::FieldAccess(Box::new(expr), field, op_pos);
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    pub fn parse_call_args(&mut self) -> VglResult<(Vec<Expr>, HashMap<String, Expr>)> {
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

    pub fn parse_expr(&mut self) -> VglResult<Expr> {
        self.parse_or()
    }
    pub fn parse_or(&mut self) -> VglResult<Expr> {
        let mut left = self.parse_and()?;
        while let Token::Keyword(ref kw) = self.peek() {
            if kw == "or" {
                let op_pos = self.peek_pos();
                self.advance();
                let right = self.parse_and()?;
                left = Expr::LogicOp("or".into(), Box::new(left), Box::new(right), op_pos);
            } else {
                break;
            }
        }
        Ok(left)
    }
    pub fn parse_and(&mut self) -> VglResult<Expr> {
        let mut left = self.parse_compare()?;
        while let Token::Keyword(ref kw) = self.peek() {
            if kw == "and" {
                let op_pos = self.peek_pos();
                self.advance();
                let right = self.parse_compare()?;
                left = Expr::LogicOp("and".into(), Box::new(left), Box::new(right), op_pos);
            } else {
                break;
            }
        }
        Ok(left)
    }
    pub fn parse_compare(&mut self) -> VglResult<Expr> {
        let left = self.parse_add()?;
        if let Token::Op(ref op) = self.peek() {
            if ["<", ">", "<=", ">=", "==", "!="].contains(&op.as_str()) {
                let op_pos = self.peek_pos();
                let op = self.advance();
                let right = self.parse_add()?;
                if let Token::Op(opstr) = op {
                    return Ok(Expr::BinOp(opstr, Box::new(left), Box::new(right), op_pos));
                }
            }
        }
        Ok(left)
    }
    pub fn parse_add(&mut self) -> VglResult<Expr> {
        let mut left = self.parse_mul()?;
        while let Token::Op(ref op) = self.peek() {
            if op == "+" || op == "-" {
                let op_pos = self.peek_pos();
                let op = self.advance();
                let right = self.parse_mul()?;
                if let Token::Op(opstr) = op {
                    left = Expr::BinOp(opstr, Box::new(left), Box::new(right), op_pos);
                }
            } else {
                break;
            }
        }
        Ok(left)
    }
    pub fn parse_mul(&mut self) -> VglResult<Expr> {
        let mut left = self.parse_unary()?;
        while let Token::Op(ref op) = self.peek() {
            if op == "*" || op == "/" {
                let op_pos = self.peek_pos();
                let op = self.advance();
                let right = self.parse_unary()?;
                if let Token::Op(opstr) = op {
                    left = Expr::BinOp(opstr, Box::new(left), Box::new(right), op_pos);
                }
            } else {
                break;
            }
        }
        Ok(left)
    }
    pub fn parse_unary(&mut self) -> VglResult<Expr> {
        if let Token::Op(ref op) = self.peek() {
            if op == "-" {
                let op_pos = self.peek_pos();
                self.advance();
                return Ok(Expr::BinOp(
                    "-".into(),
                    Box::new(Expr::Number(0.0)),
                    Box::new(self.parse_unary()?),
                    op_pos,
                ));
            }
        }
        if let Token::Keyword(ref kw) = self.peek() {
            if kw == "not" {
                let op_pos = self.peek_pos();
                self.advance();
                return Ok(Expr::UnaryNot(Box::new(self.parse_unary()?), op_pos));
            }
        }
        self.parse_primary()
    }
    pub fn parse_primary(&mut self) -> VglResult<Expr> {
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

pub fn start_pos_of(p: &Parser) -> usize {
    p.peek_pos()
}
