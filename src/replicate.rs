// ============================================================
// 图像复刻工具链（v1.0 语义化）
// 模式：
//   - semantic: 语义化分析，生成可编辑的生成蓝图（推荐）
//   - pixel: 逐像素 1:1 无损复刻（保留）
//   - block: 分块法（保留）
//   - progressive: 渐进法（保留）
// ============================================================

use image::{DynamicImage, GenericImageView, RgbaImage};

// ============================================================
// 语义化复刻（核心模式）
// 分析图像 → 提取调色板/梯度/区域 → 生成语义化 VGL 代码
// ============================================================

/// 语义化复刻：分析图像结构，生成可编辑的程序化生成代码
pub fn replicate_semantic(img: &DynamicImage, out: &mut String) {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();

    // 1. K-means 调色板提取
    let palette = kmeans_colors(&rgba, w, h, 6, 10);

    // 2. 图像结构分析
    let analysis = analyze_image(&rgba, w, h);

    // 3. 生成语义化 VGL 代码
    generate_semantic_code(w, h, &palette, &analysis, out);
}

/// K-means 颜色聚类：提取主色调
fn kmeans_colors(rgba: &RgbaImage, w: u32, h: u32, k: usize, iters: usize) -> Vec<(u8, u8, u8)> {
    // 采样像素（每 N 个取一个，加速）
    let step = ((w * h) / 2000).max(1) as u32;
    let mut samples: Vec<(f64, f64, f64)> = Vec::new();
    let mut idx = 0u32;
    for y in 0..h {
        for x in 0..w {
            if idx % step == 0 {
                let p = rgba.get_pixel(x, y);
                samples.push((p[0] as f64, p[1] as f64, p[2] as f64));
            }
            idx += 1;
        }
    }
    if samples.is_empty() {
        return vec![(0, 0, 0); k];
    }

    // 初始化质心（均匀采样）
    let mut centroids: Vec<(f64, f64, f64)> = Vec::with_capacity(k);
    for i in 0..k {
        let si = (i * samples.len()) / k;
        centroids.push(samples[si.min(samples.len() - 1)]);
    }

    // 迭代
    for _ in 0..iters {
        let mut sums = vec![(0.0f64, 0.0f64, 0.0f64); k];
        let mut counts = vec![0u32; k];

        for s in &samples {
            let mut best = 0;
            let mut best_dist = f64::MAX;
            for (ci, c) in centroids.iter().enumerate() {
                let dr = s.0 - c.0;
                let dg = s.1 - c.1;
                let db = s.2 - c.2;
                let d = dr * dr + dg * dg + db * db;
                if d < best_dist {
                    best_dist = d;
                    best = ci;
                }
            }
            sums[best].0 += s.0;
            sums[best].1 += s.1;
            sums[best].2 += s.2;
            counts[best] += 1;
        }

        for i in 0..k {
            if counts[i] > 0 {
                centroids[i] = (
                    sums[i].0 / counts[i] as f64,
                    sums[i].1 / counts[i] as f64,
                    sums[i].2 / counts[i] as f64,
                );
            }
        }
    }

    // 按亮度排序（暗→亮）
    centroids.sort_by(|a, b| {
        let ba = (a.0 + a.1 + a.2) / 3.0;
        let bb = (b.0 + b.1 + b.2) / 3.0;
        ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
    });

    centroids
        .iter()
        .map(|c| (c.0 as u8, c.1 as u8, c.2 as u8))
        .collect()
}

/// 图像分析结果
struct ImageAnalysis {
    horizon_y: f64,         // 地平线 Y 坐标（0=顶部, 1=底部）
    top_brightness: f64,    // 顶部平均亮度 (0-1)
    bottom_brightness: f64, // 底部平均亮度 (0-1)
    is_warm: bool,          // 暖色调（R > B）
    has_gradient: bool,     // 顶部有明显垂直渐变（天空特征）
    top_bottom_contrast: f64, // 上下对比度
    overall_brightness: f64, // 整体亮度
}

