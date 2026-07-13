// ============================================================
// 噪声实现
// ============================================================

pub const PERM: [usize; 512] = {
    let mut p = [0; 512];
    let mut i = 0;
    while i < 256 {
        p[i] = i;
        p[i + 256] = i;
        i += 1;
    }
    let shuffle = [151,160,137,91,90,15,131,13,201,95,96,53,194,233,7,225,
                   140,36,103,30,69,142,8,99,37,240,21,10,23,190,6,148,
                   247,120,234,75,0,26,197,62,94,252,219,203,117,35,11,32,
                   57,177,33,88,237,149,56,87,174,20,125,136,171,168,68,175,
                   74,165,71,134,139,48,27,166,77,146,158,231,83,111,229,122,
                   60,211,133,230,220,105,92,41,55,46,245,40,244,102,143,54,
                   65,25,63,161,1,216,80,73,209,76,132,187,208,89,18,169,
                   200,196,135,130,116,188,159,86,164,100,109,198,173,186,3,64,
                   52,217,226,250,124,123,5,202,38,147,118,126,255,82,85,212,
                   207,206,59,227,47,16,58,17,182,189,28,42,223,183,170,213,
                   119,248,152,2,44,154,163,70,221,153,101,155,167,43,172,9,
                   129,22,39,253,19,98,108,110,79,113,224,232,178,185,112,104,
                   218,246,97,228,251,34,242,193,238,210,144,12,191,179,162,241,
                   81,51,145,235,249,14,239,107,49,192,214,31,181,199,106,157,
                   184,84,204,176,115,121,50,45,127,4,150,254,138,236,205,93,
                   222,114,67,29,24,72,243,141,128,195,78,66,215,61,156,180];
    let mut j = 0;
    while j < 512 {
        p[j] = shuffle[j % 256];
        j += 1;
    }
    p
};

pub fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}
pub fn grad(hash: usize, x: f64, y: f64) -> f64 {
    let h = hash & 7;
    let u = if h < 4 { x } else { y };
    let v = if h < 4 { y } else { x };
    (if (h & 1) == 0 { u } else { -u }) + (if (h & 2) == 0 { v } else { -v })
}
pub fn perlin(x: f64, y: f64) -> f64 {
    let xi = x.floor() as i32 & 255;
    let yi = y.floor() as i32 & 255;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);
    let p = &PERM;
    let aaa = p[p[xi as usize] + yi as usize];
    let aba = p[p[xi as usize] + yi as usize + 1];
    let baa = p[p[xi as usize + 1] + yi as usize];
    let bba = p[p[xi as usize + 1] + yi as usize + 1];
    let x1 = lerp(grad(aaa, xf, yf), grad(baa, xf - 1.0, yf), u);
    let x2 = lerp(grad(aba, xf, yf - 1.0), grad(bba, xf - 1.0, yf - 1.0), u);
    lerp(x1, x2, v)
}

/// v0.8 可种子化的 Perlin 噪声
/// 用种子生成置换表，使 seed 语句能影响 perlin/worley/fbm 的输出
pub fn seeded_perm(seed: u64) -> [usize; 512] {
    // 用 LCG 生成 0..255 的随机排列
    let mut perm = [0usize; 256];
    for i in 0..256 {
        perm[i] = i;
    }
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    // Fisher-Yates 洗牌
    for i in (1..256).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = (state >> 33) as usize % (i + 1);
        perm.swap(i, j);
    }
    let mut p = [0usize; 512];
    for i in 0..256 {
        p[i] = perm[i];
        p[i + 256] = perm[i];
    }
    p
}

/// v0.8 可种子化的 Perlin 噪声（使用外部置换表）
pub fn perlin_seeded(x: f64, y: f64, perm: &[usize; 512]) -> f64 {
    let xi = x.floor() as i32 & 255;
    let yi = y.floor() as i32 & 255;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);
    let p = perm;
    let aaa = p[p[xi as usize] + yi as usize];
    let aba = p[p[xi as usize] + yi as usize + 1];
    let baa = p[p[xi as usize + 1] + yi as usize];
    let bba = p[p[xi as usize + 1] + yi as usize + 1];
    let x1 = lerp(grad(aaa, xf, yf), grad(baa, xf - 1.0, yf), u);
    let x2 = lerp(grad(aba, xf, yf - 1.0), grad(bba, xf - 1.0, yf - 1.0), u);
    lerp(x1, x2, v)
}

/// v0.5 修复：使用 i64 避免溢出
pub fn worley(x: f64, y: f64) -> f64 {
    let cell_size = 32.0;
    let cx = (x / cell_size).floor() as i64;
    let cy = (y / cell_size).floor() as i64;
    let mut min_dist = 1e9;
    for dx in -1..=1 {
        for dy in -1..=1 {
            let ncx = cx + dx;
            let ncy = cy + dy;
            // 用 i64 计算避免 i32 溢出
            let mut h: u64 = (ncx.wrapping_mul(374761393) + ncy.wrapping_mul(668265263)) as u64;
            h = (h ^ (h >> 13)).wrapping_mul(1274126177);
            h = h ^ (h >> 16);
            let px = ncx as f64 * cell_size + (h % cell_size as u64) as f64;
            let py = ncy as f64 * cell_size + ((h >> 8) % cell_size as u64) as f64;
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if d < min_dist {
                min_dist = d;
            }
        }
    }
    min_dist
}

pub fn fbm(x: f64, y: f64, octaves: i32) -> f64 {
    let mut total = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut norm = 0.0;
    let oct = octaves.max(1).min(8);
    for _ in 0..oct {
        total += perlin(x * freq, y * freq) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    total / norm
}
