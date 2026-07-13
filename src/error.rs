pub fn clamp_u8(v: f64) -> u8 {
    if v < 0.0 {
        0
    } else if v > 255.0 {
        255
    } else {
        v as u8
    }
}

pub fn clamp_f32(v: f32) -> f32 {
    if v < 0.0 {
        0.0
    } else if v > 255.0 {
        255.0
    } else {
        v
    }
}

// ============================================================
// 错误类型（v0.5 批次 B：错误定位 §8.2）
// ============================================================

#[derive(Debug)]
pub struct VglError {
    pub msg: String,
    pub pos: Option<usize>, // 字符偏移；None 表示未知位置
}

impl VglError {
    pub fn new(msg: impl Into<String>, pos: Option<usize>) -> Self {
        VglError {
            msg: msg.into(),
            pos,
        }
    }
}

impl std::fmt::Display for VglError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

pub type VglResult<T> = Result<T, VglError>;

/// 警告类型（v0.5 批次 E：警告系统）
#[derive(Debug)]
pub struct VglWarning {
    pub msg: String,
    pub pos: Option<usize>,
}

impl VglWarning {
    pub fn new(msg: impl Into<String>, pos: Option<usize>) -> Self {
        VglWarning {
            msg: msg.into(),
            pos,
        }
    }
}

/// 字符偏移 -> (行, 列)，均 1-based
pub fn pos_to_linecol(src: &str, pos: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in src.char_indices() {
        if i >= pos {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// 格式化错误信息: filename:line:col: msg\n  <源码行>\n  <caret>
pub fn format_error(msg: &str, src: &str, pos: Option<usize>, filename: &str) -> String {
    match pos {
        None => format!("{}: {}", filename, msg),
        Some(p) => {
            let (line, col) = pos_to_linecol(src, p);
            let lines: Vec<&str> = src.split('\n').collect();
            let src_line = if line >= 1 && line <= lines.len() {
                lines[line - 1]
            } else {
                ""
            };
            let caret: String = " ".repeat(col.saturating_sub(1)) + "^";
            format!(
                "{}:{}:{}: {}\n  {}\n  {}",
                filename, line, col, msg, src_line, caret
            )
        }
    }
}
