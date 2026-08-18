// ============================================================
// VGL v2.0 — AST 与值类型
// ============================================================

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ---------- 颜色 ----------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f64, // 0-255
    pub g: f64,
    pub b: f64,
    pub a: f64, // 0-1
}

impl Color {
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Color { r: r.clamp(0.0, 255.0), g: g.clamp(0.0, 255.0), b: b.clamp(0.0, 255.0), a: a.clamp(0.0, 1.0) }
    }

    pub fn hex(&self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            self.r.round() as u8,
            self.g.round() as u8,
            self.b.round() as u8
        )
    }
}

/// 解析 "#rgb" / "#rrggbb" / "#rrggbbaa" 颜色字符串
pub fn parse_hex_color(s: &str) -> Option<Color> {
    let h = s.strip_prefix('#')?;
    match h.len() {
        3 => {
            let r = u8::from_str_radix(&h[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&h[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&h[2..3].repeat(2), 16).ok()?;
            Some(Color::new(r as f64, g as f64, b as f64, 1.0))
        }
        6 => {
            let r = u8::from_str_radix(&h[0..2], 16).ok()?;
            let g = u8::from_str_radix(&h[2..4], 16).ok()?;
            let b = u8::from_str_radix(&h[4..6], 16).ok()?;
            Some(Color::new(r as f64, g as f64, b as f64, 1.0))
        }
        8 => {
            let r = u8::from_str_radix(&h[0..2], 16).ok()?;
            let g = u8::from_str_radix(&h[2..4], 16).ok()?;
            let b = u8::from_str_radix(&h[4..6], 16).ok()?;
            let a = u8::from_str_radix(&h[6..8], 16).ok()?;
            Some(Color::new(r as f64, g as f64, b as f64, a as f64 / 255.0))
        }
        _ => None,
    }
}

// ---------- 渐变（矢量 defs） ----------

#[derive(Debug, Clone)]
pub enum GradKind {
    Linear,
    Radial,
}

#[derive(Debug, Clone)]
pub struct GradSpec {
    pub kind: GradKind,
    /// Linear: [x1,y1,x2,y2]; Radial: [cx,cy,r]
    pub coords: Vec<f64>,
    /// (颜色, 偏移 0-1)
    pub stops: Vec<(Color, f64)>,
}

// ---------- 值 ----------

#[derive(Debug, Clone)]
pub enum Value {
    Num(f64),
    Bool(bool),
    Str(String),
    Color(Color),
    Grad(Rc<GradSpec>),
    Arr(Rc<RefCell<Vec<Value>>>),
    Fn(Rc<FnDef>),
    None,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Num(_) => "number",
            Value::Bool(_) => "bool",
            Value::Str(_) => "string",
            Value::Color(_) => "color",
            Value::Grad(_) => "gradient",
            Value::Arr(_) => "array",
            Value::Fn(_) => "function",
            Value::None => "none",
        }
    }
}

// ---------- 表达式 ----------

#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64, usize),
    Str(String, usize),
    Bool(bool, usize),
    NoneLit(usize),
    Ident(String, usize),
    Unary { op: &'static str, expr: Box<Expr>, pos: usize },
    Binary { op: &'static str, lhs: Box<Expr>, rhs: Box<Expr>, pos: usize },
    Index { obj: Box<Expr>, idx: Box<Expr>, pos: usize },
    Call {
        name: String,
        args: Vec<Expr>,
        named: Vec<(String, Expr, usize)>,
        pos: usize,
    },
    ArrLit(Vec<Expr>, usize),
}

impl Expr {
    pub fn pos(&self) -> usize {
        match self {
            Expr::Num(_, p) | Expr::Str(_, p) | Expr::Bool(_, p) | Expr::NoneLit(p)
            | Expr::Ident(_, p) | Expr::ArrLit(_, p) => *p,
            Expr::Unary { pos, .. } | Expr::Binary { pos, .. } | Expr::Index { pos, .. }
            | Expr::Call { pos, .. } => *pos,
        }
    }
}

// ---------- 语句 ----------

#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub params: Vec<(String, Option<Expr>)>,
    pub body: Vec<Stmt>,
    pub env: Rc<RefCell<Env>>,
    pub pos: usize,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Use(String, usize),
    Canvas { w: f64, h: f64, pos: usize },
    Seed(Expr, usize),
    Render(Expr, usize),
    Let { name: String, expr: Expr, pos: usize },
    FnDef(Rc<FnDef>),
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        else_body: Option<Vec<Stmt>>,
        pos: usize,
    },
    ForRange {
        var: String,
        start: Expr,
        end: Expr,
        step: Option<Expr>,
        body: Vec<Stmt>,
        pos: usize,
    },
    ForIn {
        var: String,
        arr: Expr,
        body: Vec<Stmt>,
        pos: usize,
    },
    While(Expr, Vec<Stmt>, usize),
    Return(Option<Expr>, usize),
    Break(usize),
    Continue(usize),
    /// group(命名参数...) { 子语句 }
    Group {
        named: Vec<(String, Expr, usize)>,
        body: Vec<Stmt>,
        pos: usize,
    },
    Expr(Expr),
}

impl Stmt {
    pub fn pos(&self) -> usize {
        match self {
            Stmt::Use(_, p) | Stmt::Seed(_, p) | Stmt::Render(_, p) | Stmt::Break(p)
            | Stmt::Continue(p) | Stmt::While(_, _, p) | Stmt::Return(_, p) => *p,
            Stmt::Canvas { pos, .. } | Stmt::Let { pos, .. } => *pos,
            Stmt::FnDef(f) => f.pos,
            Stmt::If { pos, .. } | Stmt::ForRange { pos, .. } | Stmt::ForIn { pos, .. }
            | Stmt::Group { pos, .. } => *pos,
            Stmt::Expr(e) => e.pos(),
        }
    }
}

// ---------- 作用域 ----------

#[derive(Debug)]
pub struct Env {
    pub vars: HashMap<String, Value>,
    pub parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    pub fn new(parent: Option<Rc<RefCell<Env>>>) -> Self {
        Env { vars: HashMap::new(), parent }
    }

    pub fn lookup(env: &Rc<RefCell<Env>>, name: &str) -> Option<Value> {
        let e = env.borrow();
        if let Some(v) = e.vars.get(name) {
            return Some(v.clone());
        }
        match &e.parent {
            Some(p) => Env::lookup(p, name),
            None => None,
        }
    }

    pub fn define(env: &Rc<RefCell<Env>>, name: &str, v: Value) {
        env.borrow_mut().vars.insert(name.to_string(), v);
    }
}
