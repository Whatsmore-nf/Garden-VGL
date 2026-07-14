<h1 align="center">🍃 VGL (Visual Graphics Language)</h1>

<p align="center">
  <em>Write code. Generate images. No GPU required.</em><br>
  <em>写代码，生成图片。不用 GPU，不要显卡。</em>
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> ·
  <a href="docs/VGL-快速入门.md">快速入门</a> ·
  <a href="docs/VGL-Quick-Reference.md">Language Reference</a> ·
  <a href="docs/VGL_语法规范%20v0.5.txt">语法规范</a> ·
  <a href="examples">Examples</a>
</p>

---

VGL is a **minimal domain-specific scripting language** for procedural image generation. Built entirely in Rust — one binary, zero dependencies, no GPU needed.

VGL 是一个极简的**程序化图像生成语言**。纯 Rust 构建——单文件二进制，零外部依赖，不需要显卡。

```vgl
canvas 800x600
bg #1a1a2e

fill_circle(128, 128, 60, (200, 80, 80))
fill_rect(20, 20, 50, 30, (80, 200, 100))

render "hello.png"
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
| **Standard Library · 标准库** | math (trig/log/exp/clamp/lerp), noise (perlin/worley/fbm), color conversion, post-processing (blur/sharpen/grain/vignette), string utilities |
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

Create `hello.vgl` / 创建 `hello.vgl`：

```vgl
canvas 256x256
bg #1a1a2e
fill_circle(128, 128, 60, (200, 80, 80))
fill_rect(20, 20, 50, 30, (80, 200, 100))
render "hello.png"
```

Run it / 运行：

```bash
vgl hello.vgl
```

Open `hello.png` — you just generated your first procedural image.

打开 `hello.png`——你生成了第一张程序化图像。

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

---

## CLI · 命令行

```bash
vgl [--continue-on-error] <file.vgl>

# Image replication · 图像复刻
vgl replicate --mode pixel <input.png> <output.vgl>
vgl replicate --mode block <input.png> <output.vgl> [--block-size 16]
vgl replicate --mode progressive <input.png> <output.vgl> [--layers 32,8,1] [--threshold 30]
```

---

## Project Philosophy · 设计理念

| English | 中文 |
|---------|------|
| **Lightweight**: Single Rust binary, no GPU, no heavy ML frameworks | **轻量**：纯 Rust 单文件，无 GPU 依赖，无重型 AI 框架 |
| **Expressive**: Minimal syntax, maximum visual output — tuple broadcasting reduces geometry code 5-10x | **高效**：最小语法 + 最大输出——元组广播让几何代码减少 5-10 倍 |
| **Quality**: SDF anti-aliasing, premultiplied alpha, sub-pixel scanlines, sRGB linear workflow | **质量**：SDF 抗锯齿、premultiply 合成、子像素精度、sRGB 线性工作流 |
| **Complete**: All reserved keywords implemented as of v0.9 | **完整**：v0.9 起全部保留关键字已实现 |

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

---

## License · 许可证

MIT
