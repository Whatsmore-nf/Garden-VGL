// ============================================================
// VGL v2.0 — 错误类型与格式化
// ============================================================

#[derive(Debug, Clone)]
pub struct VglError {
    pub msg: String,
    pub pos: usize,
}

impl VglError {
    pub fn new(msg: impl Into<String>, pos: usize) -> Self {
        VglError { msg: msg.into(), pos }
    }
}

pub type VglResult<T> = Result<T, VglError>;

/// 把错误格式化为 "文件:行:列 │ 源码行 │ 指示箭头" 的可读形式
pub fn format_error(msg: &str, src: &str, pos: usize, filename: &str) -> String {
    let (line, col, line_text) = locate(src, pos);
    let mut out = format!("{}:{}:{}: {}", filename, line, col, msg);
    if !line_text.trim().is_empty() {
        out.push_str(&format!(
            "\n    {}\n    {}^",
            line_text,
            " ".repeat(col.saturating_sub(1))
        ));
    }
    out
}

fn locate(src: &str, pos: usize) -> (usize, usize, String) {
    let pos = pos.min(src.len());
    let mut line = 1usize;
    let mut col = 1usize;
    let mut line_start = 0usize;
    for (i, ch) in src.char_indices() {
        if i >= pos {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
            line_start = i + 1;
        } else {
            col += 1;
        }
    }
    let line_end = src[line_start..]
        .find('\n')
        .map(|i| line_start + i)
        .unwrap_or(src.len());
    let text = src[line_start..line_end].trim_end().to_string();
    (line, col, text)
}
