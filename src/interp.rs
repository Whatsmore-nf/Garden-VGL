// ============================================================
// VGL v2.0 — 解释器
// 执行语句构建矢量场景图，render 时序列化为 SVG
// ============================================================

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ast::*;
use crate::error::{VglError, VglResult};
use crate::noise::{fbm, perlin, seeded_perm, Rng};
use crate::scene::{Element, Scene};
use crate::svg::{fmt_num, write_svg};

pub enum Control {
    Normal,
    Break,
    Continue,
    Return(Value),
}

const MAX_DEPTH: usize = 2048;

pub struct Interpreter {
    pub rng: Rng,
    pub perm: Vec<usize>,
    pub scene: Scene,
    /// 渐变/滤镜内容 → def id（去重）
    def_map: HashMap<String, String>,
    next_def: usize,
    /// 已 import 的绝对路径
    pub imported: Vec<String>,
    /// 主脚本目录（render 输出相对基准）
    pub base_dir: String,
    /// 当前正在执行的文件（错误定位 & use 相对路径）
    pub current_dir: String,
    pub current_filename: String,
    pub current_src: String,
    depth: usize,
    /// render 后报告
    pub rendered: Vec<String>,
}

impl Interpreter {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x5EED);
        Interpreter {
            rng: Rng::new(seed),
            perm: seeded_perm(seed),
            scene: Scene::new(),
            def_map: HashMap::new(),
            next_def: 0,
            imported: Vec::new(),
            base_dir: ".".to_string(),
            current_dir: ".".to_string(),
            current_filename: "<main>".to_string(),
            current_src: String::new(),
            depth: 0,
            rendered: Vec::new(),
        }
    }

    pub fn init_globals(env: &Rc<RefCell<Env>>) {
        Env::define(env, "PI", Value::Num(std::f64::consts::PI));
        Env::define(env, "TAU", Value::Num(std::f64::consts::TAU));
    }

    // ==================== 语句执行 ====================

    pub fn exec(&mut self, stmt: &Stmt, env: Rc<RefCell<Env>>) -> VglResult<Control> {
        match stmt {
            Stmt::Use(path, pos) => {
                self.exec_use(path, env, *pos)?;
                Ok(Control::Normal)
            }
            Stmt::Canvas { w, h, pos } => {
                if *w <= 0.0 || *h <= 0.0 {
                    return Err(VglError::new(
                        format!("canvas 尺寸必须为正数（得到 {}x{}）", w, h),
                        *pos,
                    ));
                }
                self.scene.width = *w;
                self.scene.height = *h;
                Ok(Control::Normal)
            }
            Stmt::Seed(e, pos) => {
                let v = self.eval(e, env)?;
                let bits = match v {
                    Value::Num(n) => n.to_bits(),
                    _ => return Err(VglError::new("seed 需要数字", *pos)),
                };
                self.rng = Rng::new(bits);
                self.perm = seeded_perm(bits);
                Ok(Control::Normal)
            }
            Stmt::Render(e, pos) => {
                let v = self.eval(e, env)?;
                let fname = match v {
                    Value::Str(s) => s,
                    other => {
                        return Err(VglError::new(
                            format!("render 需要 SVG 文件名字符串，得到 {}", other.type_name()),
                            *pos,
                        ))
                    }
                };
                self.render_to(&fname, *pos)?;
                Ok(Control::Normal)
            }
            Stmt::Let { name, expr, .. } => {
                let v = self.eval(expr, env.clone())?;
                Env::define(&env, name, v);
                Ok(Control::Normal)
            }
            Stmt::FnDef(f) => {
                let def = Rc::new(FnDef {
                    name: f.name.clone(),
                    params: f.params.clone(),
                    body: f.body.clone(),
                    env: env.clone(),
                    pos: f.pos,
                });
                Env::define(&env, &f.name, Value::Fn(def));
                Ok(Control::Normal)
            }
            Stmt::If { branches, else_body, .. } => {
                for (cond, body) in branches {
                    let c = self.eval(cond, env.clone())?;
                    if self.truthy(c, cond.pos())? {
                        return self.exec_block(body, env);
                    }
                }
                if let Some(body) = else_body {
                    return self.exec_block(body, env);
                }
                Ok(Control::Normal)
            }
            Stmt::ForRange { var, start, end, step, body, pos } => {
                let sv = self.eval(start, env.clone())?;
                let ev = self.eval(end, env.clone())?;
                let s = self.num(sv, *pos, "range 起点")?;
                let e = self.num(ev, *pos, "range 终点")?;
                let st = match step {
                    Some(x) => {
                        let xv = self.eval(x, env.clone())?;
                        self.num(xv, *pos, "range 步长")?
                    }
                    None => 1.0,
                };
                if st == 0.0 {
                    return Err(VglError::new("for 步长不能为 0", *pos));
                }
                let mut i = s;
                loop {
                    if st > 0.0 && i >= e {
                        break;
                    }
                    if st < 0.0 && i <= e {
                        break;
                    }
                    let scope = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    Env::define(&scope, var, Value::Num(i));
                    match self.exec_block(body, scope)? {
                        Control::Break => break,
                        Control::Return(v) => return Ok(Control::Return(v)),
                        _ => {}
                    }
                    i += st;
                }
                Ok(Control::Normal)
            }
            Stmt::ForIn { var, arr, body, pos } => {
                let v = self.eval(arr, env.clone())?;
                let items = match &v {
                    Value::Arr(a) => a.borrow().clone(),
                    _ => {
                        return Err(VglError::new(
                            format!("for-in 需要数组，得到 {}", v.type_name()),
                            *pos,
                        ))
                    }
                };
                for item in items {
                    let scope = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    Env::define(&scope, var, item);
                    match self.exec_block(body, scope)? {
                        Control::Break => break,
                        Control::Return(v) => return Ok(Control::Return(v)),
                        _ => {}
                    }
                }
                Ok(Control::Normal)
            }
            Stmt::While(cond, body, _) => {
                loop {
                    let c = self.eval(cond, env.clone())?;
                    if !self.truthy(c, cond.pos())? {
                        break;
                    }
                    let scope = Rc::new(RefCell::new(Env::new(Some(env.clone()))));
                    match self.exec_block(body, scope)? {
                        Control::Break => break,
                        Control::Return(v) => return Ok(Control::Return(v)),
                        _ => {}
                    }
                }
                Ok(Control::Normal)
            }
            Stmt::Return(e, _) => {
                let v = match e {
                    Some(x) => self.eval(x, env)?,
                    None => Value::None,
                };
                Ok(Control::Return(v))
            }
            Stmt::Break(_) => Ok(Control::Break),
            Stmt::Continue(_) => Ok(Control::Continue),
            Stmt::Group { named, body, pos } => {
                let mut named_v = Vec::new();
                for (name, e, p) in named {
                    named_v.push((name.clone(), self.eval(e, env.clone())?, *p));
                }
                let g = self.build_group(&mut named_v, *pos)?;
                self.scene.open_group(g);
                let r = self.exec_block(body, env);
                self.scene.close_group();
                r
            }
            Stmt::Expr(e) => {
                self.eval(e, env)?;
                Ok(Control::Normal)
            }
        }
    }

    fn exec_block(
        &mut self,
        stmts: &[Stmt],
        env: Rc<RefCell<Env>>,
    ) -> VglResult<Control> {
        for s in stmts {
            match self.exec(s, env.clone())? {
                Control::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Control::Normal)
    }

    fn exec_use(&mut self, path: &str, env: Rc<RefCell<Env>>, pos: usize) -> VglResult<()> {
        let full = if std::path::Path::new(path).is_absolute() {
            path.to_string()
        } else {
            join_path(&self.current_dir, path)
        };
        let abs = std::fs::canonicalize(&full)
            .map_err(|e| VglError::new(format!("无法读取 {}: {}", full, e), pos))?
            .to_string_lossy()
            .to_string();
        if self.imported.contains(&abs) {
            return Ok(());
        }
        self.imported.push(abs.clone());
        let src = std::fs::read_to_string(&abs)
            .map_err(|e| VglError::new(format!("无法读取 {}: {}", abs, e), pos))?;
        let ast = {
            let lexer = crate::lexer::Lexer::new(&src);
            let toks = lexer.tokenize()?;
            let mut parser = crate::parser::Parser::new(toks);
            parser.parse_program()?
        };
        // 切换文件上下文（嵌套 use 相对当前文件；错误定位正确）
        let (prev_dir, prev_file, prev_src) = (
            self.current_dir.clone(),
            self.current_filename.clone(),
            self.current_src.clone(),
        );
        self.current_dir = std::path::Path::new(&abs)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into());
        self.current_filename = abs.clone();
        self.current_src = src;
        for s in &ast {
            match self.exec(s, env.clone())? {
                Control::Normal => {}
                _ => break,
            }
        }
        self.current_dir = prev_dir;
        self.current_filename = prev_file;
        self.current_src = prev_src;
        Ok(())
    }

    fn render_to(&mut self, fname: &str, pos: usize) -> VglResult<()> {
        let full = join_path(&self.base_dir, fname);
        if self.scene.width <= 0.0 || self.scene.height <= 0.0 {
            return Err(VglError::new(
                "render 前需要先声明 canvas 尺寸（如 canvas 800x600）",
                pos,
            ));
        }
        if !self.scene.open.is_empty() {
            return Err(VglError::new("存在未闭合的 group", pos));
        }
        let svg = write_svg(&self.scene);
        std::fs::write(&full, svg)
            .map_err(|e| VglError::new(format!("无法写入 {}: {}", full, e), pos))?;
        let n = self.scene.root.len();
        eprintln!("已渲染: {} ({}x{}, {} 个元素)", full, fmt_num(self.scene.width), fmt_num(self.scene.height), n);
        self.rendered.push(full);
        self.scene.root.clear();
        self.scene.defs.clear();
        self.def_map.clear();
        self.next_def = 0;
        Ok(())
    }

    // ==================== 表达式求值 ====================

    pub fn eval(&mut self, expr: &Expr, env: Rc<RefCell<Env>>) -> VglResult<Value> {
        match expr {
            Expr::Num(v, _) => Ok(Value::Num(*v)),
            Expr::Str(s, _) => Ok(Value::Str(s.clone())),
            Expr::Bool(b, _) => Ok(Value::Bool(*b)),
            Expr::NoneLit(_) => Ok(Value::None),
            Expr::Ident(name, pos) => Env::lookup(&env, name).ok_or_else(|| {
                VglError::new(format!("未定义的变量 '{}'", name), *pos)
            }),
            Expr::ArrLit(items, _) => {
                let mut v = Vec::new();
                for i in items {
                    v.push(self.eval(i, env.clone())?);
                }
                Ok(Value::Arr(Rc::new(RefCell::new(v))))
            }
            Expr::Unary { op, expr, pos } => {
                let v = self.eval(expr, env)?;
                match (*op, v) {
                    ("-", Value::Num(n)) => Ok(Value::Num(-n)),
                    ("!", Value::Bool(b)) => Ok(Value::Bool(!b)),
                    (op, v) => Err(VglError::new(
                        format!("一元 '{}' 不能用于 {}", op, v.type_name()),
                        *pos,
                    )),
                }
            }
            Expr::Binary { op, lhs, rhs, pos } => self.eval_binary(op, lhs, rhs, env, *pos),
            Expr::Index { obj, idx, pos } => {
                let o = self.eval(obj, env.clone())?;
                let i = self.eval(idx, env)?;
                match (&o, i) {
                    (Value::Arr(a), Value::Num(n)) => {
                        let len = a.borrow().len();
                        let idx = n as usize;
                        if n < 0.0 || idx >= len {
                            Err(VglError::new(
                                format!("数组索引 {} 越界（长度 {}）", fmt_num(n), len),
                                *pos,
                            ))
                        } else {
                            Ok(a.borrow()[idx].clone())
                        }
                    }
                    (Value::Str(s), Value::Num(n)) => {
                        let chars: Vec<char> = s.chars().collect();
                        let idx = n as usize;
                        if n < 0.0 || idx >= chars.len() {
                            Err(VglError::new(
                                format!("字符串索引 {} 越界（长度 {}）", fmt_num(n), chars.len()),
                                *pos,
                            ))
                        } else {
                            Ok(Value::Str(chars[idx].to_string()))
                        }
                    }
                    (o, i) => Err(VglError::new(
                        format!("不能用 {} 索引 {}", i.type_name(), o.type_name()),
                        *pos,
                    )),
                }
            }
            Expr::Call { name, args, named, pos } => {
                // 用户函数优先（可覆盖内建）
                if let Some(Value::Fn(f)) = env_visibility(&env, name) {
                    let mut argv = Vec::new();
                    for a in args {
                        argv.push(self.eval(a, env.clone())?);
                    }
                    let mut named_v = Vec::new();
                    for (n, e, p) in named {
                        named_v.push((n.clone(), self.eval(e, env.clone())?, *p));
                    }
                    return self.call_user(f, argv, named_v, *pos);
                }
                let mut argv = Vec::new();
                for a in args {
                    argv.push(self.eval(a, env.clone())?);
                }
                let mut named_v = Vec::new();
                for (n, e, p) in named {
                    named_v.push((n.clone(), self.eval(e, env.clone())?, *p));
                }
                self.call_builtin(name, argv, named_v, *pos)
            }
        }
    }

    fn eval_binary(
        &mut self,
        op: &str,
        lhs: &Expr,
        rhs: &Expr,
        env: Rc<RefCell<Env>>,
        pos: usize,
    ) -> VglResult<Value> {
        // 短路逻辑
        if op == "and" || op == "or" {
            let lv = self.eval(lhs, env.clone())?;
            let l = self.truthy(lv, lhs.pos())?;
            let short = if op == "and" { !l } else { l };
            if short {
                return Ok(Value::Bool(short == (op == "or")));
            }
            let rv = self.eval(rhs, env)?;
            let r = self.truthy(rv, rhs.pos())?;
            return Ok(Value::Bool(r));
        }
        let l = self.eval(lhs, env.clone())?;
        let r = self.eval(rhs, env)?;
        match (op, &l, &r) {
            ("+", Value::Num(a), Value::Num(b)) => Ok(Value::Num(a + b)),
            ("+", Value::Str(a), _) => Ok(Value::Str(format!("{}{}", a, display(&r)))),
            ("+", _, Value::Str(b)) => Ok(Value::Str(format!("{}{}", display(&l), b))),
            ("-", Value::Num(a), Value::Num(b)) => Ok(Value::Num(a - b)),
            ("*", Value::Num(a), Value::Num(b)) => Ok(Value::Num(a * b)),
            ("/", Value::Num(a), Value::Num(b)) => {
                if *b == 0.0 {
                    Err(VglError::new("除以零", pos))
                } else {
                    Ok(Value::Num(a / b))
                }
            }
            ("%", Value::Num(a), Value::Num(b)) => {
                if *b == 0.0 {
                    Err(VglError::new("对零取模", pos))
                } else {
                    Ok(Value::Num(a % b))
                }
            }
            ("==" , _, _) => Ok(Value::Bool(values_eq(&l, &r))),
            ("!=" , _, _) => Ok(Value::Bool(!values_eq(&l, &r))),
            ("<" | "<=" | ">" | ">=", Value::Num(a), Value::Num(b)) => Ok(Value::Bool(match op {
                "<" => a < b,
                "<=" => a <= b,
                ">" => a > b,
                _ => a >= b,
            })),
            ("<" | "<=" | ">" | ">=", Value::Str(a), Value::Str(b)) => Ok(Value::Bool(match op {
                "<" => a < b,
                "<=" => a <= b,
                ">" => a > b,
                _ => a >= b,
            })),
            (op, l, r) => Err(VglError::new(
                format!("不支持 {} {} {}", l.type_name(), op, r.type_name()),
                pos,
            )),
        }
    }

    fn truthy(&self, v: Value, pos: usize) -> VglResult<bool> {
        match v {
            Value::Bool(b) => Ok(b),
            other => Err(VglError::new(
                format!("条件需要 bool，得到 {}", other.type_name()),
                pos,
            )),
        }
    }

    fn num(&self, v: Value, pos: usize, what: &str) -> VglResult<f64> {
        match v {
            Value::Num(n) => Ok(n),
            other => Err(VglError::new(
                format!("{} 需要数字，得到 {}", what, other.type_name()),
                pos,
            )),
        }
    }

    // ==================== 用户函数调用 ====================

    fn call_user(
        &mut self,
        f: Rc<FnDef>,
        args: Vec<Value>,
        named: Vec<(String, Value, usize)>,
        pos: usize,
    ) -> VglResult<Value> {
        if self.depth >= MAX_DEPTH {
            return Err(VglError::new(
                format!("调用深度超限（{} 层），检查无限递归", MAX_DEPTH),
                pos,
            ));
        }
        let env = Rc::new(RefCell::new(Env::new(Some(f.env.clone()))));
        let n_params = f.params.len();
        if args.len() > n_params {
            return Err(VglError::new(
                format!("函数 {} 最多接受 {} 个参数，得到 {}", f.name, n_params, args.len()),
                pos,
            ));
        }
        let mut filled: Vec<Option<Value>> = vec![None; n_params];
        for (i, a) in args.into_iter().enumerate() {
            filled[i] = Some(a);
        }
        for (name, v, npos) in named {
            let idx = f.params.iter().position(|(p, _)| *p == name);
            match idx {
                Some(i) => {
                    if filled[i].is_some() {
                        return Err(VglError::new(
                            format!("参数 '{}' 同时以位置和命名传入", name),
                            npos,
                        ));
                    }
                    filled[i] = Some(v);
                }
                None => {
                    return Err(VglError::new(
                        format!("函数 {} 没有参数 '{}'", f.name, name),
                        npos,
                    ))
                }
            }
        }
        for (i, (pname, default)) in f.params.iter().enumerate() {
            if filled[i].is_none() {
                match default {
                    Some(d) => filled[i] = Some(self.eval(d, env.clone())?),
                    None => {
                        return Err(VglError::new(
                            format!("函数 {} 缺少参数 '{}'", f.name, pname),
                            pos,
                        ))
                    }
                }
            }
        }
        for ((pname, _), v) in f.params.iter().zip(filled.into_iter()) {
            Env::define(&env, pname, v.unwrap());
        }
        self.depth += 1;
        let result = self.exec_block(&f.body, env);
        self.depth -= 1;
        match result? {
            Control::Return(v) => Ok(v),
            _ => Ok(Value::None),
        }
    }

    // ==================== group 构建 ====================

    fn build_group(
        &mut self,
        named: &mut Vec<(String, Value, usize)>,
        pos: usize,
    ) -> VglResult<Element> {
        let mut g = Element::new("g");
        let mut transform = String::new();
        for key in ["translate", "rotate", "scale"] {
            if let Some(v) = take_named(named, key) {
                match key {
                    "translate" | "scale" => {
                        let nums = self.flat_nums(&v.0, pos, key)?;
                        let s = match nums.as_slice() {
                            [a, b] => format!("{}({},{})", key, fmt_num(*a), fmt_num(*b)),
                            [a] => format!("{}({})", key, fmt_num(*a)),
                            _ => {
                                return Err(VglError::new(
                                    format!("{} 需要 [x, y] 数组", key),
                                    pos,
                                ))
                            }
                        };
                        transform.push_str(&s);
                        transform.push(' ');
                    }
                    _ => {
                        let d = self.num(v.0, pos, "rotate")?;
                        transform.push_str(&format!("rotate({}) ", fmt_num(d)));
                    }
                }
            }
        }
        if !transform.is_empty() {
            g.set("transform", transform.trim_end());
        }
        if let Some((v, p)) = take_named(named, "opacity") {
            let o = self.num(v, p, "opacity")?;
            g.set("opacity", fmt_num(o));
        }
        if let Some((v, p)) = take_named(named, "blur") {
            let b = self.num(v, p, "blur")?;
            if b > 0.0 {
                let id = self.blur_id(b);
                g.set("filter", format!("url(#{})", id));
            }
        }
        reject_unknown(named, "group", &["translate", "rotate", "scale", "opacity", "blur"], pos)?;
        Ok(g)
    }

    // ==================== defs 注册 ====================

    fn grad_id(&mut self, spec: &GradSpec) -> String {
        let key = format!("{:?}", spec);
        if let Some(id) = self.def_map.get(&key) {
            return id.clone();
        }
        let id = format!("g{}", self.next_def);
        self.next_def += 1;
        let mut stops = String::new();
        for (c, off) in &spec.stops {
            stops.push_str(&format!(
                "<stop offset=\"{}\" stop-color=\"{}\"{}/>",
                fmt_num(*off * 100.0),
                c.hex(),
                if c.a < 1.0 { format!(" stop-opacity=\"{}\"", fmt_num(c.a)) } else { String::new() }
            ));
        }
        let xml = match spec.kind {
            GradKind::Linear => {
                let c = &spec.coords;
                format!(
                    "<linearGradient id=\"{}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{}</linearGradient>",
                    id, fmt_num(c[0]), fmt_num(c[1]), fmt_num(c[2]), fmt_num(c[3]), stops
                )
            }
            GradKind::Radial => {
                let c = &spec.coords;
                format!(
                    "<radialGradient id=\"{}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" cy=\"{}\" r=\"{}\">{}</radialGradient>",
                    id, fmt_num(c[0]), fmt_num(c[1]), fmt_num(c[2]), stops
                )
            }
        };
        self.scene.defs.push(xml);
        self.def_map.insert(key, id.clone());
        id
    }

    fn blur_id(&mut self, radius: f64) -> String {
        let key = format!("blur:{}", radius);
        if let Some(id) = self.def_map.get(&key) {
            return id.clone();
        }
        let id = format!("b{}", self.next_def);
        self.next_def += 1;
        self.scene.defs.push(format!(
            "<filter id=\"{}\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\"><feGaussianBlur stdDeviation=\"{}\"/></filter>",
            id,
            fmt_num(radius)
        ));
        self.def_map.insert(key, id.clone());
        id
    }

    // ==================== 绘图参数处理 ====================

    /// 颜色/渐变值 → SVG 属性值；返回 (attr, 额外透明度)
    fn paint(&mut self, v: &Value, what: &str, pos: usize) -> VglResult<(String, Option<f64>)> {
        match v {
            Value::None => Ok(("none".to_string(), None)),
            Value::Color(c) => Ok((c.hex(), if c.a < 1.0 { Some(c.a) } else { None })),
            Value::Grad(g) => {
                let id = self.grad_id(g);
                Ok((format!("url(#{})", id), None))
            }
            Value::Str(s) => {
                // 允许 CSS 颜色名 / #hex / rgb() 字符串
                if s.starts_with('#') && parse_hex_color(s).is_none() {
                    return Err(VglError::new(format!("{} 非法颜色 '{}'", what, s), pos));
                }
                Ok((s.clone(), None))
            }
            other => Err(VglError::new(
                format!("{} 需要颜色/渐变/none，得到 {}", what, other.type_name()),
                pos,
            )),
        }
    }

    fn flat_nums(&self, v: &Value, pos: usize, what: &str) -> VglResult<Vec<f64>> {
        match v {
            Value::Num(n) => Ok(vec![*n]),
            Value::Arr(a) => {
                let mut out = Vec::new();
                for item in a.borrow().iter() {
                    match item {
                        Value::Num(n) => out.push(*n),
                        Value::Arr(_) => out.extend(self.flat_nums(item, pos, what)?),
                        other => {
                            return Err(VglError::new(
                                format!("{} 数组内应为数字，得到 {}", what, other.type_name()),
                                pos,
                            ))
                        }
                    }
                }
                Ok(out)
            }
            other => Err(VglError::new(
                format!("{} 需要数字数组，得到 {}", what, other.type_name()),
                pos,
            )),
        }
    }

    /// 提取形状通用命名参数（fill/stroke/stroke_width/opacity/blur/cap/join），
    /// 返回 (fill属性, fill透明度, stroke属性, stroke透明度, stroke_width, opacity, blur, cap, join)
    #[allow(clippy::type_complexity)]
    fn shape_common(
        &mut self,
        named: &mut Vec<(String, Value, usize)>,
        default_fill: bool,
        pos: usize,
    ) -> VglResult<(String, Option<f64>, String, Option<f64>, f64, f64, f64, String, String)> {
        let fill_v = take_named(named, "fill").map(|(v, _)| v);
        let stroke_v = take_named(named, "stroke").map(|(v, _)| v);
        // stroke_width 的别名: width（对 line/polyline 更自然）
        let stroke_w = match take_named(named, "stroke_width") {
            Some((v, p)) => self.num(v, p, "stroke_width")?,
            None => match take_named(named, "width") {
                Some((v, p)) => self.num(v, p, "width")?,
                None => 1.0,
            },
        };
        let opacity = match take_named(named, "opacity") {
            Some((v, p)) => self.num(v, p, "opacity")?,
            None => 1.0,
        };
        let blur = match take_named(named, "blur") {
            Some((v, p)) => self.num(v, p, "blur")?,
            None => 0.0,
        };
        let cap = match take_named(named, "cap") {
            Some((Value::Str(s), _)) => s,
            Some((v, p)) => return Err(VglError::new(format!("cap 需要字符串，得到 {}", v.type_name()), p)),
            None => "butt".to_string(),
        };
        let join = match take_named(named, "join") {
            Some((Value::Str(s), _)) => s,
            Some((v, p)) => return Err(VglError::new(format!("join 需要字符串，得到 {}", v.type_name()), p)),
            None => "miter".to_string(),
        };
        // fill 规则: 显式传 fill 用之；否则若传了 stroke 就不填充，封闭形状默认黑填充
        let (fill, fill_op) = match fill_v {
            Some(v) => self.paint(&v, "fill", pos)?,
            None => {
                if stroke_v.is_some() || !default_fill {
                    ("none".to_string(), None)
                } else {
                    ("#000000".to_string(), None)
                }
            }
        };
        let (stroke, stroke_op) = match stroke_v {
            Some(v) => self.paint(&v, "stroke", pos)?,
            None => ("none".to_string(), None),
        };
        reject_unknown(
            named,
            "形状",
            &["fill", "stroke", "stroke_width", "width", "opacity", "blur", "cap", "join"],
            pos,
        )?;
        Ok((fill, fill_op, stroke, stroke_op, stroke_w, opacity, blur, cap, join))
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_shape(
        &mut self,
        mut el: Element,
        fill: String,
        fill_op: Option<f64>,
        stroke: String,
        stroke_op: Option<f64>,
        stroke_w: f64,
        opacity: f64,
        blur: f64,
        cap: String,
        join: String,
    ) -> Element {
        if fill != "none" {
            el.set("fill", fill);
            if let Some(o) = fill_op {
                el.set("fill-opacity", fmt_num(o));
            }
        } else if stroke != "none" {
            el.set("fill", "none");
        }
        if stroke != "none" {
            el.set("stroke", stroke);
            el.set("stroke-width", fmt_num(stroke_w));
            if let Some(o) = stroke_op {
                el.set("stroke-opacity", fmt_num(o));
            }
            if cap != "butt" {
                el.set("stroke-linecap", cap);
            }
            if join != "miter" {
                el.set("stroke-linejoin", join);
            }
        }
        if opacity < 1.0 {
            el.set("opacity", fmt_num(opacity));
        }
        if blur > 0.0 {
            let id = self.blur_id(blur);
            el.set("filter", format!("url(#{})", id));
        }
        el
    }

    // ==================== 内建函数 ====================

    fn call_builtin(
        &mut self,
        name: &str,
        mut args: Vec<Value>,
        mut named: Vec<(String, Value, usize)>,
        pos: usize,
    ) -> VglResult<Value> {
        match name {
            // ---------- 绘制 ----------
            "background" => {
                self.arity(&args, 1, name, pos)?;
                let (fill, fo) = self.paint(&args.remove(0), "fill", pos)?;
                let el = Element::new("rect")
                    .attr("x", "0")
                    .attr("y", "0")
                    .attr("width", fmt_num(self.scene.width))
                    .attr("height", fmt_num(self.scene.height));
                let el = self.finish_shape(el, fill, fo, "none".into(), None, 0.0, 1.0, 0.0, String::new(), String::new());
                self.scene.emit(el);
                Ok(Value::None)
            }
            "rect" => {
                self.arity_range(&args, 4, 4, name, pos)?;
                let x = self.num(args[0].clone(), pos, "x")?;
                let y = self.num(args[1].clone(), pos, "y")?;
                let w = self.num(args[2].clone(), pos, "width")?;
                let h = self.num(args[3].clone(), pos, "height")?;
                let rx = match take_named(&mut named, "rx") {
                    Some((v, p)) => self.num(v, p, "rx")?,
                    None => 0.0,
                };
                reject_unknown(&mut named, "rect", &["rx", "fill", "stroke", "stroke_width", "width", "opacity", "blur", "cap", "join"], pos)?;
                let (f, fo, s, so, sw, o, b, cap, join) = self.shape_common_named(&mut named, true, pos)?;
                let el = Element::new("rect")
                    .attr("x", fmt_num(x))
                    .attr("y", fmt_num(y))
                    .attr("width", fmt_num(w))
                    .attr("height", fmt_num(h));
                let el = if rx > 0.0 { el.attr("rx", fmt_num(rx)) } else { el };
                let el = self.finish_shape(el, f, fo, s, so, sw, o, b, cap, join);
                self.scene.emit(el);
                Ok(Value::None)
            }
            "circle" => {
                self.arity_range(&args, 3, 3, name, pos)?;
                let cx = self.num(args[0].clone(), pos, "cx")?;
                let cy = self.num(args[1].clone(), pos, "cy")?;
                let r = self.num(args[2].clone(), pos, "r")?;
                let (f, fo, s, so, sw, o, b, cap, join) = self.shape_common_named(&mut named, true, pos)?;
                let el = Element::new("circle")
                    .attr("cx", fmt_num(cx))
                    .attr("cy", fmt_num(cy))
                    .attr("r", fmt_num(r));
                let el = self.finish_shape(el, f, fo, s, so, sw, o, b, cap, join);
                self.scene.emit(el);
                Ok(Value::None)
            }
            "ellipse" => {
                self.arity_range(&args, 4, 4, name, pos)?;
                let cx = self.num(args[0].clone(), pos, "cx")?;
                let cy = self.num(args[1].clone(), pos, "cy")?;
                let rx = self.num(args[2].clone(), pos, "rx")?;
                let ry = self.num(args[3].clone(), pos, "ry")?;
                let (f, fo, s, so, sw, o, b, cap, join) = self.shape_common_named(&mut named, true, pos)?;
                let el = Element::new("ellipse")
                    .attr("cx", fmt_num(cx))
                    .attr("cy", fmt_num(cy))
                    .attr("rx", fmt_num(rx))
                    .attr("ry", fmt_num(ry));
                let el = self.finish_shape(el, f, fo, s, so, sw, o, b, cap, join);
                self.scene.emit(el);
                Ok(Value::None)
            }
            "line" => {
                self.arity_range(&args, 4, 4, name, pos)?;
                let x1 = self.num(args[0].clone(), pos, "x1")?;
                let y1 = self.num(args[1].clone(), pos, "y1")?;
                let x2 = self.num(args[2].clone(), pos, "x2")?;
                let y2 = self.num(args[3].clone(), pos, "y2")?;
                let (f, fo, mut s, so, sw, o, b, cap, join) = self.shape_common_named(&mut named, false, pos)?;
                if s == "none" && f == "none" {
                    s = "#000000".to_string(); // line 默认可见
                }
                let el = Element::new("line")
                    .attr("x1", fmt_num(x1))
                    .attr("y1", fmt_num(y1))
                    .attr("x2", fmt_num(x2))
                    .attr("y2", fmt_num(y2));
                let el = self.finish_shape(el, f, fo, s, so, sw, o, b, cap, join);
                self.scene.emit(el);
                Ok(Value::None)
            }
            "polygon" | "polyline" => {
                self.arity_range(&args, 1, 1, name, pos)?;
                let pts = self.flat_nums(&args[0], pos, "points")?;
                if pts.len() < 4 || pts.len() % 2 != 0 {
                    return Err(VglError::new(
                        format!("points 需要偶数个坐标（至少 2 个点），得到 {} 个", pts.len()),
                        pos,
                    ));
                }
                let s: String = pts
                    .chunks(2)
                    .map(|c| format!("{},{}", fmt_num(c[0]), fmt_num(c[1])))
                    .collect::<Vec<_>>()
                    .join(" ");
                let (f, fo, st, so, sw, o, b, cap, join) =
                    self.shape_common_named(&mut named, name == "polygon", pos)?;
                let el = Element::new(if name == "polygon" { "polygon" } else { "polyline" })
                    .attr("points", s);
                let el = self.finish_shape(el, f, fo, st, so, sw, o, b, cap, join);
                self.scene.emit(el);
                Ok(Value::None)
            }
            "path" => {
                self.arity_range(&args, 1, 1, name, pos)?;
                let d = match args.remove(0) {
                    Value::Str(s) => s,
                    other => {
                        return Err(VglError::new(
                            format!("path 需要 SVG path 数据字符串，得到 {}", other.type_name()),
                            pos,
                        ))
                    }
                };
                let (f, fo, st, so, sw, o, b, cap, join) = self.shape_common_named(&mut named, false, pos)?;
                let el = Element::new("path").attr("d", d);
                let el = self.finish_shape(el, f, fo, st, so, sw, o, b, cap, join);
                self.scene.emit(el);
                Ok(Value::None)
            }
            "text" => {
                self.arity_range(&args, 3, 3, name, pos)?;
                let x = self.num(args[0].clone(), pos, "x")?;
                let y = self.num(args[1].clone(), pos, "y")?;
                let content = match args[2].clone() {
                    Value::Str(s) => s,
                    other => display(&other),
                };
                let size = match take_named(&mut named, "size") {
                    Some((v, p)) => self.num(v, p, "size")?,
                    None => 16.0,
                };
                let font = take_named(&mut named, "font").map(|(v, _)| match v {
                    Value::Str(s) => s,
                    _ => "sans-serif".to_string(),
                }).unwrap_or_else(|| "sans-serif".to_string());
                let weight = take_named(&mut named, "weight").map(|(v, _)| match v {
                    Value::Str(s) => s,
                    _ => "normal".to_string(),
                }).unwrap_or_else(|| "normal".to_string());
                let anchor = take_named(&mut named, "anchor").map(|(v, _)| match v {
                    Value::Str(s) => s,
                    _ => "start".to_string(),
                }).unwrap_or_else(|| "start".to_string());
                reject_unknown(&mut named, "text", &["size", "font", "weight", "anchor", "fill", "stroke", "stroke_width", "width", "opacity", "blur", "cap", "join"], pos)?;
                let (f, fo, st, so, sw, o, b, _cap, _join) = self.shape_common_named(&mut named, true, pos)?;
                let mut el = Element::new("text")
                    .attr("x", fmt_num(x))
                    .attr("y", fmt_num(y))
                    .attr("font-size", fmt_num(size))
                    .attr("font-family", font)
                    .attr("font-weight", weight)
                    .attr("text-anchor", anchor);
                el.text = Some(content);
                let el = self.finish_shape(el, f, fo, st, so, sw, o, b, String::new(), String::new());
                self.scene.emit(el);
                Ok(Value::None)
            }

            // ---------- 渐变 ----------
            "linear_gradient" => {
                self.arity_range(&args, 1, 1, name, pos)?;
                let stops = self.parse_stops(&args[0], pos)?;
                let mut d = |k: &str, default: f64| -> VglResult<f64> {
                    match take_named(&mut named, k) {
                        Some((v, p)) => self.num(v, p, k),
                        None => Ok(default),
                    }
                };
                let x1 = d("x1", 0.0)?;
                let y1 = d("y1", 0.0)?;
                let x2 = d("x2", 0.0)?;
                let y2 = d("y2", self.scene.height)?;
                reject_unknown(&mut named, "linear_gradient", &["x1", "y1", "x2", "y2"], pos)?;
                Ok(Value::Grad(Rc::new(GradSpec {
                    kind: GradKind::Linear,
                    coords: vec![x1, y1, x2, y2],
                    stops,
                })))
            }
            "radial_gradient" => {
                self.arity_range(&args, 1, 1, name, pos)?;
                let stops = self.parse_stops(&args[0], pos)?;
                let mut d = |k: &str, default: f64| -> VglResult<f64> {
                    match take_named(&mut named, k) {
                        Some((v, p)) => self.num(v, p, k),
                        None => Ok(default),
                    }
                };
                let cx = d("cx", self.scene.width / 2.0)?;
                let cy = d("cy", self.scene.height / 2.0)?;
                let r = d("r", self.scene.width.min(self.scene.height) / 2.0)?;
                reject_unknown(&mut named, "radial_gradient", &["cx", "cy", "r"], pos)?;
                Ok(Value::Grad(Rc::new(GradSpec {
                    kind: GradKind::Radial,
                    coords: vec![cx, cy, r],
                    stops,
                })))
            }

            // ---------- 路径工具 ----------
            "smooth" => {
                self.arity_range(&args, 1, 1, name, pos)?;
                let nums = self.flat_nums(&args[0], pos, "points")?;
                let closed = match take_named(&mut named, "closed") {
                    Some((Value::Bool(b), _)) => b,
                    Some((v, p)) => {
                        return Err(VglError::new(format!("closed 需要 bool，得到 {}", v.type_name()), p))
                    }
                    None => false,
                };
                reject_unknown(&mut named, "smooth", &["closed"], pos)?;
                Ok(Value::Str(catmull_rom_d(&nums, closed)))
            }

            // ---------- 数学 ----------
            "sin" => unary_f(args, name, pos, |x| x.sin()),
            "cos" => unary_f(args, name, pos, |x| x.cos()),
            "tan" => unary_f(args, name, pos, |x| x.tan()),
            "atan2" => {
                self.arity_range(&args, 2, 2, name, pos)?;
                let y = self.num(args[0].clone(), pos, "y")?;
                let x = self.num(args[1].clone(), pos, "x")?;
                Ok(Value::Num(y.atan2(x)))
            }
            "abs" => unary_f(args, name, pos, |x| x.abs()),
            "floor" => unary_f(args, name, pos, |x| x.floor()),
            "ceil" => unary_f(args, name, pos, |x| x.ceil()),
            "round" => unary_f(args, name, pos, |x| x.round()),
            "sqrt" => unary_f(args, name, pos, |x| x.sqrt()),
            "exp" => unary_f(args, name, pos, |x| x.exp()),
            "log" => unary_f(args, name, pos, |x| x.ln()),
            "pow" => {
                self.arity_range(&args, 2, 2, name, pos)?;
                let a = self.num(args[0].clone(), pos, "底数")?;
                let b = self.num(args[1].clone(), pos, "指数")?;
                Ok(Value::Num(a.powf(b)))
            }
            "min" => {
                self.arity_range(&args, 2, 2, name, pos)?;
                let a = self.num(args[0].clone(), pos, "min")?;
                let b = self.num(args[1].clone(), pos, "min")?;
                Ok(Value::Num(a.min(b)))
            }
            "max" => {
                self.arity_range(&args, 2, 2, name, pos)?;
                let a = self.num(args[0].clone(), pos, "max")?;
                let b = self.num(args[1].clone(), pos, "max")?;
                Ok(Value::Num(a.max(b)))
            }
            "clamp" => {
                self.arity_range(&args, 3, 3, name, pos)?;
                let v = self.num(args[0].clone(), pos, "值")?;
                let lo = self.num(args[1].clone(), pos, "下界")?;
                let hi = self.num(args[2].clone(), pos, "上界")?;
                Ok(Value::Num(v.clamp(lo.min(hi), lo.max(hi))))
            }
            "lerp" => {
                self.arity_range(&args, 3, 3, name, pos)?;
                let a = self.num(args[0].clone(), pos, "lerp")?;
                let b = self.num(args[1].clone(), pos, "lerp")?;
                let t = self.num(args[2].clone(), pos, "lerp")?;
                Ok(Value::Num(a + (b - a) * t))
            }

            // ---------- 随机与噪声 ----------
            "rand" => {
                let a = args.first().cloned().map(|v| self.num(v, pos, "rand")).transpose()?;
                let b = args.get(1).cloned().map(|v| self.num(v, pos, "rand")).transpose()?;
                let (a, b) = (a.unwrap_or(0.0), b.unwrap_or(1.0));
                Ok(Value::Num(self.rng.range(a, b)))
            }
            "rand_int" => {
                self.arity_range(&args, 2, 2, name, pos)?;
                let a = self.num(args[0].clone(), pos, "rand_int")?.round();
                let b = self.num(args[1].clone(), pos, "rand_int")?.round();
                let v = self.rng.range(a, b + 1.0).floor();
                Ok(Value::Num(v.max(a).min(b)))
            }
            "perlin" => {
                self.arity_range(&args, 2, 2, name, pos)?;
                let x = self.num(args[0].clone(), pos, "x")?;
                let y = self.num(args[1].clone(), pos, "y")?;
                Ok(Value::Num(perlin(x, y, &self.perm)))
            }
            "fbm" => {
                self.arity_range(&args, 2, 3, name, pos)?;
                let x = self.num(args[0].clone(), pos, "x")?;
                let y = self.num(args[1].clone(), pos, "y")?;
                let oct = match args.get(2) {
                    Some(v) => self.num(v.clone(), pos, "octaves")? as i32,
                    None => 4,
                };
                Ok(Value::Num(fbm(x, y, oct, &self.perm)))
            }

            // ---------- 颜色 ----------
            "color" => {
                if args.len() == 1 {
                    let s = match &args[0] {
                        Value::Str(s) => s.clone(),
                        other => {
                            return Err(VglError::new(
                                format!("color 单参数形式需要 \"#hex\" 字符串，得到 {}", other.type_name()),
                                pos,
                            ))
                        }
                    };
                    return parse_hex_color(&s)
                        .map(Value::Color)
                        .ok_or_else(|| VglError::new(format!("非法颜色 '{}'", s), pos));
                }
                self.arity_range(&args, 3, 4, name, pos)?;
                let r = self.num(args[0].clone(), pos, "r")?;
                let g = self.num(args[1].clone(), pos, "g")?;
                let b = self.num(args[2].clone(), pos, "b")?;
                let a = match args.get(3) {
                    Some(v) => self.num(v.clone(), pos, "alpha")?,
                    None => 1.0,
                };
                Ok(Value::Color(Color::new(r, g, b, a)))
            }
            "lighten" | "darken" => {
                self.arity_range(&args, 2, 2, name, pos)?;
                let c = self.color_arg(&args[0], pos)?;
                let amt = self.num(args[1].clone(), pos, "amount")?.clamp(0.0, 1.0);
                let target = if name == "lighten" { 255.0 } else { 0.0 };
                let mix = |v: f64| v + (target - v) * amt;
                Ok(Value::Color(Color::new(mix(c.r), mix(c.g), mix(c.b), c.a)))
            }
            "lerp_color" => {
                self.arity_range(&args, 3, 3, name, pos)?;
                let a = self.color_arg(&args[0], pos)?;
                let b = self.color_arg(&args[1], pos)?;
                let t = self.num(args[2].clone(), pos, "t")?;
                let m = |x, y| x + (y - x) * t;
                Ok(Value::Color(Color::new(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b), m(a.a, b.a))))
            }
            "alpha" => {
                self.arity_range(&args, 2, 2, name, pos)?;
                let c = self.color_arg(&args[0], pos)?;
                let a = self.num(args[1].clone(), pos, "alpha")?;
                Ok(Value::Color(Color::new(c.r, c.g, c.b, a)))
            }
            "red" | "green" | "blue" => {
                self.arity_range(&args, 1, 1, name, pos)?;
                let c = self.color_arg(&args[0], pos)?;
                Ok(Value::Num(match name {
                    "red" => c.r,
                    "green" => c.g,
                    _ => c.b,
                }))
            }

            // ---------- 画布与工具 ----------
            "width" => Ok(Value::Num(self.scene.width)),
            "height" => Ok(Value::Num(self.scene.height)),
            "len" => {
                self.arity_range(&args, 1, 1, name, pos)?;
                match &args[0] {
                    Value::Arr(a) => Ok(Value::Num(a.borrow().len() as f64)),
                    Value::Str(s) => Ok(Value::Num(s.chars().count() as f64)),
                    other => Err(VglError::new(
                        format!("len 需要数组或字符串，得到 {}", other.type_name()),
                        pos,
                    )),
                }
            }
            "push" => {
                self.arity_range(&args, 2, 2, name, pos)?;
                match &args[0] {
                    Value::Arr(a) => {
                        a.borrow_mut().push(args[1].clone());
                        Ok(Value::None)
                    }
                    other => Err(VglError::new(
                        format!("push 第一个参数需要数组，得到 {}", other.type_name()),
                        pos,
                    )),
                }
            }
            "print" => {
                self.arity_range(&args, 1, 1, name, pos)?;
                eprintln!("{}", display(&args[0]));
                Ok(Value::None)
            }
            _ => Err(VglError::new(format!("未定义的函数 '{}'", name), pos)),
        }
    }

    /// 从命名参数提取形状通用参数（shape_common 的入口包装，检查未知命名参数由调用方完成）
    fn shape_common_named(
        &mut self,
        named: &mut Vec<(String, Value, usize)>,
        default_fill: bool,
        pos: usize,
    ) -> VglResult<(String, Option<f64>, String, Option<f64>, f64, f64, f64, String, String)> {
        self.shape_common(named, default_fill, pos)
    }

    fn parse_stops(&mut self, v: &Value, pos: usize) -> VglResult<Vec<(Color, f64)>> {
        let arr = match v {
            Value::Arr(a) => a.borrow().clone(),
            other => {
                return Err(VglError::new(
                    format!("渐变 stops 需要颜色数组，得到 {}", other.type_name()),
                    pos,
                ))
            }
        };
        if arr.is_empty() {
            return Err(VglError::new("渐变至少需要一个颜色", pos));
        }
        // 先收集 (颜色, 可选偏移)
        let mut raw: Vec<(Color, Option<f64>)> = Vec::new();
        for item in &arr {
            match item {
                Value::Color(c) => raw.push((*c, None)),
                Value::Str(s) => {
                    let c = parse_hex_color(s)
                        .ok_or_else(|| VglError::new(format!("非法颜色 '{}'", s), pos))?;
                    raw.push((c, None));
                }
                Value::Arr(pair) => {
                    // [color, offset]
                    if pair.borrow().len() != 2 {
                        return Err(VglError::new("渐变 stop 二元组应为 [颜色, 偏移]", pos));
                    }
                    let c = self.color_arg(&pair.borrow()[0], pos)?;
                    let off = self.num(pair.borrow()[1].clone(), pos, "偏移")?;
                    raw.push((c, Some(off)));
                }
                other => {
                    return Err(VglError::new(
                        format!("渐变 stop 应为颜色或 [颜色, 偏移]，得到 {}", other.type_name()),
                        pos,
                    ))
                }
            }
        }
        // 未给偏移的均匀分布
        let n = raw.len();
        let mut stops = Vec::new();
        for (i, (c, off)) in raw.into_iter().enumerate() {
            let o = off.unwrap_or(if n == 1 { 0.0 } else { i as f64 / (n - 1) as f64 });
            stops.push((c, o.clamp(0.0, 1.0)));
        }
        stops.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(stops)
    }

    fn color_arg(&self, v: &Value, pos: usize) -> VglResult<Color> {
        match v {
            Value::Color(c) => Ok(*c),
            Value::Str(s) => parse_hex_color(s)
                .ok_or_else(|| VglError::new(format!("非法颜色 '{}'", s), pos)),
            other => Err(VglError::new(
                format!("需要颜色，得到 {}", other.type_name()),
                pos,
            )),
        }
    }

    fn arity(&self, args: &[Value], n: usize, fname: &str, pos: usize) -> VglResult<()> {
        if args.len() != n {
            Err(VglError::new(
                format!("{} 需要 {} 个参数，得到 {}", fname, n, args.len()),
                pos,
            ))
        } else {
            Ok(())
        }
    }

    fn arity_range(
        &self,
        args: &[Value],
        lo: usize,
        hi: usize,
        fname: &str,
        pos: usize,
    ) -> VglResult<()> {
        if args.len() < lo || args.len() > hi {
            Err(VglError::new(
                format!("{} 需要 {}~{} 个参数，得到 {}", fname, lo, hi, args.len()),
                pos,
            ))
        } else {
            Ok(())
        }
    }
}

