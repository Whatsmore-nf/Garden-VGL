<h1 align="center">🍃 VGL (Visual Graphics Language)</h1>

<p align="center">
  <em>A language for AI to understand and procedurally edit.</em><br>
  <em>为 AI 理解和程序化编辑而生的图像生成语言。</em><br>
  <em>代码是"活的生成蓝图"，而非"死的像素快照"。</em>
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> ·
  <a href="docs/VGL-快速入门.md">快速入门</a> ·
  <a href="docs/VGL-Quick-Reference.md">Language Reference</a> ·
  <a href="docs/VGL_语法规范%20v0.5.txt">语法规范</a> ·
  <a href="examples">Examples</a>
</p>

---

VGL is a **procedural image generation language designed for AI understanding and semantic editing**. Instead of stacking meaningless pixel coordinates, VGL uses functions, variables, materials, and fields to create **living generation blueprints** — code that describes *how* to generate an image, not *what* each pixel is.

VGL 是一个**为 AI 理解和程序化编辑而生的程序化图像生成语言**。VGL 不堆砌无意义的像素坐标，而是用函数、变量、材质和场来创建**活的生成蓝图**——代码描述的是"如何生成"图像，而非"每个像素是什么"。

```vgl
import "lib/palette.vgl"
import "lib/sky.vgl"
import "lib/terrain.vgl"

from Sky import gradient, sun, clouds
from Terrain import mountains, ground

canvas 800x600
seed 42

// 调色板：语义化变量 + v1.0 color() 构造器，一目了然
let sky_top = color(30, 20, 60)
let sky_horizon = color(255, 180, 100)
let mountain_far = color(60, 50, 80)
let ground_col = color(40, 35, 50)
let sun_color = color(255, 220, 150)

// 生成蓝图：调用语义化函数（v1.0 命名参数，一看即懂）
gradient(sky_top, sky_horizon)
sun(500, 100, color: sun_color)
clouds(count: 5, base_color: color(255, 240, 200), opacity: 0.4)
mountains(3, [mountain_far, color(45, 40, 65), color(30, 25, 45)], 380, spacing: 40)
ground(390, color: ground_col, noise_amount: 0.2)

vignette(0.35, 0.7)
render "sunset.png"
```

---

## ✨ Features · 功能特性

| Category · 分类 | Capabilities · 能力 |
|----------|-------------|
| **Drawing · 绘制** | pixel, stroke (AA lines/circles/beziers), fill (rect/circle/ellipse/polygon/flood), gradient (linear/radial), text (5×7 dot matrix) |
| **Transform & Clip · 变换与裁剪** | translate/rotate/scale, push/pop transform stack, clip rect with intersection stack |
| **Materials & Layers · 材质与图层** | material definitions with noise perturbation, off-screen layer compositing (12 blend modes), color fields |
| **Data Types · 数据类型** | number, bool, string, color (with alpha), tuple, array, dict, struct, enum, class, path |
| **Control Flow · 控制流** | if/else-if/else, while, for range, for-in array, labeled break/continue, match/case pattern matching |
| **Functions · 函数** | first-class functions, closures with mutable capture, recursion |
| **OOP & Modules · 面向对象与模块** | class inheritance, method override, module namespaces, from...import |
| **Semantic Library · 语义化标准库** | `lib/` — Palette (gradients, presets), Sky (gradient/sun/stars/clouds/aurora), Terrain (mountains/ground/grass/rocks/dunes), Water (surface/ripples/reflection/waterfall), Vegetation (trees/bushes/flowers/forest), Atmosphere (fog/god_rays/snow/rain/fireflies) |
| **Standard Library · 标准库** | math (trig/log/exp/clamp/lerp), noise (perlin/worley/fbm), color conversion, post-processing (blur/sharpen/grain/vignette), string utilities |
| **AI-Ready · AI 友好** | Semantic replicate mode: image→code with K-means palette extraction + structure analysis (2800x smaller than progressive mode) |
| **Quality · 质量** | SDF anti-aliasing, premultiplied alpha, sub-pixel scanlines, sRGB linear workflow, miter/bevel/round stroke joins |

---

## Quick Start · 快速上手

### Install · 安装

```bash
git clone https://github.com/Whatsmore-nf/Garden-VGL.git
cd Garden-VGL
cargo build --release
```

The single binary `target/release/vgl.exe` (or `vgl` on Linux/macOS) is ready to use.

编译后得到单文件 `target/release/vgl.exe`（或 Linux/macOS 下的 `vgl`），可直接使用。

### Your First Image · 第一张图

Create `sunset.vgl` / 创建 `sunset.vgl`：

```vgl
import "lib/sky.vgl"
import "lib/terrain.vgl"

from Sky import gradient, sun, clouds
from Terrain import mountains, ground

canvas 800x600
seed 42

let sky_top = color(30, 20, 60)
let sky_horizon = color(255, 180, 100)
let mountain_col = color(45, 40, 65)
let ground_col = color(40, 35, 50)

gradient(sky_top, sky_horizon)
sun(400, 100, color: color(255, 220, 150))
clouds(count: 5, base_color: color(255, 240, 200), opacity: 0.4)
mountains(2, [mountain_col, color(30, 25, 45)], 380, spacing: 40)
ground(390, color: ground_col, noise_amount: 0.2)

render "sunset.png"
```

Run it / 运行：

```bash
vgl sunset.vgl
```

This code is a **generation blueprint** — change a color variable, adjust mountain count, or swap the sun position, and the entire image updates. No pixel-by-pixel editing needed.

这段代码是一个**生成蓝图**——修改一个颜色变量、调整山脉数量或移动太阳位置，整张图片就会更新。无需逐像素编辑。

