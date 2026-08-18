// ============================================================
// VGL v2.0 — 矢量场景图
// 绘图语句构建 SVG 元素树，render 时由 svg.rs 序列化
// ============================================================

/// 一个 SVG 元素节点。tag 为 SVG 标签名，attrs 保持插入顺序，
/// children 用于 <g> 嵌套，text 用于 <text> 内容。
#[derive(Debug, Clone)]
pub struct Element {
    pub tag: &'static str,
    pub attrs: Vec<(&'static str, String)>,
    pub children: Vec<Element>,
    pub text: Option<String>,
}

impl Element {
    pub fn new(tag: &'static str) -> Self {
        Element { tag, attrs: Vec::new(), children: Vec::new(), text: None }
    }

    pub fn attr(mut self, k: &'static str, v: impl Into<String>) -> Self {
        self.attrs.push((k, v.into()));
        self
    }

    /// 已有同名属性则覆盖（后设置的优先）
    pub fn set(&mut self, k: &'static str, v: impl Into<String>) {
        let v = v.into();
        if let Some(slot) = self.attrs.iter_mut().find(|(key, _)| *key == k) {
            slot.1 = v;
        } else {
            self.attrs.push((k, v));
        }
    }
}

/// 当前正在构建的场景（一次 render 一景）
#[derive(Debug, Default)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    /// 已完成的最外层元素
    pub root: Vec<Element>,
    /// 打开中的 group 栈（内层在栈顶）
    pub open: Vec<Element>,
    /// <defs> 内的渐变 / 滤镜定义片段
    pub defs: Vec<String>,
}

impl Scene {
    pub fn new() -> Self {
        Scene::default()
    }

    /// 把元素放入当前容器（打开的 group 或根）
    pub fn emit(&mut self, el: Element) {
        match self.open.last_mut() {
            Some(g) => g.children.push(el),
            None => self.root.push(el),
        }
    }

    pub fn open_group(&mut self, g: Element) {
        self.open.push(g);
    }

    pub fn close_group(&mut self) {
        if let Some(g) = self.open.pop() {
            self.emit(g);
        }
    }
}