/// 分析图像结构：地平线、渐变、色温
fn analyze_image(rgba: &RgbaImage, w: u32, h: u32) -> ImageAnalysis {
    // 按行计算平均亮度
    let mut row_brightness: Vec<f64> = Vec::with_capacity(h as usize);
    let mut row_avg_r: Vec<f64> = Vec::with_capacity(h as usize);
    let mut row_avg_b: Vec<f64> = Vec::with_capacity(h as usize);

    for y in 0..h {
        let mut sum_l = 0u64;
        let mut sum_r = 0u64;
        let mut sum_b = 0u64;
        for x in 0..w {
            let p = rgba.get_pixel(x, y);
            sum_l += (p[0] as u64 + p[1] as u64 + p[2] as u64) / 3;
            sum_r += p[0] as u64;
            sum_b += p[2] as u64;
        }
        let n = w as u64;
        row_brightness.push(sum_l as f64 / n as f64 / 255.0);
        row_avg_r.push(sum_r as f64 / n as f64);
        row_avg_b.push(sum_b as f64 / n as f64);
    }

    // 地平线检测：寻找亮度变化最大的行
    let mut max_diff = 0.0f64;
    let mut horizon_y = 0.5;
    for i in 1..h as usize {
        let top = row_brightness[(i / 4).max(1) - 1];
        let bot = row_brightness[((i + h as usize / 4) / 2).min(h as usize - 1)];
        let diff = (bot - top).abs();
        if diff > max_diff {
            max_diff = diff;
            horizon_y = i as f64 / h as f64;
        }
    }

    // 顶部和底部亮度
    let top_quarter = h / 4;
    let bottom_quarter = h * 3 / 4;
    let mut top_sum = 0.0;
    let mut bot_sum = 0.0;
    let mut top_r = 0.0;
    let mut top_b = 0.0;
    let mut all_sum = 0.0;
    for y in 0..h {
        all_sum += row_brightness[y as usize];
        if y < top_quarter {
            top_sum += row_brightness[y as usize];
            top_r += row_avg_r[y as usize];
            top_b += row_avg_b[y as usize];
        }
        if y >= bottom_quarter {
            bot_sum += row_brightness[y as usize];
        }
    }

    let top_brightness = top_sum / top_quarter as f64;
    let bottom_brightness = bot_sum / (h - bottom_quarter) as f64;
    let overall_brightness = all_sum / h as f64;
    let top_r_avg = top_r / top_quarter as f64;
    let top_b_avg = top_b / top_quarter as f64;
    let is_warm = top_r_avg > top_b_avg;
    let top_bottom_contrast = (top_brightness - bottom_brightness).abs();
    let has_gradient = top_bottom_contrast > 0.15;

    ImageAnalysis {
        horizon_y,
        top_brightness,
        bottom_brightness,
        is_warm,
        has_gradient,
        top_bottom_contrast,
        overall_brightness,
    }
}

