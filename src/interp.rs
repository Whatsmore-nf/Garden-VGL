// ============================================================
// 控制流信号
// ============================================================

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use image::{ImageBuffer, Rgb};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::ast::{Env, Expr, Stmt, StmtWithPos, Value};
use crate::canvas::{Canvas, MaterialParams};
use crate::error::{clamp_f32, clamp_u8, format_error, VglError, VglResult};
use crate::lexer::Lexer;
use crate::noise::{fbm, perlin, worley};
use crate::parser::Parser;

#[derive(Debug)]
pub enum Control {
    Normal,                   // 正常执行，无控制流
    Break(Option<String>), // v0.4 带标签 break
    Continue,
    Return(Value),
}

pub type ExecResult = Result<Control, VglError>;

// ============================================================
// 解释器
// ============================================================

pub struct Interpreter {
    pub canvas: Option<Canvas>,
    pub layers: HashMap<String, Value>,
    pub struct_defs: HashMap<String, (Vec<String>, Vec<Value>)>,
    pub imported: Vec<String>,
    pub rng: Rc<RefCell<StdRng>>,
    pub current_dir: String,
    pub current_filename: String,
    pub current_src: String,
    pub current_pos: Option<usize>,
    pub warnings: Vec<crate::error::VglWarning>,
    pub bg_set: bool,
}

