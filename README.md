<h1 align="center">🍃 VGL v2.0 (Visual Graphics Language)</h1>

<p align="center">
  <em>A procedural language that generates SVG — for AI and humans to read, write, and edit.</em><br>
  <em>过程式矢量图形语言：输出 SVG，为 AI 与人类的阅读、生成、修改而生。</em><br>
  <em>代码是"活的生成蓝图"，输出是无限精度的矢量。</em>
</p>

---

VGL 是一门**输出 SVG 的过程式图形语言**。它不堆像素坐标，而是用变量、函数、循环和噪声描述"如何生成"一张图：

- **矢量原生** — 输出即 SVG（渐变、路径、滤镜、变换），浏览器直接打开，Figma/Illustrator 直接编辑，放大到任意尺寸不失真
- **确定性生成** — `seed 42` 永远得到同一张图；改一个变量、换一个种子，整幅场景随之重生
- **AI 友好** — 语法精简（~20 个关键字），语义库命名直白（`mountains`、`fog`、`aurora`），AI 读代码即懂画面
- **零依赖单二进制** — 纯 Rust，无外部库，`cargo build` 即可

```vgl
use "../lib/palette.vgl"
use "../lib/sky.vgl"
use "../lib/terrain.vgl"
use "../lib/water.vgl"
use "../lib/atmosphere.vgl"

canvas 1000x700
seed 7

let cols = palette("dusk")           // 语义调色板
sky(cols[0], cols[1])                // 黄昏渐变天空
sun(500, 430, 46, cols[2])           // 低垂的太阳
mountains(4, 470, color(90, 70, 100), color(30, 22, 48), 90)  // 层叠远山
water(530)                           // 湖面
shimmer(500, 530, 120, cols[2])      // 日光倒影
ground_fog(520)                      // 贴地雾
vignette(0.4)                        // 暗角

render "landscape.svg"
```

---

## ✨ Features · 功能特性

| Category · 分类 | Capabilities · 能力 |
|----------|-------------|
| **Shapes · 形状** | `rect` `circle` `ellipse` `line` `polygon` `polyline` `path` `text`（真 SVG 文本） |
| **Paint · 填充** | `color(r,g,b,a)` / `#hex` / `linear_gradient` / `radial_gradient`，任意形状可填充渐变 |
| **Effects · 效果** | `blur:`（高斯模糊滤镜）、`opacity:`、`smooth()`（Catmull-Rom 平滑路径） |
| **Transform · 变换** | `group(translate:, rotate:, scale:)` 嵌套变换组 |
| **Procedural · 过程式** | `fn`（默认参数）、`for i in a..b step s`、`while`、`if/else`、数组、`return` |
| **Noise · 噪声** | 确定性 `rand` / `rand_int` / `perlin` / `fbm`（同一 seed 完全可复现） |
| **Semantic Library · 语义库** | `lib/` 六个模块 30+ 场景原语（见下表） |

### 语义库 lib/

| Module | Primitives |
|--------|-----------|
| `palette.vgl` | `palette("dusk"/"night"/...)` `palette_sky` `palette_pick` |
| `sky.vgl` | `sky` `sun` `moon` `stars` `meteor` `clouds` `cloud_band` `aurora` |
| `terrain.vgl` | `ridge` `mountains` `snow_cap` `dunes` |
| `water.vgl` | `water` `waves` `shimmer` |
| `vegetation.vgl` | `tree` `broadleaf` `conifer` `forest` `grass` |
| `atmosphere.vgl` | `fog` `ground_fog` `rain` `snow` `light_rays` `vignette` `tint` |

---

## Quick Start · 快速上手

```bash
git clone https://github.com/Whatsmore-nf/Garden-VGL.git
cd Garden-VGL
cargo build --release

./target/release/vgl examples/landscape.vgl   # → examples/landscape.svg
```

浏览器打开生成的 SVG 即可查看。

### 三行起步

```vgl
canvas 400x300
circle(200, 150, 80, fill: linear_gradient([color(255,107,107), color(78,205,196)]))
render "first.svg"
```

---

## Language · 语言速览

```vgl
// 结构：use 引库 → canvas 定尺寸 → seed 定随机 → 绘制 → render 输出
canvas 800x600
seed 42

let n = 5                            // 变量
let cols = ["#ff6b6b", "#4ecdc4"]    // 数组

fn ring(x, y, r, n = 6) {            // 函数 + 默认参数
    for i in 0..n {                  // 循环
        let a = i / n * TAU
        if i % 2 == 0 {              // 条件
            circle(x + cos(a) * r, y + sin(a) * r, 8, fill: cols[0])
        } else {
            circle(x + cos(a) * r, y + sin(a) * r, 8, fill: cols[1])
        }
    }
}

group(translate: [400, 300], rotate: 15) {   // 变换组
    ring(0, 0, 100)
    text(0, 6, "VGL", size: 24, fill: "#fff", anchor: "middle")
}

let pts = []
for i in 0..30 {
    push(pts, i * 27)
    push(pts, 550 + fbm(i * 0.2, 0) * 40)   // 分形噪声
}
path(smooth(pts), stroke: "#4ecdc4", width: 2)  // 平滑曲线

render "out.svg"
```

完整语法见 [快速入门](docs/VGL-快速入门.md) 与 [Quick Reference](docs/VGL-Quick-Reference.md)。

---

## Examples · 示例

| Script · 脚本 | Highlights · 亮点 |
|--------|-----------|
| [smoke.vgl](examples/smoke.vgl) | 语言冒烟测试：全部语法特性 |
| [landscape.vgl](examples/landscape.vgl) | 黄昏山水湖景：palette/sun/mountains/water/forest/fog |
| [night_aurora.vgl](examples/night_aurora.vgl) | 极光雪夜：stars/aurora/moon/snow/conifer |

---

## Project Layout · 项目结构

```
src/
  lexer.rs    # 词法分析
  parser.rs   # 递归下降语法分析
  ast.rs      # AST + 环境
  interp.rs   # 树遍历解释器 + 内建绘图函数
  scene.rs    # 矢量场景图（元素/组/defs）
  svg.rs      # SVG 序列化
  noise.rs    # xorshift64* + 柏林噪声 / fbm
lib/          # 语义库（VGL 自举编写）
examples/     # 示例脚本与生成结果
```

## Design Philosophy · 设计理念

| 中文 | English |
|------|---------|
| **矢量优先**：精度即 SVG 精度，无限缩放，浏览器/编辑器/AI 全部原生支持 | **Vector-first**: precision is SVG precision; infinitely scalable, natively supported everywhere |
| **活的蓝图**：代码描述"如何生成"，改参数即改画面，永不逐像素修补 | **Living blueprint**: code describes *how to generate*; tweak a parameter, not pixels |
| **确定性**：同一 seed 必得同一结果，生成过程可复现、可调试 | **Deterministic**: same seed, same image — reproducible and debuggable |
| **为 AI 而生**：精简语法 + 语义命名，AI 读写代码即读写画面 | **AI-native**: minimal syntax + semantic names; reading code = reading the image |

## Version History · 版本历史

| Version | Theme · 主题 |
|---------|-------|
| v0.1–v0.9 | 光栅渲染时代：Python 原型 → Rust 重写 → SDF 抗锯齿、材质、类与模块 |
| v1.0 | 语义库 + 图像复刻（replicate）工具链 |
| **v2.0** | **全面转向矢量**：重写语法（精简为 ~20 关键字）、输出 SVG、删除光栅管线 / C++ 实现 / replicate，语义库全部矢量重写 |

## License · 许可证

MIT
