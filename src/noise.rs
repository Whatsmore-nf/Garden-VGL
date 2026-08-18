// ============================================================
// VGL v2.0 — 确定性随机与柏林噪声
// ============================================================

/// xorshift64* 伪随机数生成器 — 同一 seed 序列完全可复现
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed } }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// [0, 1) 均匀浮点
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// [a, b) 均匀浮点
    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + self.next_f64() * (b - a)
    }
}

/// 用种子生成 Perlin 置换表（Fisher-Yates 洗牌）
pub fn seeded_perm(seed: u64) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..256).collect();
    let mut rng = Rng::new(seed ^ 0xB5297A4D);
    for i in (1..256).rev() {
        let j = (rng.next_u64() >> 33) as usize % (i + 1);
        perm.swap(i, j);
    }
    let mut p = Vec::with_capacity(512);
    for i in 0..512 {
        p.push(perm[i & 255]);
    }
    p
}

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

fn grad(hash: usize, x: f64, y: f64) -> f64 {
    let h = hash & 7;
    let u = if h < 4 { x } else { y };
    let v = if h < 4 { y } else { x };
    (if (h & 1) == 0 { u } else { -u }) + (if (h & 2) == 0 { v } else { -v })
}

/// 柏林噪声，返回约 [-1, 1]
pub fn perlin(x: f64, y: f64, p: &[usize]) -> f64 {
    let xi = x.floor() as i32 & 255;
    let yi = y.floor() as i32 & 255;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);
    let aaa = p[p[xi as usize] + yi as usize];
    let aba = p[p[xi as usize] + yi as usize + 1];
    let baa = p[p[xi as usize + 1] + yi as usize];
    let bba = p[p[xi as usize + 1] + yi as usize + 1];
    let x1 = lerp(grad(aaa, xf, yf), grad(baa, xf - 1.0, yf), u);
    let x2 = lerp(grad(aba, xf, yf - 1.0), grad(bba, xf - 1.0, yf - 1.0), u);
    lerp(x1, x2, v)
}

/// 分形布朗运动（多倍频叠加），返回约 [-1, 1]
pub fn fbm(x: f64, y: f64, octaves: i32, p: &[usize]) -> f64 {
    let mut total = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut norm = 0.0;
    let oct = octaves.clamp(1, 8);
    for _ in 0..oct {
        total += perlin(x * freq, y * freq, p) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    total / norm
}
