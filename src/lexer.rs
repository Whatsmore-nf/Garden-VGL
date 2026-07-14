// ============================================================
// 词法分析
// ============================================================

use crate::error::{VglError, VglResult};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    String(String),
    Color(u8, u8, u8, u8), // v0.9: 8 位 hex 支持 alpha 通道
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
pub struct TokenWithPos {
    pub tok: Token,
    pub pos: usize,
}

pub struct Lexer {
    pub chars: Vec<char>,
    pub pos: usize,
}

impl Lexer {
    pub fn new(s: &str) -> Self {
        Lexer {
            chars: s.chars().collect(),
            pos: 0,
        }
    }
    pub fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    pub fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        self.pos += 1;
        c
    }
    pub fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if !c.is_whitespace() {
                break;
            }
            self.advance();
        }
    }
    pub fn read_number(&mut self) -> VglResult<f64> {
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
    pub fn read_ident(&mut self) -> String {
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
    pub fn read_string(&mut self) -> VglResult<String> {
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
    pub fn read_color(&mut self) -> VglResult<(u8, u8, u8, u8)> {
        // v0.9: 返回 (r, g, b, a)，支持 #RGB / #RRGGBB / #RGBA / #RRGGBBAA
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
            Ok((r, g, b, 255))
        } else if hex.len() == 8 {
            // v0.9: #RRGGBBAA 8 位 hex 支持 alpha 通道
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap();
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap();
            Ok((r, g, b, a))
        } else if hex.len() == 3 {
            let r = u8::from_str_radix(&format!("{}{}", &hex[0..1], &hex[0..1]), 16).unwrap();
            let g = u8::from_str_radix(&format!("{}{}", &hex[1..2], &hex[1..2]), 16).unwrap();
            let b = u8::from_str_radix(&format!("{}{}", &hex[2..3], &hex[2..3]), 16).unwrap();
            Ok((r, g, b, 255))
        } else if hex.len() == 4 {
            // v0.9: #RGBA 4 位 hex 支持 alpha 通道
            let r = u8::from_str_radix(&format!("{}{}", &hex[0..1], &hex[0..1]), 16).unwrap();
            let g = u8::from_str_radix(&format!("{}{}", &hex[1..2], &hex[1..2]), 16).unwrap();
            let b = u8::from_str_radix(&format!("{}{}", &hex[2..3], &hex[2..3]), 16).unwrap();
            let a = u8::from_str_radix(&format!("{}{}", &hex[3..4], &hex[3..4]), 16).unwrap();
            Ok((r, g, b, a))
        } else {
            Err(VglError::new(format!("非法颜色 #{}", hex), Some(start_pos)))
        }
    }
    pub fn next_token(&mut self) -> VglResult<TokenWithPos> {
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
            let (r, g, b, a) = self.read_color()?;
            return Ok(TokenWithPos { tok: Token::Color(r, g, b, a), pos: tok_pos });
        }
        if c.is_alphabetic() || c == '_' {
            let ident = self.read_ident();
            let kw = [
                "canvas", "bg", "let", "for", "in", "if", "else", "fn", "return", "pixel",
                "stroke", "render", "while", "break", "and", "or", "not", "seed", "true",
                "false", "continue", "struct", "import", "material", "layer", "field",
                "null", "const", "var", // v0.9: null 字面量 + const/var 绑定
                "as", "match", "case", "default", "enum", "class", "from", "module", // v0.9: as/match/enum/class/module 关键字
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
            // v0.8: 添加 % 单字符运算符；扩展双字符识别以支持复合赋值 += -= *= /= %=
            // v0.9: 新增位运算符 & | ^ ~ 与移位 << >>、自增自减 ++ --
            '+' | '-' | '*' | '/' | '=' | '<' | '>' | '!' | '%' | '&' | '|' | '^' | '~' => {
                self.advance();
                let mut op = c.to_string();
                if let Some(nxt) = self.peek() {
                    // v0.9: 双字符运算符 << >> ++ -- => （移位/自增自减/match 箭头，优先于 = 检测）
                    let two_char = match (c, nxt) {
                        ('<', '<') => Some("<<"),
                        ('>', '>') => Some(">>"),
                        ('+', '+') => Some("++"),
                        ('-', '-') => Some("--"),
                        ('=', '>') => Some("=>"),
                        _ => None,
                    };
                    if let Some(tw) = two_char {
                        self.advance();
                        op = tw.to_string();
                    } else if nxt == '=' && matches!(c, '<' | '>' | '=' | '!' | '+' | '-' | '*' | '/' | '%') {
                        // 双字符运算符：==, !=, <=, >= 以及复合赋值 +=, -=, *=, /=, %=
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
    pub fn tokenize(&mut self) -> VglResult<Vec<TokenWithPos>> {
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
