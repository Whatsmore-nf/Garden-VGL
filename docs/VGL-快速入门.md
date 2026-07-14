# VGL 快速入门

> 写代码，生成图片。不用 GPU，不要显卡。

VGL 是一个极简的**程序化图像生成语言**。你写一个 `.vgl` 脚本，运行 `vgl script.vgl`，就得到一张 PNG 图片。

---

## 目录

- [五分钟上手](#五分钟上手)
- [语言速览](#语言速览)
- [一步一步学](#一步一步学)
  - [画布与背景](#1-画布与背景)
  - [像素绘制](#2-像素绘制)
  - [笔触绘制](#3-笔触绘制)
  - [填充](#4-填充)
  - [渐变](#5-渐变)
  - [颜色](#6-颜色)
  - [变量与循环](#7-变量与循环)
  - [函数](#8-函数)
  - [用噪声生成有机纹理](#9-用噪声生成有机纹理)
  - [变换与裁剪](#10-变换与裁剪)
  - [图层合成](#11-图层合成)
  - [材质](#12-材质)
- [进阶技巧](#进阶技巧)
- [下一步](#下一步)

---

## 五分钟上手

安装：

```bash
git clone https://github.com/Whatsmore-nf/Garden-VGL.git
cd Garden-VGL
cargo build --release
```

创建 `hello.vgl`：

```vgl
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

打开 `hello.png`——你生成了第一张程序化图像。

---

## 语言速览

一个 VGL 程序的结构：

```
canvas 800x600          // 必须：设置画布尺寸
bg #1a1a2e              // 可选：背景色
// ... 绘制语句 ...
render "output.png"     // 必须：输出为 PNG
```

所有保留字：

```
canvas  bg  let  const  var  for  in  if  else  fn  return
pixel  stroke  render  while  break  continue  and  or  not
seed  true  false  null  struct  import  material  layer  field
as  match  case  default  enum  class  from  module
```

---

## 一步一步学

### 1. 画布与背景

`canvas` 设置画布宽度 × 高度（像素），`bg` 设置背景色：

```vgl
canvas 400x300
bg #0a0a1a
// 接下来所有绘制都在这个 400×300 的画布上
```

颜色可以用 `#RRGGBB` 格式，也可以用三元组 `(r, g, b)`：

```vgl
bg #ff0000    // 红色背景
bg (0, 0, 255)  // 蓝色背景
```

### 2. 像素绘制

`pixel` 在指定坐标画一个像素：

```vgl
pixel(x: 10, y: 20, rgb: #ff0000)   // 在 (10,20) 画一个红点
pixel(x: x, y: y, rgb: (255, 255, 255))
```

参数必须使用**关键字参数**形式（`x:`, `y:`, `rgb:`）。

### 3. 笔触绘制

`stroke` 是绘制线条和形状的主要方式：

```vgl
// 画一条线段
stroke {
    path: line((0, 0), (100, 100))
    width: 2
    color: #ff0000
}

// 画一个圆
stroke {
    path: circle(200, 150, 50)
    width: 3
    color: #00ff00
}

// 画一条贝塞尔曲线
stroke {
    path: bezier((0, 200), (100, 50), (200, 50), (300, 200))
    width: 2
    color: #ff6699
}
```

`stroke` 块支持这些字段：

| 字段 | 说明 | 必填 |
|------|------|------|
| `path` | 路径（line/circle/bezier/qbezier/path/rect/ellipse/arc/polygon/triangle） | ✅ |
| `width` | 线宽（像素） | ✅ |
| `color` | 颜色（被 material 覆盖时可省略） | ❌ |
| `material` | 材质引用 | ❌ |
| `samples` | 贝塞尔采样点数（默认 64） | ❌ |
| `join` | 折线连接方式：`"round"` / `"miter"` / `"bevel"` | ❌ |

### 4. 填充

VGL 提供了多种填充函数：

```vgl
// 矩形填充
fill_rect(x, y, w, h, (r, g, b))

// 圆形填充
fill_circle(cx, cy, radius, (r, g, b))

// 椭圆填充
fill_ellipse(cx, cy, rx, ry, (r, g, b))

// 多边形填充
fill_polygon(((0, 0), (100, 0), (50, 100)), (255, 200, 50))

// 泛洪填充——替换同色区域
flood_fill(x, y, (r, g, b))
```

### 5. 渐变

渐变填充不需要逐像素循环，一行搞定：

```vcl
// 线性渐变
fill_linear_gradient(0, 0, 400, 300, #ff6600, #003366)

// 径向渐变
fill_radial_gradient(200, 150, 100, #ffffff, #0000ff)
```

### 6. 颜色

颜色处理函数：

```vgl
// RGB ↔ HSL 转换（调色时超有用）
let h = rgb_to_hsl(255, 100, 50)
// → (hue, saturation, lightness)
let c = hsl_to_rgb(0.05, 0.8, 0.6)

// 颜色插值
let mid = lerp_color(#ff0000, #0000ff, 0.5)

// 调整明暗和饱和度
let bright = brighten(color, 50)   // 提亮
let dark = darken(color, 30)       // 变暗
let vivid = saturate(color, 0.3)   // 增加饱和度
```

### 7. 变量与循环

```vgl
// 变量声明
let count = 0
const PI = 3.14159       // 不可修改

// 复合赋值
count += 1
count++                  // 自增

// for 循环遍历坐标范围
for y in 0..300 {
    for x in 0..400 {
        pixel(x: x, y: y, rgb: (x, y, 128))
    }
}

// while 循环
let x = 200
while x > 0 and x < 400 {
    x = x + int(rand(-3, 4))
}

// 遍历数组
let colors = [#ff0000, #00ff00, #0000ff]
for c in colors {
    fill_circle(100, 100, 20, c)
}
```

### 8. 函数

```vgl
// 定义函数
fn star(cx, cy, r) {
    for i in 0..5 {
        let a = i * 1.25663706  // 2*PI/5
        stroke {
            path: line((cx, cy), (cx + r * cos(a), cy + r * sin(a)))
            width: 1
            color: #ffff00
        }
    }
}

// 调用
star(200, 150, 50)

// 闭包（捕获外层变量）
fn make_walker(x0, y0) {
    let state = (x0, y0)
    fn step() {
        let dx = int(rand(-2, 3))
        let dy = int(rand(-2, 3))
        state = (state[0] + dx, state[1] + dy)
        return state
    }
    return step
}
let walk = make_walker(200, 200)
for i in 0..1000 {
    let p = walk()
    pixel(x: p[0], y: p[1], rgb: #ffffff)
}
```

**小技巧**：用 `(p1 + p2) / 2` 代替笨重的 `((p1[0]+p2[0])/2, (p1[1]+p2[1])/2)`，这就是**元组广播**。

### 9. 用噪声生成有机纹理

噪声函数让你的图像摆脱"纯数学"的生硬感：

```vgl
// 云朵纹理
field cloud(x, y) {
    let n = perlin(x * 0.02, y * 0.02)
    let v = int((n + 1) * 127.5)
    return (v, v, int(v * 1.5))
}
fill("cloud")

// 细胞纹理
field cells(x, y) {
    let d = worley(x, y)
    let v = int(d * 8)
    return (v, v, v)
}
fill("cells")

// 分形噪声叠加
field mountains(x, y) {
    let n = fbm(x * 0.01, y * 0.01, 6)
    let v = int((n + 1) * 127.5)
    return (v, v, int(v * 1.2))
}
fill("mountains")
```

`seed` 语句控制噪声的可复现性：

```vgl
seed 42        // 同一种子 → 相同输出
```

### 10. 变换与裁剪

坐标系变换用 `translate` / `rotate` / `scale`，配合变换栈管理：

```vgl
// 平移坐标系到画布中心
translate(320, 240)

// 旋转 45 度并绘制
rotate(pi() / 4)
fill_rect(-50, -50, 100, 100, (200, 80, 80))

// 弹出变换，回到原始坐标系
pop_transform()

// 裁剪矩形区域
clip_rect(50, 50, 200, 150)
// 之后的所有绘制只在这个矩形内可见
fill_circle(200, 175, 60, (80, 160, 200))

// 清除裁剪
clip_clear()
```

变换栈的关键原则：**每次 push_transform 后记得 pop_transform**，否则变换会一直累积。

### 11. 图层合成

把不同视觉元素画到不同图层，最后合成在一起：

```vgl
// 定义图层
layer stars {
    seed 999
    for i in 0..100 {
        let x = int(rand(0, 400))
        let y = int(rand(0, 300))
        let size = int(rand(1, 3))
        fill_circle(x, y, size, (255, 255, 200))
    }
}

layer glow {
    fill_radial_gradient(200, 150, 120, #ffffff, #000000)
}

// 合成到主画布
compose("stars", "add")      // 星星用加法混合变亮
compose("glow", "screen")    // 辉光用滤色混合
```

混合模式对照：

| 模式 | 效果 |
|------|------|
| `"over"` | 普通叠加（默认） |
| `"add"` | 加法混合，亮色叠加 |
| `"mul"` | 乘法混合，暗色叠加 |
| `"screen"` | 滤色混合，变亮 |
| `"overlay"` | 叠加，增强对比 |
| `"soft_light"` / `"hard_light"` | 柔光 / 强光 |
| `"difference"` / `"exclusion"` | 差值 / 排除 |

### 12. 材质

材质让你的笔触有"笔触感"——颜色不是死的，而是带有噪音扰动：

```vgl
// 定义材质
material gold {
    color: (255, 200, 50)
    noise: 0.3       // 亮度扰动强度（0=平涂，1=毛刺）
    alpha: 0.8       // 透明度 [0, 1]
}

// 使用材质
stroke {
    path: line((0, 0), (400, 300))
    width: 8
    material: gold
}

// 使用预设材质库
material water { color: preset("watercolor") }
material oil  { color: preset("oil_painting") }
material neo  { color: preset("neon") }
```

预设材质：`"watercolor"` / `"oil_painting"` / `"neon"` / `"pencil"` / `"crayon"`

---

## 进阶技巧

### 让 AI 帮你写 VGL

把这个文档发给 AI，可以说"用 VGL 画一幅 xxx"，然后给出具体需求。AI 生成的代码质量取决于 prompt 的清晰度——越具体越好。

### 10 条代码优化建议

1. **用元组广播**：`(p1 + p2) / 2` 比 `((p1[0]+p2[0])/2, (p1[1]+p2[1])/2)` 短 5-10 倍
2. **函数封装重复模式**：相同图案用函数参数化，不要复制粘贴
3. **field + fill 代替像素循环**：连续渐变用 field，比硬编码 for 循环快
4. **图层分层**：星星、背景、前景分到不同 layer，用 compose 组合
5. **固定 seed 调试**：调试时用 `seed 42`，跑正式时去掉
6. **stroke + path 代替 fill**：复杂几何图案用路径比逐像素 fill 高效
7. **画布别太大**：800×600 以下渲染快，太大 O(W×H) 会变慢
8. **用噪声生成有机质感**：perlin/worley 比数学公式自然
9. **颜色用 #RRGGBB**：比三元组 `(255,0,0)` 更直观
10. **总是写 bg 和 render**：虽然 render 可以省略，但大多数时候你需要看到输出

### 常见模式模板

```vgl
// 地平线渐变 + 噪声纹理
canvas 640x480
bg #000000

field sky(x, y) {
    let t = y / 480.0
    let n = perlin(x * 0.01, y * 0.01) * 0.2
    return lerp_color(#0a0a2e, #ff6633, t + n)
}
fill("sky")

render "sunset.png"
```

---

## 下一步

- 📖 **完整参考**：[VGL-Quick-Reference.md](VGL-Quick-Reference.md)（英文，AI 友好版，~900 行）
- 📗 **语法规范**：[VGL_语法规范 v0.5.txt](VGL_语法规范%20v0.5.txt)（中文完整版，含 EBNF）
- 🖼️ **示例脚本**：[examples/](../examples) 目录下的 `.vgl` 文件
- 📺 **Wiki**：https://github.com/Whatsmore-nf/Garden-VGL/wiki
