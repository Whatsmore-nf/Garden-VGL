// ============================================================
// VGL v2.0 — SVG 序列化器
// ============================================================

use crate::scene::{Element, Scene};

/// 数字格式化：最多 3 位小数，去除尾零
pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let mut s = format!("{:.3}", v);
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    s
}

pub fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 场景 → SVG 文档字符串（带缩进，对 AI 可读）
pub fn write_svg(scene: &Scene) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        fmt_num(scene.width),
        fmt_num(scene.height),
        fmt_num(scene.width),
        fmt_num(scene.height)
    ));
    if !scene.defs.is_empty() {
        out.push_str("<defs>\n");
        for d in &scene.defs {
            out.push_str("  ");
            out.push_str(d);
            out.push('\n');
        }
        out.push_str("</defs>\n");
    }
    for el in &scene.root {
        write_element(el, 1, &mut out);
    }
    out.push_str("</svg>\n");
    out
}

fn write_element(el: &Element, depth: usize, out: &mut String) {
    let ind = "  ".repeat(depth);
    out.push_str(&ind);
    out.push('<');
    out.push_str(el.tag);
    for (k, v) in &el.attrs {
        out.push_str(&format!(" {}=\"{}\"", k, escape_attr(v)));
    }
    if let Some(t) = &el.text {
        out.push('>');
        out.push_str(&escape_text(t));
        out.push_str(&format!("</{}>\n", el.tag));
        return;
    }
    if el.children.is_empty() {
        out.push_str("/>\n");
    } else {
        out.push_str(">\n");
        for c in &el.children {
            write_element(c, depth + 1, out);
        }
        out.push_str(&ind);
        out.push_str(&format!("</{}>\n", el.tag));
    }
}