/// 生成语义化 VGL 代码
fn generate_semantic_code(
    w: u32,
    h: u32,
    palette: &[(u8, u8, u8)],
    analysis: &ImageAnalysis,
    out: &mut String,
) {
    let horizon_px = (analysis.horizon_y * h as f64) as u32;

    out.push_str("// 语义化 VGL — 语义化复刻\n");
    out.push_str("// 由 vgl replicate --mode semantic 生成\n");
    out.push_str("// 此代码描述\"如何生成\"此图像，而非\"每个像素是什么颜色\"\n");
    out.push_str("// 调整下方参数即可改变生成效果，AI 可直接编辑\n\n");

    // 导入标准库（v1.0: 语义化库 + 默认参数）
    out.push_str("import \"lib/palette.vgl\"\n");
    out.push_str("import \"lib/sky.vgl\"\n");
    out.push_str("import \"lib/terrain.vgl\"\n");
    out.push_str("import \"lib/water.vgl\"\n");
    out.push_str("import \"lib/vegetation.vgl\"\n");
    out.push_str("import \"lib/atmosphere.vgl\"\n\n");

    // 导入所需函数（按需导入）
    out.push_str("from Palette import pick\n");
    if analysis.has_gradient {
        out.push_str("from Sky import gradient, sun, clouds, stars\n");
        out.push_str("from Water import surface, sparkles, reflection\n");
        out.push_str("from Vegetation import tree, pine, bush, flowerbed\n");
        out.push_str("from Atmosphere import height_fog, fireflies\n");
    }
    out.push_str("from Terrain import mountains, ground\n\n");

    // 画布与种子
    out.push_str(&format!("canvas {}x{}\n", w, h));
    out.push_str("seed 42\n\n");

    // 调色板定义（v1.0: 使用 color() 语义化构造器）
    out.push_str("// ===== 调色板（从原图提取，v1.0 color() 构造器）=====\n");
    let names = if analysis.has_gradient {
        // 风景画：天空+地形
        vec!["sky_top", "sky_horizon", "mountain_far", "mountain_mid", "mountain_near", "ground_col"]
    } else {
        // 通用：明度梯度
        vec!["darkest", "dark", "mid", "light", "lightest", "accent"]
    };

    for (i, &(r, g, b)) in palette.iter().enumerate() {
        let name = names.get(i).unwrap_or(&"extra");
        out.push_str(&format!("let {} = color({}, {}, {})\n", name, r, g, b));
    }
    out.push_str(&format!(
        "let sun_col = color({}, {}, {})\n\n",
        if analysis.is_warm { 255 } else { 200 },
        if analysis.is_warm { 245 } else { 230 },
        if analysis.is_warm { 200 } else { 255 }
    ));

    // 天空（v1.0: 命名参数让意图清晰）
    if analysis.has_gradient {
        out.push_str("// ===== 天空 =====\n");
        let sky_top = &names[0];
        let sky_horizon = &names[1];
        out.push_str(&format!("gradient({}, {})\n", sky_top, sky_horizon));

        // 星空（如果整体偏暗）
        if analysis.overall_brightness < 0.4 {
            out.push_str("stars(count: 80, max_brightness: 0.5)\n");
        }

        // 太阳（如果暖色调且较亮）
        if analysis.is_warm && analysis.top_brightness > 0.4 {
            let sun_x = w / 2;
            let sun_y = (h as f64 * 0.15) as u32;
            out.push_str(&format!("sun({}, {}, color: sun_col)\n", sun_x, sun_y));
        }

        // 云（命名参数）
        let cloud_count = if analysis.top_brightness > 0.6 { 3 } else { 6 };
        out.push_str(&format!("clouds(count: {}, base_color: color(255, 255, 255), opacity: 0.5)\n\n", cloud_count));
    } else {
        // 无明显渐变：用径向渐变做背景
        out.push_str("// ===== 背景 =====\n");
        out.push_str(&format!(
            "fill_radial_gradient({}, {}, {}, {}, {})\n\n",
            w / 2, h / 2, w / 2,
            names[palette.len().min(2) - 1],
            names[0]
        ));
    }

    // 地形（v1.0: spacing 命名参数）
    out.push_str("// ===== 地形 =====\n");
    if analysis.has_gradient {
        // 多层山脉
        let mountain_palette = if palette.len() >= 4 {
            format!("[{}, {}, {}]", names[2], names[3], names[4].min(names[3]))
        } else {
            format!("[{}, {}]", names[1], names[0])
        };
        out.push_str(&format!(
            "mountains(3, {}, {}, spacing: 35)\n",
            mountain_palette, horizon_px
        ));
        out.push_str(&format!("ground({}, color: {}, noise_amount: 0.15)\n\n", horizon_px + 10, names[5].min(names[4]).min(names[3])));
    } else {
        // 无明显地平线：用纹理填充
        out.push_str(&format!("ground({}, color: {}, noise_amount: 0.2)\n\n", h / 2, names[2].min(names[1])));
    }

    // 水面 + 前景（v1.0: 风景画自动生成完整场景）
    if analysis.has_gradient {
        out.push_str("// ===== 水面 =====\n");
        let water_y = horizon_px + 5;
        out.push_str(&format!("surface({}, depth: 80, color1: color(20, 30, 60), color2: color(10, 15, 35), wave_intensity: 0.3)\n", water_y));
        out.push_str(&format!("sparkles({}, count: 30, color: sun_col)\n", water_y));
        out.push_str(&format!("reflection(y_water: {}, sky_color: {}, sun_x: {}, sun_y: {}, sun_color: sun_col)\n\n", water_y, names[1], w / 2, (h as f64 * 0.15) as u32));

        out.push_str("// ===== 前景植被 =====\n");
        out.push_str("pine(100, 500, scale: 1.5, trunk_color: color(15, 10, 20), leaf_color: color(20, 30, 25))\n");
        out.push_str("tree(700, 505, scale: 1.3, trunk_color: color(15, 10, 20), leaf_color: color(20, 30, 25))\n");
        out.push_str("bush(300, 520, scale: 1.2, color: color(20, 30, 25))\n");
        out.push_str("flowerbed(200, 530, 150, count: 8, palette: [color(255, 100, 100), color(255, 200, 100), color(200, 100, 255)])\n\n");

        out.push_str("// ===== 大气 =====\n");
        out.push_str("height_fog(350, color: color(100, 80, 120), max_intensity: 0.3)\n");
        out.push_str("fireflies(count: 15, color: color(255, 255, 150))\n\n");
    }

    // 后处理
    let vignette_strength = if analysis.overall_brightness > 0.6 { 0.2 } else { 0.35 };
    out.push_str(&format!("vignette({}, 0.7)\n", vignette_strength));

    if analysis.is_warm {
        out.push_str("grain(0.02, 1.0)\n");
    }

    // 渲染
    out.push_str("\nrender \"replica_semantic.png\"\n");
}

