// ============================================================
// 图像复刻工具链（v0.6）
// 三种模式：
//   - pixel: 逐像素 1:1 无损复刻（load + pixel_at）
//   - block: 分块法，每块用平均色填充
//   - progressive: 渐进法，分层细化（大块 → 中块 → 像素）
// ============================================================

use image::{DynamicImage, GenericImageView, RgbaImage};

/// 模式 A：像素级精确复刻
/// 生成紧凑的 .vgl 代码：用 load() 加载原图，循环用 pixel_at() 读取并绘制
/// 文件小（不依赖内联像素），但运行时需要原图文件存在
pub fn replicate_pixel(img: &DynamicImage, input_path: &str, out: &mut String) {
    let (w, h) = img.dimensions();
    out.push_str(&format!("// v0.6 像素法复刻：{}x{}\n", w, h));
    out.push_str(&format!("canvas {}x{}\n", w, h));
    out.push_str("bg #000000\n\n");
    out.push_str(&format!("let src = load(\"{}\")\n", escape_path(input_path)));
    out.push_str(&format!("for y in 0..{} {{\n", h));
    out.push_str(&format!("    for x in 0..{} {{\n", w));
    out.push_str("        pixel(x: x, y: y, rgb: pixel_at(src, x, y))\n");
    out.push_str("    }\n}\n\n");
    out.push_str("render \"replica_pixel.png\"\n");
}

/// 模式 C：分块法
/// 将图片分成 block_size×block_size 的块，每块用平均色填充
/// 自包含（不依赖原图），文件大小 = 块数 × 一行代码
pub fn replicate_block(img: &DynamicImage, block_size: u32, out: &mut String) {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();

    out.push_str(&format!("// v0.6 分块法复刻：{}x{}，块大小 {}\n", w, h, block_size));
    out.push_str(&format!("canvas {}x{}\n", w, h));
    out.push_str("bg #000000\n\n");

    // fill_block 函数定义
    out.push_str("fn fill_block(x0, y0, size, r, g, b) {\n");
    out.push_str("    for dy in 0..size {\n");
    out.push_str("        for dx in 0..size {\n");
    out.push_str("            pixel(x: x0 + dx, y: y0 + dy, rgb: (r, g, b))\n");
    out.push_str("        }\n    }\n}\n\n");

    // 遍历每个块，计算平均色并生成调用
    let mut by = 0u32;
    while by < h {
        let mut bx = 0u32;
        while bx < w {
            let (r, g, b, _var) = block_stats(&rgba, bx, by, block_size, w, h);
            out.push_str(&format!(
                "fill_block({}, {}, {}, {}, {}, {})\n",
                bx, by, block_size, r, g, b
            ));
            bx += block_size;
        }
        by += block_size;
    }

    out.push_str("\nrender \"replica_block.png\"\n");
}