---

## Learn the Language · 学习语言

| Resource · 资源 | Description · 说明 |
|----------------|-------------------|
| [📘 VGL 快速入门](docs/VGL-快速入门.md) | 中文教程：从零到上手，12 个步骤 + 进阶技巧 |
| [📕 VGL Quick Reference](docs/VGL-Quick-Reference.md) | English concise language reference for AI (~900 lines) |
| [📗 完整语法规范](docs/VGL_语法规范%20v0.5.txt) | 中文完整 EBNF 语法 + 版本历史 |
| [📺 Wiki](https://github.com/Whatsmore-nf/Garden-VGL/wiki) | Development workflow, architecture, version history |

## Examples · 示例

| Script · 脚本 | Highlights · 亮点 |
|--------|-----------|
| [demo.vgl](examples/demo.vgl) | Basic drawing primitives / 基础绘图 |
| [rings.vgl](examples/rings.vgl) | Concentric circle patterns / 同心圆 |
| [v07_demo.vgl](examples/v07_demo.vgl) | Standard library / 标准库演示 |
| [v075_demo.vgl](examples/v075_demo.vgl) | Drawing primitives + material presets / 绘图原语 + 材质预设 |
| [v08_demo.vgl](examples/v08_demo.vgl) | v0.8: gradients, transforms, clipping, text, Perlin noise |
| [v09_demo.vgl](examples/v09_demo.vgl) | v0.9: bitwise ops, match, enum, class, modules, sRGB linear |
| [showcase_semantic.vgl](examples/showcase_semantic.vgl) | **v1.0: semantic generation showcase** — sunset scene using lib/ standard library / 语义化生成展示 |

---

## CLI · 命令行

```bash
# Run a VGL script · 运行 VGL 脚本
vgl [--continue-on-error] <file.vgl>

# Semantic replicate (recommended) · 语义化复刻（推荐）
# 分析图像结构，生成可编辑的语义化生成蓝图（~1KB / 40行）
vgl replicate --mode semantic <input.png> <output.vgl>

# Legacy modes · 传统模式
vgl replicate --mode pixel <input.png> <output.vgl>
vgl replicate --mode block <input.png> <output.vgl> [--block-size 16]
vgl replicate --mode progressive <input.png> <output.vgl> [--layers 32,8,1] [--threshold 30]
```

### Semantic Replicate vs Progressive · 语义化复刻 vs 渐进法

| | Semantic 语义化 | Progressive 渐进法 |
|---|---|---|
| **Output size** | ~1 KB / 40 lines | ~2.8 MB / 65,000 lines |
| **Compression** | **2800x smaller** | baseline |
| **AI-editable** | ✅ Yes — variables, functions, clear structure | ❌ No — raw pixel coordinates |
| **Visual fidelity** | Semantic approximation | Near-lossless |
| **Use case** | AI editing, creative iteration | Pixel-perfect archival |

Semantic mode analyzes the image (K-means color clustering, horizon detection, brightness gradient, color temperature) and generates VGL code using the `lib/` semantic standard library. The output is a readable, editable generation blueprint.

语义化模式分析图像（K-means 颜色聚类、地平线检测、亮度梯度、色温分类），使用 `lib/` 语义化标准库生成 VGL 代码。输出是可读、可编辑的生成蓝图。

---

## Project Philosophy · 设计理念

| English | 中文 |
|---------|------|
| **Living Blueprint**: Code describes *how* to generate, not *what each pixel is* — variables, functions, and materials make every parameter editable | **活的蓝图**：代码描述"如何生成"，而非"每个像素是什么"——变量、函数、材质让每个参数都可编辑 |
| **AI-Native**: Designed for AI understanding and procedural editing — semantic replicate produces 1KB of readable code instead of 65K lines of coordinates | **AI 原生**：为 AI 理解和程序化编辑而设计——语义化复刻生成 1KB 可读代码而非 65K 行坐标 |
| **Semantic Library**: `lib/` standard library provides high-level scene primitives (sky, terrain, water, vegetation, atmosphere) | **语义化标准库**：`lib/` 标准库提供高层场景原语（天空、地形、水面、植被、大气） |
| **Lightweight**: Single Rust binary, no GPU, no heavy ML frameworks | **轻量**：纯 Rust 单文件，无 GPU 依赖，无重型 AI 框架 |
| **Quality**: SDF anti-aliasing, premultiplied alpha, sub-pixel scanlines, sRGB linear workflow | **质量**：SDF 抗锯齿、premultiply 合成、子像素精度、sRGB 线性工作流 |

---

## Version History · 版本历史

| Version · 版本 | Theme · 主题 |
|---------|-------|
| v0.1–v0.2.1 | Python prototype / Python 原型期 |
| v0.3 | Control flow & geometry / 控制流与几何 |
| v0.4 | Block scoping / 块作用域 |
| v0.5 | Data structures & rendering / 数据结构与渲染 |
| v0.55 | Rust rewrite + floating-point canvas / Rust 重构 + 浮点画布 |
| v0.6 | Image replication toolchain / 图像复刻工具链 |
| v0.7 | Standard library expansion / 标准库扩展 |
| v0.75 | Drawing primitives + material presets / 绘图原语 + 材质预设 |
| **v0.8** | **Image quality leap** (SDF AA, premultiply, gradients, transform, text) |
| **v0.9** | **Language completeness** (bitwise, match, enum, class, modules, sRGB linear) |
| **v1.0** | **Semantic generation** — `lib/` standard library (6 modules, 50+ scene primitives), semantic replicate mode, `width()`/`height()` builtins |

---

## License · 许可证

MIT
