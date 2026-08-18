// ============================================================
// VGL v2.0 — 矢量图形语言解释器
// 用法: vgl <file.vgl>
// 渲染目标为 SVG — 精度即 SVG 精度，无限缩放
// ============================================================

mod ast;
mod error;
mod interp;
mod lexer;
mod noise;
mod parser;
mod scene;
mod svg;

use std::cell::RefCell;
use std::env;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use ast::Env;
use error::{format_error, VglResult};
use interp::{Control, Interpreter};
use lexer::Lexer;
use parser::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 || args[1].starts_with("--") {
        eprintln!("VGL v2.0 — 矢量图形语言（输出 SVG）");
        eprintln!("用法: vgl <file.vgl>");
        std::process::exit(1);
    }
    let filename = &args[1];
    std::process::exit(run(filename));
}

fn run(filename: &str) -> i32 {
    let src = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("无法读取文件 {}: {}", filename, e);
            return 1;
        }
    };
    let mut interp = Interpreter::new();
    interp.base_dir = Path::new(filename)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    interp.current_dir = interp.base_dir.clone();
    interp.current_filename = filename.to_string();
    interp.current_src = src.clone();
    if let Ok(abs) = fs::canonicalize(filename) {
        interp.imported.push(abs.to_string_lossy().to_string());
    }

    let result: VglResult<()> = (|| {
        let toks = Lexer::new(&src).tokenize()?;
        let ast = Parser::new(toks).parse_program()?;
        let global_env = Rc::new(RefCell::new(Env::new(None)));
        Interpreter::init_globals(&global_env);
        for s in &ast {
            match interp.exec(s, global_env.clone()) {
                Ok(Control::Normal) | Ok(Control::Return(_)) => {}
                Ok(Control::Break) | Ok(Control::Continue) => {
                    return Err(error::VglError::new("break/continue 泄漏到顶层", s.pos()));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    })();

    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("VGL 错误: {}", format_error(&e.msg, &interp.current_src, e.pos, &interp.current_filename));
            1
        }
    }
}
