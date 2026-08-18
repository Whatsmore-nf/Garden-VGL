# VGL v2.0 快速入门

VGL 是一门输出 **SVG** 的过程式图形语言。这份教程带你从零写出第一张矢量图。

> 环境：`cargo build` 编译出 `vgl` 二进制，`vgl <file.vgl>` 运行脚本，输出 SVG 文件。

---

## 1. 第一张图

```vgl
canvas 400x300
background(color(30, 20, 60))
circle(200, 150, 60, fill: "#ff6b6b")
render "first.svg"
```

要点：

- `canvas 400x300` — 声明画布（宽x高，`x` 是分隔符）
- `color(r, g, b)` / `color(r, g, b, a)` — 构造颜色，也可直接写 `"#ff6b6b"`
- `render "xxx.svg"` — 输出文件（路径相对脚本所在目录）

## 2. 形状与通用命名参数

所有形状共享一组命名参数：

```vgl
rect(x, y, w, h, fill:, stroke:, stroke_width:, rx:, opacity:, blur:)
circle(cx, cy, r, ...)
ellipse(cx, cy, rx, ry, ...)
line(x1, y1, x2, y2, stroke:, width:, cap: "round"|"butt"|"square")
polygon([x1, y1, x2, y2, ...], fill: ..., opacity: ...)
polyline([x1, y1, ...], stroke:, width:, join: "round"|"miter"|"bevel")
path(d, stroke:, fill: ..., width:)
text(x, y, "内容", size:, fill:, anchor: "start"|"middle"|"end", weight:, font:)
```

示例：

```vgl
rect(50, 50, 120, 80, fill: "#4ecdc4", rx: 10, opacity: 0.9)
line(50, 200, 350, 200, stroke: "#fff", width: 2, cap: "round")
text(200, 280, "你好 VGL", size: 24, fill: "#ffe66e", anchor: "middle")
```

- `fill: none` 表示不填充（`none` 是关键字）
- `stroke_width` 可简写 `width`（line 上）
- `blur: 5` 给该元素加高斯模糊（生成 SVG filter）

## 3. 渐变

渐变是"值"，可以存进变量、当作 `fill` / `stroke` 传入：

```vgl
let dusk = linear_gradient([color(34, 24, 64), color(255, 170, 110)])
background(dusk)

let glow = radial_gradient([color(255, 220, 150), alpha(color(255, 180, 100), 0)])
circle(500, 200, 120, fill: glow)
```

- `linear_gradient(colors, x1:, y1:, x2:, y2:)` — 默认从画布顶到底
- `radial_gradient(colors, cx:, cy:, r:)` — 默认画布中心
- `alpha(col, a)` — 改透明度，用于让渐变两端淡出

## 4. 变量、数组与函数

```vgl
let cols = ["#ff6b6b", "#4ecdc4", "#ffe66e"]   // 数组
let i = 1
print(cols[i])                                  // 取色

fn dot_row(y, n = 8, r = 10) {                  // 默认参数
    for i in 0..n {
        let x = 40 + i * 50
        circle(x, y, r + fbm(i * 0.3, 0) * 4, fill: cols[i % 3])
    }
}

dot_row(100)        // 用默认 n=8, r=10
dot_row(200, 12, 6) // 全部自定义
```

注意：VGL 没有裸赋值（`x = 1` 非法），定义变量一律 `let`；函数通过 `return` 返回值。

## 5. 控制流

```vgl
for i in 0..10 { ... }          // 0..9（含头不含尾）
for x in 0..800 step 40 { ... } // 步进
while n < 100 { ... }
if a > b { ... } else if a == b { ... } else { ... }
break / continue                // 循环内跳出/下一轮
```

`for i in 0..n` 的 i 是数字（允许浮点边界）。

## 6. 确定性随机与噪声

```vgl
seed 42                 // 设定种子：之后所有 rand/perlin/fbm 都可复现

rand(0, 100)            // 均匀浮点 [0, 100)
rand_int(1, 6)          // 整数掷骰
perlin(x, y)            // 柏林噪声，约 [-1, 1]
fbm(x, y, 4)            // 分形叠加（octaves 1..8），更自然的起伏
```

