// ============================================================
// 绘图引擎
// ============================================================

#[derive(Clone, Debug)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<f32>, // RGBA，每像素 4 个 f32，范围 [0.0, 255.0]
    pub bg: (f32, f32, f32, f32), // 背景色 RGBA，alpha 默认 255.0
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Canvas {
            width: w,
            height: h,
            pixels: vec![0.0; (w * h * 4) as usize],
            bg: (0.0, 0.0, 0.0, 255.0),
        }
    }

    /// 写入不透明像素（覆盖模式，alpha=255）
    pub fn put_pixel(&mut self, x: i32, y: i32, r: f32, g: f32, b: f32) {
        self.put_pixel_rgba(x, y, r, g, b, 255.0)
    }

    /// 写入带 alpha 的像素，与现有像素做 source-over 合成
    /// src=新像素，dst=现有像素
    /// out_alpha = src_a + dst_a * (1 - src_a) / 255
    /// out_rgb = (src_rgb * src_a + dst_rgb * dst_a * (1 - src_a/255)) / out_alpha
    pub fn put_pixel_rgba(&mut self, x: i32, y: i32, r: f32, g: f32, b: f32, a: f32) {
        if x < 0 || x >= self.width as i32 || y < 0 || y >= self.height as i32 {
            return;
        }
        let idx = (y as u32 * self.width + x as u32) as usize * 4;
        let sa = a.max(0.0).min(255.0) / 255.0;
        if sa <= 0.0 {
            return;
        }
        let da = self.pixels[idx + 3] / 255.0;
        let out_a = sa + da * (1.0 - sa);
        if out_a <= 0.0 {
            self.pixels[idx..idx + 4].copy_from_slice(&[r, g, b, a]);
            return;
        }
        let r2 = (r * sa + self.pixels[idx] * da * (1.0 - sa)) / out_a;
        let g2 = (g * sa + self.pixels[idx + 1] * da * (1.0 - sa)) / out_a;
        let b2 = (b * sa + self.pixels[idx + 2] * da * (1.0 - sa)) / out_a;
        self.pixels[idx] = r2.max(0.0).min(255.0);
        self.pixels[idx + 1] = g2.max(0.0).min(255.0);
        self.pixels[idx + 2] = b2.max(0.0).min(255.0);
        self.pixels[idx + 3] = out_a * 255.0;
    }

    /// 抗锯齿绘制：alpha∈[0,1] 与现有像素合成（保留旧接口语义，内部转 f32）
    pub fn put_pixel_aa(&mut self, x: i32, y: i32, r: f32, g: f32, b: f32, alpha: f64) {
        self.put_pixel_rgba(x, y, r, g, b, (alpha.max(0.0).min(1.0) * 255.0) as f32);
    }

    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, width: f64, r: f32, g: f32, b: f32) {
        if width <= 1.0 {
            self.wu_line(x0, y0, x1, y1, r, g, b);
        } else {
            for (x, y) in self.bresenham_points(x0, y0, x1, y1) {
                self.brush(x, y, width as i32, r, g, b);
            }
        }
    }
    pub fn bresenham_points(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
        let mut pts = Vec::new();
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut x = x0;
        let mut y = y0;
        loop {
            pts.push((x, y));
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
        pts
    }
    pub fn brush(&mut self, cx: i32, cy: i32, radius: i32, r: f32, g: f32, b: f32) {
        let rad = radius as f64 / 2.0;
        let r2 = rad * rad;
        let ri = (rad + 1.0) as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f64 <= r2 {
                    self.put_pixel(cx + dx, cy + dy, r, g, b);
                }
            }
        }
    }
    pub fn wu_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, r: f32, g: f32, b: f32) {
        fn ipart(x: f64) -> i32 { x.floor() as i32 }
        fn fpart(x: f64) -> f64 { x - x.floor() }
        fn rfpart(x: f64) -> f64 { 1.0 - fpart(x) }
        let (mut x0, mut y0, mut x1, mut y1) = (x0, y0, x1, y1);
        let steep = (y1 - y0).abs() > (x1 - x0).abs();
        if steep {
            std::mem::swap(&mut x0, &mut y0);
            std::mem::swap(&mut x1, &mut y1);
        }
        if x0 > x1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }
        let dx = x1 - x0;
        let dy = y1 - y0;
        let grad = if dx != 0 { dy as f64 / dx as f64 } else { 1.0 };
        let xend = (x0 as f64).round() as i32;
        let yend = y0 as f64 + grad * (xend - x0) as f64;
        let xgap = rfpart(x0 as f64 + 0.5);
        let xpxl1 = xend;
        let ypxl1 = ipart(yend);
        if steep {
            self.put_pixel_aa(ypxl1, xpxl1, r, g, b, rfpart(yend) * xgap);
            self.put_pixel_aa(ypxl1 + 1, xpxl1, r, g, b, fpart(yend) * xgap);
        } else {
            self.put_pixel_aa(xpxl1, ypxl1, r, g, b, rfpart(yend) * xgap);
            self.put_pixel_aa(xpxl1, ypxl1 + 1, r, g, b, fpart(yend) * xgap);
        }
        let mut intery = yend + grad;
        let xend2 = (x1 as f64).round() as i32;
        let yend2 = y1 as f64 + grad * (xend2 - x1) as f64;
        let xgap2 = fpart(x1 as f64 + 0.5);
        let xpxl2 = xend2;
        let ypxl2 = ipart(yend2);
        if steep {
            self.put_pixel_aa(ypxl2, xpxl2, r, g, b, rfpart(yend2) * xgap2);
            self.put_pixel_aa(ypxl2 + 1, xpxl2, r, g, b, fpart(yend2) * xgap2);
        } else {
            self.put_pixel_aa(xpxl2, ypxl2, r, g, b, rfpart(yend2) * xgap2);
            self.put_pixel_aa(xpxl2, ypxl2 + 1, r, g, b, fpart(yend2) * xgap2);
        }
        for x in (xpxl1 + 1)..xpxl2 {
            if steep {
                self.put_pixel_aa(ipart(intery), x, r, g, b, rfpart(intery));
                self.put_pixel_aa(ipart(intery) + 1, x, r, g, b, fpart(intery));
            } else {
                self.put_pixel_aa(x, ipart(intery), r, g, b, rfpart(intery));
                self.put_pixel_aa(x, ipart(intery) + 1, r, g, b, fpart(intery));
            }
            intery += grad;
        }
    }
    pub fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32, width: f64, r: f32, g: f32, b: f32) {
        let mut x = radius;
        let mut y = 0;
        let mut err = 0;
        while x >= y {
            for (px, py) in &[
                (cx + x, cy + y),
                (cx + y, cy + x),
                (cx - y, cy + x),
                (cx - x, cy + y),
                (cx - x, cy - y),
                (cx - y, cy - x),
                (cx + y, cy - x),
                (cx + x, cy - y),
            ] {
                if width <= 1.0 {
                    self.put_pixel(*px, *py, r, g, b);
                } else {
                    self.brush(*px, *py, width as i32, r, g, b);
                }
            }
            y += 1;
            if err <= 0 {
                err += 2 * y + 1;
            }
            if err > 0 {
                x -= 1;
                err -= 2 * x + 1;
            }
        }
    }
    pub fn sample_bezier3(
        &self,
        p1: (f64, f64),
        p2: (f64, f64),
        p3: (f64, f64),
        p4: (f64, f64),
        n: usize,
    ) -> Vec<(i32, i32)> {
        let mut pts = Vec::new();
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let q0 = (p1.0 + (p2.0 - p1.0) * t, p1.1 + (p2.1 - p1.1) * t);
            let q1 = (p2.0 + (p3.0 - p2.0) * t, p2.1 + (p3.1 - p2.1) * t);
            let q2 = (p3.0 + (p4.0 - p3.0) * t, p3.1 + (p4.1 - p3.1) * t);
            let r0 = (q0.0 + (q1.0 - q0.0) * t, q0.1 + (q1.1 - q0.1) * t);
            let r1 = (q1.0 + (q2.0 - q1.0) * t, q1.1 + (q2.1 - q1.1) * t);
            let point = (r0.0 + (r1.0 - r0.0) * t, r0.1 + (r1.1 - r0.1) * t);
            pts.push((point.0.round() as i32, point.1.round() as i32));
        }
        pts
    }
    /// 二次贝塞尔采样（通过提升为三次）
    pub fn sample_bezier2(
        &self,
        p1: (f64, f64),
        p2: (f64, f64),
        p3: (f64, f64),
        n: usize,
    ) -> Vec<(i32, i32)> {
        // 二次 -> 三次：c1 = p1 + 2/3*(p2-p1), c2 = p3 + 2/3*(p2-p3)
        let c1 = (p1.0 + (p2.0 - p1.0) * 2.0 / 3.0, p1.1 + (p2.1 - p1.1) * 2.0 / 3.0);
        let c2 = (p3.0 + (p2.0 - p3.0) * 2.0 / 3.0, p3.1 + (p2.1 - p3.1) * 2.0 / 3.0);
        self.sample_bezier3(p1, c1, c2, p3, n)
    }
    pub fn fill(&mut self, r: f32, g: f32, b: f32) {
        for i in (0..self.pixels.len()).step_by(4) {
            self.pixels[i] = r;
            self.pixels[i + 1] = g;
            self.pixels[i + 2] = b;
            self.pixels[i + 3] = 255.0;
        }
    }
}

