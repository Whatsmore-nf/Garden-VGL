// ============================================================
// VGL v2.0 — 词法分析器
// ============================================================

use crate::error::{VglError, VglResult};

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Num(f64),
    Str(String),
    Kw(&'static str),
    Punct(&'static str),
    Eof,
}

const KEYWORDS: &[&str] = &[
    "let", "fn", "if", "else", "for", "in", "while",
    "return", "break", "continue", "use", "canvas", "seed",
    "render", "true", "false", "none",
];

pub struct Lexer<'a> {
    src: &'a str,
    chars: Vec<(usize, char)>,
    idx: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer { src, chars: src.char_indices().collect(), idx: 0 }
    }

    fn peek(&self) -> Option<(usize, char)> {
        self.chars.get(self.idx).copied()
    }

    fn peek_at(&self, off: usize) -> Option<(usize, char)> {
        self.chars.get(self.idx + off).copied()
    }

    fn bump(&mut self) -> Option<(usize, char)> {
        let c = self.peek();
        if c.is_some() {
            self.idx += 1;
        }
        c
    }

    pub fn tokenize(mut self) -> VglResult<Vec<(Tok, usize)>> {
        let mut out = Vec::new();
        loop {
            let (pos, ch) = match self.peek() {
                Some(c) => c,
                None => {
                    out.push((Tok::Eof, self.src.len()));
                    break;
                }
            };
            match ch {
                c if c.is_whitespace() => {
                    self.bump();
                }
                '/' if matches!(self.peek_at(1), Some((_, '/'))) => {
                    while let Some((_, c)) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                '/' if matches!(self.peek_at(1), Some((_, '*'))) => {
                    self.bump();
                    self.bump();
                    let mut closed = false;
                    while let Some((_, c)) = self.bump() {
                        if c == '*' && matches!(self.peek(), Some((_, '/'))) {
                            self.bump();
                            closed = true;
                            break;
                        }
                    }
                    if !closed {
                        return Err(VglError::new("未闭合的块注释 /*", pos));
                    }
                }
                '0'..='9' => {
                    let num = self.read_number()?;
                    // 尺寸字面量: 800x600 → Num(800) Ident("x") Num(600)
                    if let Some((_, 'x' | 'X')) = self.peek() {
                        if matches!(self.peek_at(1), Some((_, '0'..='9'))) {
                            self.bump();
                            let h = self.read_number()?;
                            out.push((num.0, pos));
                            out.push((Tok::Ident("x".to_string()), pos));
                            let hp = self.peek().map(|(p, _)| p).unwrap_or(pos);
                            out.push((h.0, hp));
                            continue;
                        }
                    }
                    out.push(num);
                }
                '"' => {
                    let s = self.read_string(pos)?;
                    out.push((Tok::Str(s), pos));
                }
                '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | ';' | '+' | '-' | '*' | '/'
                | '%' | '<' | '>' | '=' | '!' | '.' => {
                    let tok = self.read_punct()?;
                    out.push((tok, pos));
                }
                c if c.is_alphabetic() || c == '_' => {
                    let mut s = String::new();
                    while let Some((_, c)) = self.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            s.push(c);
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    let kw = KEYWORDS.iter().find(|k| **k == s);
                    out.push((match kw {
                        Some(k) => Tok::Kw(k),
                        None => Tok::Ident(s),
                    }, pos));
                }
                other => {
                    return Err(VglError::new(
                        format!("非法字符 '{}'", other),
                        pos,
                    ));
                }
            }
        }
        Ok(out)
    }

    fn read_number(&mut self) -> VglResult<(Tok, usize)> {
        let pos = self.peek().map(|(p, _)| p).unwrap_or(0);
        let mut s = String::new();
        while let Some((_, c)) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        if let Some((_, '.')) = self.peek() {
            if matches!(self.peek_at(1), Some((_, '0'..='9'))) {
                s.push('.');
                self.bump();
                while let Some((_, c)) = self.peek() {
                    if c.is_ascii_digit() {
                        s.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
        }
        let v: f64 = s
            .parse()
            .map_err(|_| VglError::new(format!("非法数字 '{}'", s), pos))?;
        Ok((Tok::Num(v), pos))
    }

    fn read_string(&mut self, start: usize) -> VglResult<String> {
        self.bump(); // 开头 "
        let mut s = String::new();
        loop {
            match self.bump() {
                Some((_, '"')) => return Ok(s),
                Some((_, '\\')) => match self.bump() {
                    Some((_, 'n')) => s.push('\n'),
                    Some((_, 't')) => s.push('\t'),
                    Some((_, '"')) => s.push('"'),
                    Some((_, '\\')) => s.push('\\'),
                    Some((p, c)) => {
                        return Err(VglError::new(format!("非法转义 \\{}", c), p));
                    }
                    None => return Err(VglError::new("未闭合的字符串", start)),
                },
                Some((_, '\n')) | None => {
                    return Err(VglError::new("未闭合的字符串", start));
                }
                Some((_, c)) => s.push(c),
            }
        }
    }

    fn read_punct(&mut self) -> VglResult<Tok> {
        let (pos, c) = self.peek().unwrap();
        let two: &[&str] = &["..", "==", "!=", "<=", ">="];
        if c == '.' || c == '=' || c == '!' || c == '<' || c == '>' {
            if let Some((_, c2)) = self.peek_at(1) {
                let pair: String = [c, c2].iter().collect();
                if two.contains(&pair.as_str()) {
                    self.bump();
                    self.bump();
                    return Ok(Tok::Punct(match pair.as_str() {
                        ".." => "..",
                        "==" => "==",
                        "!=" => "!=",
                        "<=" => "<=",
                        _ => ">=",
                    }));
                }
            }
        }
        self.bump();
        let p = match c {
            '(' => "(",
            ')' => ")",
            '[' => "[",
            ']' => "]",
            '{' => "{",
            '}' => "}",
            ',' => ",",
            ':' => ":",
            ';' => ";",
            '+' => "+",
            '-' => "-",
            '*' => "*",
            '/' => "/",
            '%' => "%",
            '<' => "<",
            '>' => ">",
            '=' => "=",
            '!' => "!",
            _ => unreachable!(),
        };
        let _ = pos;
        Ok(Tok::Punct(p))
    }
}
