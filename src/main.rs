// VGL 解释器 — Rust 版（v0.55 完整实现）
// 对应规范: VGL_语法规范 v0.5.txt（v0.55 修订）
// 用法: vgl <file.vgl>

mod error;
mod lexer;
mod ast;
mod parser;
mod canvas;
mod noise;
mod interp;

use std::cell::RefCell;
use std::env;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use crate::ast::Env;
use crate::error::{format_error, VglResult};
use crate::interp::{Control, Interpreter};
use crate::lexer::Lexer;
use crate::parser::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("用法: vgl [--continue-on-error] <file.vgl>");
        std::process::exit(1);
    }

    let mut continue_on_error = false;
    let mut filename: Option<String> = None;
    for a in &args[1..] {
        if a == "--continue-on-error" {
            continue_on_error = true;
        } else if !a.starts_with("--") {
            filename = Some(a.clone());
        }
    }
    let filename = match filename {
        Some(f) => f,
        None => {
            println!("用法: vgl [--continue-on-error] <file.vgl>");
            std::process::exit(1);
        }
    };

    let src = match fs::read_to_string(&filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("无法读取文件 {}: {}", filename, e);
            std::process::exit(1);
        }
    };
    let mut interp = Interpreter::new();
    interp.current_dir = Path::new(&filename)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    interp.current_filename = filename.clone();
    interp.current_src = src.clone();
    if let Ok(abs) = fs::canonicalize(&filename) {
        interp.imported.push(abs.to_string_lossy().to_string());
    }

    let mut had_error = false;
    let result: VglResult<()> = (|| {
        let mut lexer = Lexer::new(&src);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let ast = parser.parse_program()?;
        let global_env = Rc::new(RefCell::new(Env::new(None)));
        for sp in &ast {
            match interp.exec(sp, global_env.clone()) {
                Ok(Control::Normal) | Ok(Control::Return(_)) => {}
                Ok(Control::Break(_)) | Ok(Control::Continue) => {
                    if continue_on_error {
                        eprintln!("警告: 控制流信号泄漏到顶层");
                        had_error = true;
                    } else {
                        eprintln!("{}: 控制流信号泄漏到顶层", filename);
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    let full =
                        format_error(&e.msg, &interp.current_src, e.pos, &interp.current_filename);
                    eprintln!("VGL 错误: {}", full);
                    had_error = true;
                    if !continue_on_error {
                        std::process::exit(1);
                    }
                    // continue-on-error 模式：继续执行下一条语句
                }
            }
        }
        Ok(())
    })();

    // 处理 lexer/parser 错误（这些错误通过 ? 直接传播，未经过循环）
    if let Err(e) = result {
        let full = format_error(&e.msg, &interp.current_src, e.pos, &interp.current_filename);
        eprintln!("VGL 错误: {}", full);
        std::process::exit(1);
    }

    // 打印警告
    for w in &interp.warnings {
        let full = format_error(&w.msg, &interp.current_src, w.pos, &interp.current_filename);
        eprintln!("VGL 警告: {}", full);
    }

    if had_error {
        std::process::exit(1);
    }
}