// ============================================================
// 材质参数（v0.55 批次 D：逐像素 noise + alpha 集成）
// ============================================================

/// 材质参数（供 stroke 绘制时逐像素计算）
pub struct MaterialParams {
    pub r: f32,       // 基色 R [0,255]
    pub g: f32,
    pub b: f32,
    pub noise: f64,   // noise 强度 [0,1]，0=无扰动
    pub alpha: f32,   // 不透明度 [0,255]，默认 255
}

impl Canvas {
    /// 逐像素计算材质颜色：base + perlin(x*scale, y*scale) * noise * 255
    fn material_color(&self, x: i32, y: i32, m: &MaterialParams) -> (f32, f32, f32, f32) {
        if m.noise == 0.0 {
            return (m.r, m.g, m.b, m.alpha);
        }
        // 用像素坐标做 perlin，scale 控制纹理粗细
        let scale = 0.1; // 每个 noise cell 约 10 像素
        let n = crate::noise::perlin(x as f64 * scale, y as f64 * scale) * m.noise;
        // n ∈ [-noise, noise]，映射到 [-255*noise, 255*noise] 亮度偏移
        let offset = (n * 255.0) as f32;
        (
            (m.r + offset).max(0.0).min(255.0),
            (m.g + offset).max(0.0).min(255.0),
            (m.b + offset).max(0.0).min(255.0),
            m.alpha,
        )
    }