同一 `seed` 永远生成同一张图 — 这是"生成蓝图"可复现的基础。

## 7. 平滑路径与曲线

`smooth(points)` 把 `[x1, y1, x2, y2, ...]` 点列转成 Catmull-Rom 平滑的 SVG 路径：

```vgl
let pts = []
for i in 0..20 {
    push(pts, 20 + i * 38)
    push(pts, 300 + sin(i * 0.6) * 60)
}
path(smooth(pts), stroke: "#4ecdc4", width: 3)
// path(smooth(pts, closed: true), fill: "#4ecdc4", opacity: 0.5)  // 闭合填充
```

## 8. group 变换

```vgl
group(translate: [400, 300], rotate: 15, scale: 1.2, opacity: 0.8, blur: 2) {
    rect(-40, -15, 80, 30, fill: "#4ecdc4")
    text(0, 5, "中心", anchor: "middle")
}
```

组内坐标以组为原点，可任意嵌套。

## 9. 使用语义库 lib/

库本身用 VGL 编写，`use` 引入（相对当前脚本路径）：

```vgl
use "../lib/palette.vgl"
use "../lib/sky.vgl"
use "../lib/terrain.vgl"

canvas 1000x700
seed 7

let cols = palette("dusk")
sky(cols[0], cols[1])
sun(500, 200, 40, cols[2])
mountains(4, 500)
render "sunset.svg"
```

常用库速查：

| 库 | 函数 |
|----|------|
| palette | `palette(name)` `palette_sky(name)` `palette_pick(cols, i)` |
| sky | `sky(top, bottom)` `sun(x,y,r)` `moon(x,y,r)` `stars(n)` `meteor(x,y,len,angle)` `clouds(n)` `cloud_band(y)` `aurora(n)` |
| terrain | `ridge(y,amp,freq,col,off)` `mountains(layers,y)` `snow_cap(...)` `dunes(y,amp,col)` |
| water | `water(y)` `waves(y,rows)` `shimmer(x,y,w,col)` |
| vegetation | `tree(x,ground,s,kind)` `forest(n)` `grass(y)` |
| atmosphere | `fog(y)` `ground_fog(y)` `rain(n)` `snow(n)` `light_rays(x,y)` `vignette(s)` `tint(col,s)` |

## 10. 完整练习：黄昏山水

见 [examples/landscape.vgl](../examples/landscape.vgl)，核心思路：

1. `palette("dusk")` 定调色
2. `sky` + `sun` 铺天空
3. `mountains` 由远及近叠山，`snow_cap` 加雪线
4. `water` + `waves` + `shimmer` 画湖
5. `forest` + `grass` 长植被
6. `ground_fog` + `vignette` 收尾氛围

改 `seed` 换构图，改 palette 换时间（`"dawn"`/`"noon"`/`"night"`），改参数改细节 — 这就是"活的生成蓝图"。

---

## 速查：全部内建函数

| 类别 | 函数 |
|------|------|
| 画布 | `canvas WxH` `seed N` `render "f.svg"` `background(paint)` `width()` `height()` |
| 形状 | `rect` `circle` `ellipse` `line` `polygon` `polyline` `path` `text` |
| 渐变 | `linear_gradient(stops, x1:,y1:,x2:,y2:)` `radial_gradient(stops, cx:,cy:,r:)` |
| 路径 | `smooth(points, closed:)` |
| 数学 | `sin cos tan atan2 abs floor ceil round sqrt exp log pow min max clamp lerp` |
| 随机 | `rand(a,b)` `rand_int(a,b)` `perlin(x,y)` `fbm(x,y,oct)` |
| 颜色 | `color(r,g,b,a)` `lighten(c,t)` `darken(c,t)` `lerp_color(a,b,t)` `alpha(c,a)` `red green blue` |
| 数组 | `len(a)` `push(a,v)` |
| 其他 | `print(...)` `TAU` 常量 |

关键字：`let fn if else while for in return break continue use canvas seed render true false none`
