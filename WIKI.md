# Garden-VGL 项目 Wiki

> VGL (Visual Graphics Language) —— 一种用于程序化生成图像的领域专用语言

## 目录

- [项目简介](#项目简介)
- [开发流程](#开发流程)
- [版本迭代历程](#版本迭代历程)
- [技术架构](#技术架构)
- [功能特性](#功能特性)
- [快速上手](#快速上手)

---

## 项目简介

Garden-VGL 是一个用 Rust 编写的轻量级程序化图像生成工具。它定义了一种简洁的脚本语言 VGL，用户通过编写 `.vgl` 脚本即可生成位图图像（PNG）。

**设计目标**：
- 轻量单文件，无 GPU 依赖，不引入重型推理框架
- 语法接近自然描述，聚焦 2D 绘图场景
- 高质量渲染（SDF 抗锯齿、premultiply 合成、子像素精度）

---

## 开发流程

本项目采用**版本驱动的迭代开发**模式，每个版本对应规范文档的一个修订点：

```
规范修订 → 分批次实现 → 编译验证 → 示例测试 → 版本提交 → 文档同步
```

### 开发节奏

1. **规范先行**：每次大版本更新前，先修订 `docs/VGL_语法规范 v0.5.txt`，明确目标语义
2. **分批次实现**：将一个版本拆分为 A/B/C/D... 多个批次，每批次聚焦一类功能
3. **编译验证**：`cargo build --release` 必须通过（允许无害 dead_code 警告）
4. **示例测试**：每个版本附带演示脚本（如 `v08_demo.vgl`），渲染验证
5. **版本统一**：同步更新 `Cargo.toml`、`main.rs` 头注释、规范文档的版本号
6. **Git 提交**：提交信息按 `vX.Y: 概要（批次明细）` 格式

### 工具链

- **语言**：Rust（stable-x86_64-pc-windows-gnu 工具链）
- **依赖**：`rand` 0.8（随机数）、`image` 0.24（PNG 编解码）
- **构建**：`cargo build --release` 生成单文件 `vgl.exe`

---

## 版本迭代历程

### v0.1 ~ v0.2.1（Python 原型期）

| 版本 | 内容 |
|------|------|
| v0.1 | 最小实现：canvas/bg/pixel/stroke/render 基本语句 |
| v0.2 | 运算符优先级、类型系统、作用域、标准库函数签名、错误处理规范 |
| v0.2.1 | 规范与实现对齐修订，标注 [已实现]/[部分实现]/[未实现] |

### v0.3（控制流与几何表达增强）

| 批次 | 内容 |
|------|------|
| 批次 1 | 比较运算符、逻辑运算符（and/or/not 短路）、while 循环、break、seed 随机种子、赋值语句 |
| 批次 2+3 | 元组索引、元组广播运算、贝塞尔曲线（bezier/qbezier）、数学函数（pow/sqrt）、几何函数（dot/length） |
| 批次 4 | 闭包与可变捕获 |

### v0.4（块作用域补全）

- 块作用域 for/if/while/stroke（每次进入块创建子 Environment）
- continue 语句
- 带标签 break（`label: for/while ... break label`）

### v0.5（复合数据结构 + 渲染增强）

| 批次 | 内容 |
|------|------|
| A | struct 类型、array 数组、dict 字典、字符串转义序列 |
| B | import 模块导入（相对路径/循环保护）、错误信息行号列号 caret、stroke Wu 抗锯齿 |
| C | material 材质系统、layer 图层系统、field 颜色场、perlin/worley/fbm 噪声函数 |

### v0.55（Rust 重构 + 浮点画布）

**重大转折**：项目从 Python 完整迁移到 Rust。

- 模块化拆分：lexer/parser/ast/canvas/noise/interp/error 七模块
- 画布浮点化：`Canvas.pixels` 改为 `Vec<f32>`，RGBA 每通道 [0.0, 255.0]
- 真 alpha 合成：source-over 公式 `out_a = sa + da*(1-sa)`
- 逐像素材质 noise：MaterialParams + `_mat` 系列方法
- `load(path)` 加载外部图片
- 警告系统：VglWarning + 重复 bg/函数覆盖检测
- `--continue-on-error` CLI 模式
- 表达式级错误定位：6 类 Expr 节点附加 pos
- tuple 字典序比较

### v0.6（图像复刻工具链）

- `vgl replicate --mode pixel|block|progressive` 子命令
- 模式 A（pixel）：逐像素 1:1 无损复刻
- 模式 C（block）：分块法，每块用平均色填充
- 模式 D（progressive）：渐进法，分层细化
- `pixel_at(img, x, y)` 内建函数

### v0.7（标准库扩展）

| 批次 | 内容 |
|------|------|
| A | 三角函数补全（tan/asin/acos/atan/atan2）、对数指数（log/log2/log10/exp）、round/sign/clamp/lerp/smoothstep、radians/degrees、pi/e |
| B | 颜色函数：rgb_to_hsl/hsl_to_rgb/lerp_color/brighten/darken/saturate |
| C | 后处理：grain/vignette/blur/sharpen |
| D | 混合模式扩展：overlay/soft_light/hard_light/color_dodge/color_burn/linear_burn/difference/exclusion |
| E | 字符串函数：str/concat/substr/upper/lower/find |

### v0.75（绘图原语 + 材质库预设）

- 绘图原语：rect/ellipse/arc/polygon/triangle 路径构造
- 填充函数：fill_rect/fill_circle/fill_ellipse/fill_polygon/flood_fill
- 材质库预设：`preset("watercolor"|"oil_painting"|"neon"|"pencil"|"crayon")`

### v0.8（图像质量飞跃 + 语言完整度）

**大版本更新**，8 个批次全面提升图像质量与语言完整度：

| 批次 | 内容 |
|------|------|
| A | SDF 距离场抗锯齿（brush/draw_circle/draw_line 粗线，2px 过渡带，自带圆头 cap） |
| B | 子像素精度扫描线 fill_polygon + fill_* 改用 put_pixel_rgba 真 source-over |
| C | 线性/径向渐变 fill_linear_gradient/fill_radial_gradient + 5x7 点阵文本 text() |
| D | 2D 仿射变换栈 Transform + translate/rotate/scale + push/pop_transform + 裁剪栈 clip_rect/clip_clear |
| E | premultiply 颜色合成，修复 out_a=0 边界 bug |
| F | 语法补全：% 取模、+= -= *= /= %= 复合赋值、else-if 链、for-in-array |
| G | 可种子化 Perlin（seeded_perm + perlin_seeded）+ eval_error 改为 Err(VglError) |
| H | v08_demo.vgl 综合演示 + 版本号统一 + 规范文档更新 |

### 规范修订（v0.8 后）

- 移除 GPU Compute Shader 后端计划（项目追求轻量，无 GPU 依赖）
- 移除神经风格编码模式 B（不引入 MNN/PyTorch 等重型推理框架）

---

## 技术架构

### 源码模块

```
src/
├── main.rs       — 入点 + CLI 参数解析 + replicate 子命令
├── lexer.rs      — 词法分析器（Token 流生成）
├── parser.rs     — 语法分析器（AST 构建）
├── ast.rs        — 抽象语法树定义（Expr/Stmt/Value/Env）
├── canvas.rs     — 绘图引擎（SDF AA + premultiply 合成 + 子像素精度）
├── noise.rs      — 噪声实现（Perlin + Worley + fBm + 可种子化）
├── interp.rs     — 解释器（树遍历执行 + 变换栈 + 裁剪栈 + 渐变 + 文本）
└── error.rs      — 错误处理（VglError + VglWarning + 行列定位）
```

### 渲染管线

```
.vgl 脚本
   ↓ lexer（词法）
Token 流
   ↓ parser（语法）
AST
   ↓ interp（树遍历执行）
Canvas（Vec<f32> RGBA 浮点缓冲）
   ↓ canvas.render()
PNG 文件
```

### 关键技术

- **SDF 距离场抗锯齿**：`alpha = clamp(half_w + 0.5 - dist, 0, 1)`，2px 过渡带
- **Premultiply 颜色合成**：src/dst 先乘 alpha 再线性混合，修复透明边界 bug
- **子像素精度扫描线**：fill_polygon 取像素中心 y+0.5，边界按覆盖率计算
- **2D 仿射变换栈**：`Transform { a,b,c,d,e,f }` 2x3 矩阵，支持 compose 矩阵乘法
- **可种子化 Perlin**：LCG + Fisher-Yates 洗牌生成置换表，seed 影响噪声输出

---

## 功能特性

### 语法层

- 变量声明 `let`、赋值 `=`、复合赋值 `+= -= *= /= %=`
- 控制流：`if/else/else-if`、`while`、`for i in a..b`、`for x in array`、`break label`、`continue`
- 运算符：算术 `+ - * / %`、比较 `< > <= >= == !=`、逻辑 `and or not`、索引 `[]`、字段 `.`
- 复合类型：tuple、array、dict、struct
- 函数：`fn name(args) { ... }`、闭包、递归
- 模块导入：`import "path.vgl"`

### 绘图层

- 绘图原语：pixel、line、rect、ellipse、arc、circle、polygon、triangle、bezier、qbezier、path
- 填充：fill_rect/fill_circle/fill_ellipse/fill_polygon/flood_fill
- 渐变：fill_linear_gradient/fill_radial_gradient
- 文本：text（5x7 点阵，88 ASCII 字符）
- 变换：translate/rotate/scale/push_transform/pop_transform
- 裁剪：clip_rect/clip_clear
- 材质系统：material 定义 + preset 预设
- 图层系统：layer + compose（over/add/mul/screen/overlay/...）
- 颜色场：field/fill

### 标准库

- 数学：sin/cos/tan/asin/acos/atan/atan2/log/log2/log10/exp/pow/sqrt/round/sign/clamp/lerp/smoothstep/radians/degrees/pi/e
- 随机：rand(a,b)/seed n
- 颜色：rgb_to_hsl/hsl_to_rgb/lerp_color/brighten/darken/saturate
- 噪声：perlin/worley/fbm（可种子化）
- 后处理：grain/vignette/blur/sharpen
- 字符串：str/concat/substr/upper/lower/find
- 图像：load/pixel_at

### CLI

```
vgl [--continue-on-error] <file.vgl>
vgl replicate --mode pixel <input.png> <output.vgl>
vgl replicate --mode block <input.png> <output.vgl> [--block-size 16]
vgl replicate --mode progressive <input.png> <output.vgl> [--layers 32,8,1] [--threshold 30]
```

---

## 快速上手

### 安装

```bash
cargo build --release
# 生成 target/release/vgl.exe（单文件，可移植）
```

### 第一个脚本

创建 `hello.vgl`：

```
canvas 256x256
bg #1a1a2e

fill_circle(128, 128, 60, (200, 80, 80))
fill_rect(20, 20, 50, 30, (80, 200, 100))

render "hello.png"
```

运行：

```bash
vgl hello.vgl
```

### 更多示例

- `examples/demo.vgl` — 基础绘图演示
- `examples/rings.vgl` — 同心圆图案
- `examples/v07_demo.vgl` — 标准库函数演示
- `examples/v075_demo.vgl` — 绘图原语 + 材质预设
- `examples/v08_demo.vgl` — v0.8 全特性综合演示（渐变/变换/裁剪/文本/Perlin）

---

## 完整规范

详见 [`docs/VGL_语法规范 v0.5.txt`](docs/VGL_语法规范%20v0.5.txt)（当前版本 v0.8）。