    /// 带材质的 put_pixel
    pub fn put_pixel_mat(&mut self, x: i32, y: i32, m: &MaterialParams) {
        if x < 0 || x >= self.width as i32 || y < 0 || y >= self.height as i32 {
            return;
        }
        let (r, g, b, a) = self.material_color(x, y, m);
        self.put_pixel_rgba(x, y, r, g, b, a);
    }

    /// 带材质的抗锯齿 put_pixel（alpha 由 Wu 算法计算，材质 alpha 相乘）
    pub fn put_pixel_aa_mat(&mut self, x: i32, y: i32, m: &MaterialParams, aa_alpha: f64) {
        if aa_alpha <= 0.0 || x < 0 || x >= self.width as i32 || y < 0 || y >= self.height as i32 {
            return;
        }
        let (r, g, b, a) = self.material_color(x, y, m);
        let combined = aa_alpha.max(0.0).min(1.0) * (a / 255.0) as f64;
        self.put_pixel_rgba(x, y, r, g, b, (combined * 255.0) as f32);
    }

    /// 带材质的 brush（圆形笔刷）
    pub fn brush_mat(&mut self, cx: i32, cy: i32, radius: i32, m: &MaterialParams) {
        let rad = radius as f64 / 2.0;
        let r2 = rad * rad;
        let ri = (rad + 1.0) as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f64 <= r2 {
                    self.put_pixel_mat(cx + dx, cy + dy, m);
                }
            }
        }
    }

    /// 带材质的 wu_line
    pub fn wu_line_mat(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, m: &MaterialParams) {
        fn ipart(x: f64) -> i32 {
            x.floor() as i32
        }
        fn fpart(x: f64) -> f64 {
            x - x.floor()
        }
        fn rfpart(x: f64) -> f64 {
            1.0 - fpart(x)
        }
        let (mut x0, mut y0, mut x1, mut y1) = (x0, y0, x1, y1);
        let steep = (y1 - y0).abs() > (x1 - x0).abs();
        if steep {
            std::mem::swap(&mut x0, &mut y0);
            std::mem::swap(&mut x1, &mut y1);
        }
        if x0 > x1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }
        let dx = x1 - x0;
        let dy = y1 - y0;
        let grad = if dx != 0 { dy as f64 / dx as f64 } else { 1.0 };
        let xend = (x0 as f64).round() as i32;
        let yend = y0 as f64 + grad * (xend - x0) as f64;
        let xgap = rfpart(x0 as f64 + 0.5);
        let xpxl1 = xend;
        let ypxl1 = ipart(yend);
        if steep {
            self.put_pixel_aa_mat(ypxl1, xpxl1, m, rfpart(yend) * xgap);
            self.put_pixel_aa_mat(ypxl1 + 1, xpxl1, m, fpart(yend) * xgap);
        } else {
            self.put_pixel_aa_mat(xpxl1, ypxl1, m, rfpart(yend) * xgap);
            self.put_pixel_aa_mat(xpxl1, ypxl1 + 1, m, fpart(yend) * xgap);
        }
        let mut intery = yend + grad;
        let xend2 = (x1 as f64).round() as i32;
        let yend2 = y1 as f64 + grad * (xend2 - x1) as f64;
        let xgap2 = fpart(x1 as f64 + 0.5);
        let xpxl2 = xend2;
        let ypxl2 = ipart(yend2);
        if steep {
            self.put_pixel_aa_mat(ypxl2, xpxl2, m, rfpart(yend2) * xgap2);
            self.put_pixel_aa_mat(ypxl2 + 1, xpxl2, m, fpart(yend2) * xgap2);
        } else {
            self.put_pixel_aa_mat(xpxl2, ypxl2, m, rfpart(yend2) * xgap2);
            self.put_pixel_aa_mat(xpxl2, ypxl2 + 1, m, fpart(yend2) * xgap2);
        }
        for x in (xpxl1 + 1)..xpxl2 {
            if steep {
                self.put_pixel_aa_mat(ipart(intery), x, m, rfpart(intery));
                self.put_pixel_aa_mat(ipart(intery) + 1, x, m, fpart(intery));
            } else {
                self.put_pixel_aa_mat(x, ipart(intery), m, rfpart(intery));
                self.put_pixel_aa_mat(x, ipart(intery) + 1, m, fpart(intery));
            }
            intery += grad;
        }
    }

    /// 带材质的 draw_line
    pub fn draw_line_mat(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        width: f64,
        m: &MaterialParams,
    ) {
        if width <= 1.0 {
            self.wu_line_mat(x0, y0, x1, y1, m);
        } else {
            for (x, y) in self.bresenham_points(x0, y0, x1, y1) {
                self.brush_mat(x, y, width as i32, m);
            }
        }
    }

    /// 带材质的 draw_circle
    pub fn draw_circle_mat(
        &mut self,
        cx: i32,
        cy: i32,
        radius: i32,
        width: f64,
        m: &MaterialParams,
    ) {
        let mut x = radius;
        let mut y = 0;
        let mut err = 0;
        while x >= y {
            for (px, py) in &[
                (cx + x, cy + y),
                (cx + y, cy + x),
                (cx - y, cy + x),
                (cx - x, cy + y),
                (cx - x, cy - y),
                (cx - y, cy - x),
                (cx + y, cy - x),
                (cx + x, cy - y),
            ] {
                if width <= 1.0 {
                    self.put_pixel_mat(*px, *py, m);
                } else {
                    self.brush_mat(*px, *py, width as i32, m);
                }
            }
            y += 1;
            if err <= 0 {
                err += 2 * y + 1;
            }
            if err > 0 {
                x -= 1;
                err -= 2 * x + 1;
            }
        }
    }
}