/// 模式 D：渐进法（分层细化）
/// layers 例如 [32, 8, 1]：先 32x32 块填充，再对差异大的 8x8 块覆盖，最后对差异大的像素覆盖
/// threshold 为 0-255 的单通道差异阈值，块内最大单通道差异超过阈值则进入下一层
pub fn replicate_progressive(
    img: &DynamicImage,
    layers: &[u32],
    threshold: u8,
    out: &mut String,
) {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();

    out.push_str(&format!(
        "// v0.6 渐进法复刻：{}x{}，层 {}, 阈值 {}\n",
        w, h,
        layers.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("/"),
        threshold
    ));
    out.push_str(&format!("canvas {}x{}\n", w, h));
    out.push_str("bg #000000\n\n");

    // fill_block 函数定义
    out.push_str("fn fill_block(x0, y0, size, r, g, b) {\n");
    out.push_str("    for dy in 0..size {\n");
    out.push_str("        for dx in 0..size {\n");
    out.push_str("            pixel(x: x0 + dx, y: y0 + dy, rgb: (r, g, b))\n");
    out.push_str("        }\n    }\n}\n\n");

    // 维护"是否需要细化"的块列表
    // 每层处理：第一层全填充，后续层仅填充上一层方差 > 阈值的块
    // 用 (bx, by, bs) 三元组表示待处理的块
    let mut pending: Vec<(u32, u32, u32)> = Vec::new();

    for (layer_idx, &bs) in layers.iter().enumerate() {
        let mut next_pending: Vec<(u32, u32, u32)> = Vec::new();

        if layer_idx == 0 {
            // 第一层：所有块都处理
            let mut by = 0u32;
            while by < h {
                let mut bx = 0u32;
                while bx < w {
                    pending.push((bx, by, bs));
                    bx += bs;
                }
                by += bs;
            }
        }

        // 处理 pending 列表
        for (bx, by, parent_bs) in &pending {
            // 当前层块大小为 bs，父块大小为 parent_bs
            // 在父块范围内细分为 bs×bs 的子块
            let parent_bs = *parent_bs;
            let mut sy = *by;
            while sy < (*by + parent_bs).min(h) {
                let mut sx = *bx;
                while sx < (*bx + parent_bs).min(w) {
                    let (r, g, b, var) = block_stats(&rgba, sx, sy, bs, w, h);
                    // 第一层总是填充；后续层仅填充父块标记的（已在 pending 中）
                    if bs == 1 {
                        // 1x1 块直接用 pixel()
                        out.push_str(&format!(
                            "pixel(x: {}, y: {}, rgb: ({}, {}, {}))\n",
                            sx, sy, r, g, b
                        ));
                    } else {
                        out.push_str(&format!(
                            "fill_block({}, {}, {}, {}, {}, {})\n",
                            sx, sy, bs, r, g, b
                        ));
                    }
                    // 如果块内方差超过阈值，标记为下一层待处理
                    if var > threshold && layer_idx + 1 < layers.len() {
                        next_pending.push((sx, sy, bs));
                    }
                    sx += bs;
                }
                sy += bs;
            }
        }

        pending = next_pending;
    }

    out.push_str("\nrender \"replica_progressive.png\"\n");
}

/// 计算块的平均色和块内最大单通道差异
fn block_stats(
    rgba: &RgbaImage,
    bx: u32,
    by: u32,
    bs: u32,
    w: u32,
    h: u32,
) -> (u8, u8, u8, u8) {
    let y_end = (by + bs).min(h);
    let x_end = (bx + bs).min(w);
    let mut sum_r = 0u64;
    let mut sum_g = 0u64;
    let mut sum_b = 0u64;
    let mut count = 0u64;
    let mut min_r = 255u8;
    let mut max_r = 0u8;
    let mut min_g = 255u8;
    let mut max_g = 0u8;
    let mut min_b = 255u8;
    let mut max_b = 0u8;
    for y in by..y_end {
        for x in bx..x_end {
            let p = rgba.get_pixel(x, y);
            sum_r += p[0] as u64;
            sum_g += p[1] as u64;
            sum_b += p[2] as u64;
            if p[0] < min_r { min_r = p[0]; }
            if p[0] > max_r { max_r = p[0]; }
            if p[1] < min_g { min_g = p[1]; }
            if p[1] > max_g { max_g = p[1]; }
            if p[2] < min_b { min_b = p[2]; }
            if p[2] > max_b { max_b = p[2]; }
            count += 1;
        }
    }
    let r = (sum_r / count) as u8;
    let g = (sum_g / count) as u8;
    let b = (sum_b / count) as u8;
    let var = (max_r - min_r).max(max_g - min_g).max(max_b - min_b);
    (r, g, b, var)
}

/// 转义文件路径中的反斜杠（Windows 路径 → .vgl 字符串字面量）
fn escape_path(p: &str) -> String {
    p.replace('\\', "/").replace('"', "\\\"")
}