// ============================================================
// 旧模式（保留向后兼容）
// ============================================================

/// 模式 A：像素级精确复刻
pub fn replicate_pixel(img: &DynamicImage, input_path: &str, out: &mut String) {
    let (w, h) = img.dimensions();
    out.push_str(&format!("// 像素法复刻：{}x{}\n", w, h));
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
pub fn replicate_block(img: &DynamicImage, block_size: u32, out: &mut String) {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();

    out.push_str(&format!("// 分块法复刻：{}x{}，块大小 {}\n", w, h, block_size));
    out.push_str(&format!("canvas {}x{}\n", w, h));
    out.push_str("bg #000000\n\n");

    out.push_str("fn fill_block(x0, y0, size, r, g, b) {\n");
    out.push_str("    for dy in 0..size {\n");
    out.push_str("        for dx in 0..size {\n");
    out.push_str("            pixel(x: x0 + dx, y: y0 + dy, rgb: (r, g, b))\n");
    out.push_str("        }\n    }\n}\n\n");

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

/// 模式 D：渐进法
pub fn replicate_progressive(
    img: &DynamicImage,
    layers: &[u32],
    threshold: u8,
    out: &mut String,
) {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();

    out.push_str(&format!(
        "// 渐进法复刻：{}x{}，层 {}, 阈值 {}\n",
        w, h,
        layers.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("/"),
        threshold
    ));
    out.push_str(&format!("canvas {}x{}\n", w, h));
    out.push_str("bg #000000\n\n");

    out.push_str("fn fill_block(x0, y0, size, r, g, b) {\n");
    out.push_str("    for dy in 0..size {\n");
    out.push_str("        for dx in 0..size {\n");
    out.push_str("            pixel(x: x0 + dx, y: y0 + dy, rgb: (r, g, b))\n");
    out.push_str("        }\n    }\n}\n\n");

    let mut pending: Vec<(u32, u32, u32)> = Vec::new();

    for (layer_idx, &bs) in layers.iter().enumerate() {
        let mut next_pending: Vec<(u32, u32, u32)> = Vec::new();

        if layer_idx == 0 {
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

        for (bx, by, parent_bs) in &pending {
            let parent_bs = *parent_bs;
            let mut sy = *by;
            while sy < (*by + parent_bs).min(h) {
                let mut sx = *bx;
                while sx < (*bx + parent_bs).min(w) {
                    let (r, g, b, var) = block_stats(&rgba, sx, sy, bs, w, h);
                    if bs == 1 {
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

/// 转义文件路径
fn escape_path(p: &str) -> String {
    p.replace('\\', "/").replace('"', "\\\"")
}