// ==================== 辅助函数 ====================

fn take_named(named: &mut Vec<(String, Value, usize)>, key: &str) -> Option<(Value, usize)> {
    let idx = named.iter().position(|(k, _, _)| k == key)?;
    let (_, v, p) = named.remove(idx);
    Some((v, p))
}

fn reject_unknown(
    named: &[(String, Value, usize)],
    fname: &str,
    allowed: &[&str],
    pos: usize,
) -> VglResult<()> {
    for (k, _, _) in named {
        if !allowed.contains(&k.as_str()) {
            return Err(VglError::new(
                format!("{} 不支持命名参数 '{}'（可用: {}）", fname, k, allowed.join(", ")),
                pos,
            ));
        }
    }
    Ok(())
}

fn env_visibility(env: &Rc<RefCell<Env>>, name: &str) -> Option<Value> {
    Env::lookup(env, name)
}

fn unary_f(
    args: Vec<Value>,
    fname: &str,
    pos: usize,
    f: impl Fn(f64) -> f64,
) -> VglResult<Value> {
    if args.len() != 1 {
        return Err(VglError::new(format!("{} 需要 1 个参数，得到 {}", fname, args.len()), pos));
    }
    match args[0] {
        Value::Num(x) => Ok(Value::Num(f(x))),
        ref other => Err(VglError::new(
            format!("{} 需要数字，得到 {}", fname, other.type_name()),
            pos,
        )),
    }
}

