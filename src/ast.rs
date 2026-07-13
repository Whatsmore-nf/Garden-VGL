// ============================================================
// AST
// ============================================================

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::canvas::Canvas;

#[derive(Clone, Debug)]
pub enum Expr {
    Number(f64),
    String(String),
    Color(u8, u8, u8),
    Bool(bool),
    Ident(String),
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    BinOp(String, Box<Expr>, Box<Expr>, usize),
    LogicOp(String, Box<Expr>, Box<Expr>, usize),
    UnaryNot(Box<Expr>, usize),
    Index(Box<Expr>, Box<Expr>, usize),
    FieldAccess(Box<Expr>, String, usize),
    Call(String, Vec<Expr>, HashMap<String, Expr>, usize),
}

#[derive(Clone, Debug)]
pub enum Stmt {
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
pub struct StmtWithPos {
    pub stmt: Stmt,
    pub pos: usize,
}

/// 为表达式附加位置信息（运行时错误定位用）
#[derive(Clone, Debug)]
pub struct ExprWithPos {
    pub expr: Expr,
    pub pos: usize,
}

// ============================================================
// 运行时环境
// ============================================================

#[derive(Clone, Debug)]
pub enum Value {
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
    Image(Rc<Canvas>), // 加载的图片，复用 Canvas 结构
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
            (Value::Image(a), Value::Image(b)) => Rc::ptr_eq(a, b),
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
            Value::Image(c) => (Rc::as_ptr(c) as usize).hash(state),
            _ => 0.hash(state),
        }
    }
}

impl Value {
    pub fn as_number(&self) -> Option<f64> {
        if let Value::Number(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    pub fn as_tuple(&self) -> Option<Vec<Value>> {
        if let Value::Tuple(t) = self {
            Some(t.clone())
        } else {
            None
        }
    }
    pub fn as_string(&self) -> Option<String> {
        if let Value::String(s) = self {
            Some(s.clone())
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct Env {
    pub vars: HashMap<String, Value>,
    pub parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    pub fn new(parent: Option<Rc<RefCell<Env>>>) -> Self {
        Env {
            vars: HashMap::new(),
            parent,
        }
    }
    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(v) = self.vars.get(name) {
            return Some(v.clone());
        }
        if let Some(ref p) = self.parent {
            return p.borrow().get(name);
        }
        None
    }
    pub fn contains(&self, name: &str) -> bool {
        if self.vars.contains_key(name) {
            true
        } else if let Some(ref p) = self.parent {
            p.borrow().contains(name)
        } else {
            false
        }
    }
    pub fn set(&mut self, name: &str, val: Value) -> Result<(), String> {
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