impl Interpreter {
    pub fn new() -> Self {
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
            warnings: Vec::new(),
            bg_set: false,
        }
    }

    pub fn warn(&mut self, msg: impl Into<String>) {
        self.warnings
            .push(crate::error::VglWarning::new(msg, self.current_pos));
    }

    pub fn eval(&mut self, expr: &Expr, env: Rc<RefCell<Env>>) -> VglResult<Value> {
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
            Expr::BinOp(op, l, r, pos) => {
                self.current_pos = Some(*pos);
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
                        match op.as_str() {
                            "+" => {
                                let res: Vec<Value> = a.iter().zip(b.iter()).map(|(x, y)| {
                                    Value::Number(x.as_number().unwrap_or(0.0) + y.as_number().unwrap_or(0.0))
                                }).collect();
                                return Ok(Value::Tuple(res));
                            }
                            "-" => {
                                let res: Vec<Value> = a.iter().zip(b.iter()).map(|(x, y)| {
                                    Value::Number(x.as_number().unwrap_or(0.0) - y.as_number().unwrap_or(0.0))
                                }).collect();
                                return Ok(Value::Tuple(res));
                            }
                            "==" => {
                                let eq = a.iter().zip(b.iter()).all(|(x, y)| {
                                    if let (Value::Number(xn), Value::Number(yn)) = (x, y) { xn == yn }
                                    else { x == y }
                                });
                                return Ok(Value::Bool(eq));
                            }
                            "!=" => {
                                let ne = a.iter().zip(b.iter()).any(|(x, y)| {
                                    if let (Value::Number(xn), Value::Number(yn)) = (x, y) { xn != yn }
                                    else { x != y }
                                });
                                return Ok(Value::Bool(ne));
                            }
                            "<" | ">" | "<=" | ">=" => {
                                for (x, y) in a.iter().zip(b.iter()) {
                                    let xn = x.as_number().ok_or_else(|| VglError::new("元组比较需要元素为 number", self.current_pos))?;
                                    let yn = y.as_number().ok_or_else(|| VglError::new("元组比较需要元素为 number", self.current_pos))?;
                                    if xn < yn { return Ok(Value::Bool(op == "<" || op == "<=")); }
                                    if xn > yn { return Ok(Value::Bool(op == ">" || op == ">=")); }
                                }
                                return Ok(Value::Bool(op == "<=" || op == ">="));
                            }
                            _ => return Err(VglError::new("元组只支持 +/-/比较", self.current_pos)),
                        }
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
            Expr::LogicOp(op, l, r, pos) => {
                self.current_pos = Some(*pos);
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
            Expr::UnaryNot(e, pos) => {
                self.current_pos = Some(*pos);
                let v = self.eval(e, env.clone())?;
                match v {
                    Value::Bool(b) => Ok(Value::Bool(!b)),
                    Value::Number(n) => Ok(Value::Bool(n == 0.0)),
                    _ => Err(VglError::new("not 作用于非 bool", self.current_pos)),
                }
            }
            Expr::Call(name, args, kwargs, pos) => {
                self.current_pos = Some(*pos);
                self.eval_call(name, args, kwargs, env)
            }
            Expr::Index(base, idx, pos) => {
                self.current_pos = Some(*pos);
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
            Expr::FieldAccess(obj, field, pos) => {
                self.current_pos = Some(*pos);
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

    pub fn eval_call(
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
    pub fn call_builtin(&mut self, name: &str, args: &[Value]) -> VglResult<Option<Value>> {
        macro_rules! num {
            ($i:expr) => {
                args.get($i).and_then(|v| v.as_number()).unwrap_or(0.0)
            };
        }
        let v = match name {
            "rand" => {
                let a = num!(0);
                let b = num!(1);
                if a >= b {
                    return Err(VglError::new(format!("rand(a,b) 要求 a < b，得到 a={}, b={}", a, b), self.current_pos));
                }
                Value::Number(self.rng.borrow_mut().gen_range(a..b))
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
            "load" => {
                let path = match args.get(0) {
                    Some(Value::String(s)) => s.clone(),
                    _ => return Err(VglError::new("load 需要字符串路径参数", self.current_pos)),
                };
                let full = if Path::new(&path).is_absolute() {
                    path.clone()
                } else {
                    format!("{}/{}", self.current_dir, path)
                };
                let img = image::open(&full).map_err(|e| {
                    VglError::new(format!("load 失败: {} ({})", path, e), self.current_pos)
                })?;
                let rgb = img.to_rgb8();
                let (w, h) = (rgb.width(), rgb.height());
                let mut canvas = Canvas::new(w, h);
                for y in 0..h {
                    for x in 0..w {
                        let px = rgb.get_pixel(x, y);
                        let idx = (y * w + x) as usize * 4;
                        canvas.pixels[idx] = px[0] as f32;
                        canvas.pixels[idx + 1] = px[1] as f32;
                        canvas.pixels[idx + 2] = px[2] as f32;
                        canvas.pixels[idx + 3] = 255.0;
                    }
                }
                Value::Image(Rc::new(canvas))
            }
            _ => return Ok(None),
        };
        Ok(Some(v))
    }

    pub fn exec(&mut self, sp: &StmtWithPos, env: Rc<RefCell<Env>>) -> ExecResult {
        // 更新当前语句位置（运行时错误定位）
        self.current_pos = Some(sp.pos);
        let stmt = &sp.stmt;
        match stmt {
            Stmt::Canvas(w, h) => {
                self.canvas = Some(Canvas::new(*w, *h));
                Ok(Control::Normal)
            }
            Stmt::Bg(expr) => {
                if self.bg_set {
                    self.warn("重复设置背景色");
                } else {
                    self.bg_set = true;
                }
                let val = self.eval(expr, env.clone())?;
                let (r, g, b) = match val {
                    Value::Color(r, g, b) => (r as f32, g as f32, b as f32),
                    Value::Tuple(t) => {
                        if t.len() >= 3 {
                            (
                                clamp_f32(t[0].as_number().unwrap_or(0.0) as f32),
                                clamp_f32(t[1].as_number().unwrap_or(0.0) as f32),
                                clamp_f32(t[2].as_number().unwrap_or(0.0) as f32),
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
                    canvas.bg = (r, g, b, 255.0);
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
                if let Some(Value::Closure(_, _, _, _)) = env.borrow().get(name) {
                    self.warn(format!("函数 {} 被覆盖", name));
                }
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
                    Value::Color(r, g, b) => (r as f32, g as f32, b as f32),
                    Value::Tuple(t) => (
                        clamp_f32(t.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
                        clamp_f32(t.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
                        clamp_f32(t.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
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
                    // f32 RGBA → u8 RGB（丢弃 alpha，背景已合成）
                    let mut rgb_bytes = Vec::with_capacity((canvas.width * canvas.height * 3) as usize);
                    for i in (0..canvas.pixels.len()).step_by(4) {
                        rgb_bytes.push(clamp_u8(canvas.pixels[i] as f64));
                        rgb_bytes.push(clamp_u8(canvas.pixels[i + 1] as f64));
                        rgb_bytes.push(clamp_u8(canvas.pixels[i + 2] as f64));
                    }
                    let img = ImageBuffer::<Rgb<u8>, _>::from_vec(
                        canvas.width,
                        canvas.height,
                        rgb_bytes,
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
    pub fn eval_error(&self, msg: &str) -> Result<Control, VglError> {
        let full = format_error(
            msg,
            &self.current_src,
            self.current_pos,
            &self.current_filename,
        );
        eprintln!("VGL 错误: {}", full);
        std::process::exit(1);
    }

    pub fn exec_stroke(&mut self, fields: &HashMap<String, Expr>, env: Rc<RefCell<Env>>) -> ExecResult {
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
        // v0.5 批次 B：samples 字段支持
        let samples = fields
            .get("samples")
            .map(|e| self.eval(e, block_env.clone()).map(|v| v.as_number().unwrap_or(0.0) as i32))
            .transpose()?
            .unwrap_or(0);
        // v0.55 批次 D：材质分支用 _mat 系列方法（逐像素 noise + alpha 集成）
        if let Some(mat_expr) = fields.get("material") {
            let mat_val = self.eval(mat_expr, block_env.clone())?;
            if let Value::Material(mat_map) = mat_val {
                let base = mat_map.get("color").cloned().unwrap_or(Value::Color(0, 0, 0));
                let (cr, cg, cb): (f32, f32, f32) = match base {
                    Value::Color(r, g, b) => (r as f32, g as f32, b as f32),
                    Value::Tuple(t) => (
                        clamp_f32(t.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
                        clamp_f32(t.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
                        clamp_f32(t.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
                    ),
                    _ => (0.0, 0.0, 0.0),
                };
                let noise = mat_map.get("noise").and_then(|v| v.as_number()).unwrap_or(0.0);
                // 材质 alpha 默认 1.0（完全不透明），存为 [0,255]
                let alpha = mat_map.get("alpha").and_then(|v| v.as_number()).unwrap_or(1.0) as f32 * 255.0;
                let mat = MaterialParams { r: cr, g: cg, b: cb, noise, alpha };
                if let Some(canvas) = &mut self.canvas {
                    match path_val {
                        Value::Path(tag, args) => match tag.as_str() {
                            "line" => {
                                let p1 = args.get(0).and_then(|v| v.as_tuple()).unwrap_or_default();
                                let p2 = args.get(1).and_then(|v| v.as_tuple()).unwrap_or_default();
                                canvas.draw_line_mat(
                                    p1.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                    p1.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                    p2.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                    p2.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                    width, &mat,
                                );
                            }
                            "circle" => {
                                let cx = args.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
                                let cy = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
                                let rad = args.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
                                canvas.draw_circle_mat(cx, cy, rad, width, &mat);
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
                                    canvas.draw_line_mat(pts[i].0, pts[i].1, pts[i + 1].0, pts[i + 1].1, width, &mat);
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
                                    canvas.draw_line_mat(pts[i].0, pts[i].1, pts[i + 1].0, pts[i + 1].1, width, &mat);
                                }
                            }
                            "polyline" => {
                                if args.len() > 1 {
                                    for i in 0..args.len() - 1 {
                                        let p1 = args[i].as_tuple().unwrap_or_default();
                                        let p2 = args[i + 1].as_tuple().unwrap_or_default();
                                        canvas.draw_line_mat(
                                            p1.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                            p1.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                            p2.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                            p2.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                                            width, &mat,
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
                return Ok(Control::Normal);
            } else {
                let _ = self.eval_error("material 必须是材质类型")?;
                unreachable!()
            }
        }
        // 无 material 分支：保持原有逻辑（draw_line/draw_circle 传 f32 颜色，不透明）
        let (r, g, b): (f32, f32, f32) = {
            let color_val = self.eval(fields.get("color").unwrap_or(&Expr::Color(0, 0, 0)), block_env.clone())?;
            match color_val {
                Value::Color(r, g, b) => (r as f32, g as f32, b as f32),
                Value::Tuple(t) => (
                    clamp_f32(t.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
                    clamp_f32(t.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
                    clamp_f32(t.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
                ),
                _ => {
                    let _ = self.eval_error("color 错误")?;
                    unreachable!()
                }
            }
        };
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

    pub fn do_import(&mut self, path: &str, env: Rc<RefCell<Env>>) -> ExecResult {
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

    pub fn exec_layer(&mut self, name: &str, body: &[StmtWithPos], env: Rc<RefCell<Env>>) -> ExecResult {
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

    pub fn execute_block(&mut self, body: &[StmtWithPos], env: Rc<RefCell<Env>>) -> ExecResult {
        for stmt in body {
            match self.exec(stmt, env.clone())? {
                Control::Normal => {},
                c => return Ok(c),
            }
        }
        Ok(Control::Normal)
    }

    pub fn construct_struct(
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

    pub fn compose_layer(&mut self, name: &str, blend: &str) -> VglResult<()> {
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
                    let idx = (y * lw + x) as usize * 4;
                    let mr = canvas.pixels[idx];
                    let mg = canvas.pixels[idx + 1];
                    let mb = canvas.pixels[idx + 2];
                    let ma = canvas.pixels[idx + 3];
                    let lr = layer.pixels[idx];
                    let lg = layer.pixels[idx + 1];
                    let lb = layer.pixels[idx + 2];
                    let la = layer.pixels[idx + 3];
                    let (nr, ng, nb, na) = match blend {
                        "add" => (
                            (mr + lr).min(255.0),
                            (mg + lg).min(255.0),
                            (mb + lb).min(255.0),
                            ma.max(la),
                        ),
                        "mul" => (
                            mr * lr / 255.0,
                            mg * lg / 255.0,
                            mb * lb / 255.0,
                            ma.max(la),
                        ),
                        "screen" => (
                            255.0 - (255.0 - mr) * (255.0 - lr) / 255.0,
                            255.0 - (255.0 - mg) * (255.0 - lg) / 255.0,
                            255.0 - (255.0 - mb) * (255.0 - lb) / 255.0,
                            ma.max(la),
                        ),
                        _ => {
                            // over: 真 alpha 合成
                            // src = layer (lr,lg,lb,la)，dst = canvas (mr,mg,mb,ma)
                            let sa = la / 255.0;
                            let da = ma / 255.0;
                            let out_a = sa + da * (1.0 - sa);
                            if out_a <= 0.0 {
                                (0.0, 0.0, 0.0, 0.0)
                            } else {
                                let or = (lr * sa + mr * da * (1.0 - sa)) / out_a;
                                let og = (lg * sa + mg * da * (1.0 - sa)) / out_a;
                                let ob = (lb * sa + mb * da * (1.0 - sa)) / out_a;
                                (or, og, ob, out_a * 255.0)
                            }
                        }
                    };
                    canvas.pixels[idx] = nr.max(0.0).min(255.0);
                    canvas.pixels[idx + 1] = ng.max(0.0).min(255.0);
                    canvas.pixels[idx + 2] = nb.max(0.0).min(255.0);
                    canvas.pixels[idx + 3] = na.max(0.0).min(255.0);
                }
            }
        }
        Ok(())
    }

    pub fn fill_field(&mut self, name: &str, env: Rc<RefCell<Env>>) -> VglResult<()> {
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
                    Value::Color(r, g, b) => Some((r as f32, g as f32, b as f32)),
                    Value::Tuple(ref t) if t.len() >= 3 => Some((
                        clamp_f32(t.get(0).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
                        clamp_f32(t.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
                        clamp_f32(t.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as f32),
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