fn display(v: &Value) -> String {
    match v {
        Value::Num(n) => fmt_num(*n),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => s.clone(),
        Value::Color(c) => c.hex(),
        Value::None => "none".to_string(),
        Value::Arr(a) => {
            let items: Vec<String> = a.borrow().iter().map(display).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Fn(f) => format!("<fn {}>", f.name),
        Value::Grad(_) => "<gradient>".to_string(),
    }
}

fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Color(x), Value::Color(y)) => {
            x.r == y.r && x.g == y.g && x.b == y.b && x.a == y.a
        }
        (Value::None, Value::None) => true,
        (Value::Arr(x), Value::Arr(y)) => {
            let xb = x.borrow();
            let yb = y.borrow();
            xb.len() == yb.len() && xb.iter().zip(yb.iter()).all(|(a, b)| values_eq(a, b))
        }
        (Value::Grad(x), Value::Grad(y)) => Rc::ptr_eq(x, y),
        (Value::Fn(x), Value::Fn(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

fn join_path(dir: &str, file: &str) -> String {
    if file.starts_with('/') || file.starts_with("\\") {
        return file.to_string();
    }
    let d = dir.trim_end_matches('/');
    format!("{}/{}", d, file)
}

/// Catmull-Rom 样条 → SVG path d（三次贝塞尔）
fn catmull_rom_d(nums: &[f64], closed: bool) -> String {
    let pts: Vec<(f64, f64)> = nums
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0], c[1]))
        .collect();
    let n = pts.len();
    if n < 2 {
        return String::new();
    }
    if n == 2 {
        return format!(
            "M{} {} L{} {}",
            fmt_num(pts[0].0),
            fmt_num(pts[0].1),
            fmt_num(pts[1].0),
            fmt_num(pts[1].1)
        );
    }
    let get = |i: isize| -> (f64, f64) {
        if closed {
            pts[(i.rem_euclid(n as isize)) as usize]
        } else if i < 0 {
            pts[0]
        } else if i as usize >= n {
            pts[n - 1]
        } else {
            pts[i as usize]
        }
    };
    let segs = if closed { n } else { n - 1 };
    let mut d = format!("M{} {}", fmt_num(pts[0].0), fmt_num(pts[0].1));
    for i in 0..segs {
        let p0 = get(i as isize - 1);
        let p1 = get(i as isize);
        let p2 = get(i as isize + 1);
        let p3 = get(i as isize + 2);
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        d.push_str(&format!(
            " C{} {} {} {} {} {}",
            fmt_num(c1.0),
            fmt_num(c1.1),
            fmt_num(c2.0),
            fmt_num(c2.1),
            fmt_num(p2.0),
            fmt_num(p2.1)
        ));
    }
    if closed {
        d.push_str(" Z");
    }
    d
}
