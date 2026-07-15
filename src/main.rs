// VGL 解释器 — Rust 版（v1.0 语义化生成）
// 用法:
//   vgl [--continue-on-error] <file.vgl>
//   vgl replicate --mode semantic <input.png> <output.vgl>  (推荐)
//   vgl replicate --mode pixel <input.png> <output.vgl>
//   vgl replicate --mode block <input.png> <output.vgl> [--block-size 16]
//   vgl replicate --mode progressive <input.png> <output.vgl> [--layers 32,8,1] [--threshold 30]

mod error;
mod lexer;
mod ast;
mod parser;
mod canvas;
mod noise;
mod interp;
mod replicate;

use std::cell::RefCell;
use std::env;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use image::GenericImageView;

use crate::ast::Env;
use crate::error::{format_error, VglResult};
use crate::interp::{Control, Interpreter};
use crate::lexer::Lexer;
use crate::parser::Parser;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    // 子命令分发：replicate 进入图像复刻工具链
    if args[1] == "replicate" {
        std::process::exit(run_replicate(&args[2..]));
    }

    // 默认：解释执行 .vgl 文件
    std::process::exit(run_interp(&args[1..]));
}

fn print_usage() {
    eprintln!("VGL v1.0 用法:");
    eprintln!("  vgl [--continue-on-error] <file.vgl>");
    eprintln!("  vgl replicate --mode semantic <input.png> <output.vgl>  (推荐: 语义化生成蓝图)");
    eprintln!("  vgl replicate --mode pixel <input.png> <output.vgl>");
    eprintln!("  vgl replicate --mode block <input.png> <output.vgl> [--block-size 16]");
    eprintln!("  vgl replicate --mode progressive <input.png> <output.vgl> [--layers 32,8,1] [--threshold 30]");
}

fn run_interp(args: &[String]) -> i32 {
    let mut continue_on_error = false;
    let mut filename: Option<String> = None;
    for a in args {
        if a == "--continue-on-error" {
            continue_on_error = true;
        } else if !a.starts_with("--") {
            filename = Some(a.clone());
        }
    }
    let filename = match filename {
        Some(f) => f,
        None => {
            print_usage();
            return 1;
        }
    };

    let src = match fs::read_to_string(&filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("无法读取文件 {}: {}", filename, e);
            return 1;
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
        return 1;
    }

    // 打印警告
    for w in &interp.warnings {
        let full = format_error(&w.msg, &interp.current_src, w.pos, &interp.current_filename);
        eprintln!("VGL 警告: {}", full);
    }

    if had_error {
        return 1;
    }
    0
}

fn run_replicate(args: &[String]) -> i32 {
    // vgl replicate --mode <mode> <input> <output> [options]
    let mut mode = String::new();
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut block_size: u32 = 16;
    let mut layers: Vec<u32> = vec![32, 8, 1];
    let mut threshold: u8 = 30;

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--mode" => {
                i += 1;
                if i < args.len() {
                    mode = args[i].clone();
                }
            }
            "--block-size" => {
                i += 1;
                if i < args.len() {
                    block_size = args[i].parse().unwrap_or(16);
                }
            }
            "--layers" => {
                i += 1;
                if i < args.len() {
                    layers = args[i]
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                    if layers.is_empty() {
                        layers = vec![32, 8, 1];
                    }
                }
            }
            "--threshold" => {
                i += 1;
                if i < args.len() {
                    threshold = args[i].parse().unwrap_or(30);
                }
            }
            _ => {
                if input.is_none() {
                    input = Some(a.clone());
                } else if output.is_none() {
                    output = Some(a.clone());
                }
            }
        }
        i += 1;
    }

    let input = match input {
        Some(f) => f,
        None => {
            eprintln!("错误: 缺少输入图片路径");
            print_usage();
            return 1;
        }
    };
    let output = match output {
        Some(f) => f,
        None => {
            eprintln!("错误: 缺少输出 .vgl 路径");
            print_usage();
            return 1;
        }
    };

    let img = match image::open(&input) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("无法加载图片 {}: {}", input, e);
            return 1;
        }
    };

    // pixel 模式生成 load() 调用，需要绝对路径以保证 .vgl 在任意目录运行都能找到原图
    let input_abs = fs::canonicalize(&input)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| input.clone());

    let mut code = String::new();
    match mode.as_str() {
        "semantic" => replicate::replicate_semantic(&img, &mut code),
        "pixel" => replicate::replicate_pixel(&img, &input_abs, &mut code),
        "block" => replicate::replicate_block(&img, block_size, &mut code),
        "progressive" => replicate::replicate_progressive(&img, &layers, threshold, &mut code),
        _ => {
            eprintln!("错误: 未知模式 '{}'（支持: semantic / pixel / block / progressive）", mode);
            return 1;
        }
    }

    if let Err(e) = fs::write(&output, &code) {
        eprintln!("无法写入 {}: {}", output, e);
        return 1;
    }

    let (w, h) = img.dimensions();
    eprintln!(
        "已生成: {} ({}x{}, 模式: {}, {} 字节)",
        output,
        w,
        h,
        mode,
        code.len()
    );
    0
}
