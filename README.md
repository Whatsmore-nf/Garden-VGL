<h1 align="center">🍃 VGL (Visual Graphics Language)</h1>

<p align="center">
  <em>Write code. Generate images. No GPU required.</em>
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> ·
  <a href="docs/VGL-Quick-Reference.md">Language Reference</a> ·
  <a href="examples">Examples</a> ·
  <a href="docs/VGL_语法规范%20v0.5.txt">Full Specification</a>
</p>

---

VGL is a **minimal domain-specific scripting language** for procedural image generation. Built entirely in Rust — one binary, zero dependencies, no GPU needed.

```vgl
canvas 800x600
bg #1a1a2e

fill_circle(128, 128, 60, (200, 80, 80))
fill_rect(20, 20, 50, 30, (80, 200, 100))

render "hello.png"
```

## ✨ Features

| Category | Capabilities |
|----------|-------------|
| **Drawing** | pixel, stroke (AA lines/circles/beziers), fill (rect/circle/ellipse/polygon/flood), gradient (linear/radial), text (5×7 dot matrix) |
| **Transform & Clip** | translate/rotate/scale, push/pop transform stack, clip rect with intersection stack |
| **Materials & Layers** | material definitions with noise perturbation, off-screen layer compositing (12 blend modes), color fields |
| **Data Types** | number, bool, string, color (with alpha), tuple, array, dict, struct, enum, class, path |
| **Control Flow** | if/else-if/else, while, for range, for-in array, labeled break/continue, match/case pattern matching |
| **Functions** | first-class functions, closures with mutable capture, recursion |
| **OOP & Modules** | class inheritance, method override, module namespaces, from...import |
| **Standard Library** | math (trig/log/exp/clamp/lerp), noise (perlin/worley/fbm), color conversion, post-processing (blur/sharpen/grain/vignette), string utilities |
| **Quality** | SDF anti-aliasing, premultiplied alpha, sub-pixel scanlines, sRGB linear workflow, miter/bevel/round stroke joins |

## Quick Start

### Install

```bash
git clone https://github.com/Whatsmore-nf/Garden-VGL.git
cd Garden-VGL
cargo build --release
```

The single binary `target/release/vgl.exe` (or `vgl` on Linux) is ready to use.

### Your First Image

Create `hello.vgl`:

```vgl
canvas 256x256
bg #1a1a2e
fill_circle(128, 128, 60, (200, 80, 80))
fill_rect(20, 20, 50, 30, (80, 200, 100))
render "hello.png"
```

Run it:

```bash
vgl hello.vgl
```

Open `hello.png` — congratulations, you just generated your first procedural image.

## Examples

All examples are in the [`examples/`](examples) directory:

| Script | Highlights |
|--------|-----------|
| [demo.vgl](examples/demo.vgl) | Basic drawing primitives |
| [rings.vgl](examples/rings.vgl) | Concentric circle patterns |
| [v07_demo.vgl](examples/v07_demo.vgl) | Standard library demo (gradients, post-processing) |
| [v075_demo.vgl](examples/v075_demo.vgl) | Drawing primitives + material presets |
| [v08_demo.vgl](examples/v08_demo.vgl) | v0.8 full demo (gradients, transforms, clipping, text, Perlin noise) |
| [v09_demo.vgl](examples/v09_demo.vgl) | v0.9 full demo (bitwise ops, match, enum, class, modules, sRGB linear) |

## Learn the Language

- **[VGL Quick Reference](docs/VGL-Quick-Reference.md)** — Concise, AI-friendly language reference with all syntax, types, and examples (~900 lines)
- **[Full Specification](docs/VGL_语法规范%20v0.5.txt)** — Complete EBNF grammar, version history, and implementation notes (Chinese)
- **[Wiki](https://github.com/Whatsmore-nf/Garden-VGL/wiki)** — Development workflow, architecture, version history, feature overview

## CLI

```bash
vgl [--continue-on-error] <file.vgl>

# Image replication (convert PNG to VGL code)
vgl replicate --mode pixel <input.png> <output.vgl>
vgl replicate --mode block <input.png> <output.vgl> [--block-size 16]
vgl replicate --mode progressive <input.png> <output.vgl> [--layers 32,8,1] [--threshold 30]
```

## Project Philosophy

- **Lightweight**: Single Rust binary, no GPU, no heavy ML frameworks (no PyTorch/MNN/ONNX)
- **Expressive**: Minimal syntax, maximum visual output — tuple broadcasting alone reduces geometry code by 5-10x
- **Quality**: SDF anti-aliasing, premultiplied alpha compositing, sub-pixel scanlines, sRGB linear workflow
- **Complete**: All reserved keywords implemented as of v0.9

## Version History

| Version | Theme |
|---------|-------|
| v0.1–v0.2.1 | Python prototype |
| v0.3 | Control flow & geometry |
| v0.4 | Block scoping |
| v0.5 | Data structures & rendering |
| v0.55 | Rust rewrite + floating-point canvas |
| v0.6 | Image replication toolchain |
| v0.7 | Standard library expansion |
| v0.75 | Drawing primitives + material presets |
| **v0.8** | **Image quality leap** (SDF AA, premultiply, gradients, transform, text) |
| **v0.9** | **Language completeness** (bitwise, match, enum, class, modules, sRGB linear) |

## License

MIT
