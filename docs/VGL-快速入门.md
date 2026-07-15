# VGL（Visual Graphics Language）快速参考

> **面向 AI 的完整语言定义** — 用于程序化生成图像的最小领域专用语言
> 版本 v1.0 · 2026-07-15

---

## 目录

- [一、语言概述](#一语言概述)
- [二、词法规范](#二词法规范)
- [三、语法结构（类 EBNF）](#三语法结构类-ebnf)
- [四、类型系统](#四类型系统)
- [五、运算符优先级与表达式](#五运算符优先级与表达式)
- [六、语句详解](#六语句详解)
- [七、标准库函数](#七标准库函数)
- [八、绘图原语与填充](#八绘图原语与填充)
- [九、材质/图层/颜色场](#九材质图层颜色场)
- [十、高级特性](#十高级特性)
- [十一、编码建议与最佳实践](#十一编码建议与最佳实践)
- [十二、完整示例](#十二完整示例)
- [附录：保留字全表](#附录保留字全表)

---

## 一、语言概述

VGL 是一种面向程序化图像生成的领域特定脚本语言。一个 VGL 程序的核心结构是：

```
设置画布 → 声明背景色 → 绘制内容 → 输出渲染
```

**一句话总结**：写一个 `.vgl` 脚本，运行 `vgl script.vgl`，得到一张 PNG 图片。

### 设计约束

1. 纯文本源码，UTF-8 编码，扩展名 `.vgl`
2. 动态类型，运行时携带类型标签
3. 顺序执行，画布为全局状态
4. 位置参数与关键字参数可混用（v1.0）——位置参数在前，关键字参数在后

### 程序骨架

```
canvas 800x600          // 必须：设置画布宽×高（像素）
bg #1a1a2e              // 可选：背景色（建议显式声明）

// ... 绘制语句 ...

render "output.png"     // 必须：输出为 PNG 文件
```

---

## 二、词法规范

### 2.1 标识符

```
标识符 = 字母 | 下划线，后跟字母/数字/下划线
正则: [a-zA-Z_][a-zA-Z0-9_]*
大小写敏感，建议 ≤64 字符
示例: x, my_var, _temp, strokeCount
```

### 2.2 字面量

| 类型 | 语法 | 示例 |
|------|------|------|
| 整数 | 纯数字 | `0`, `123`, `42` |
| 浮点 | 带小数点 | `3.14`, `0.5`, `.75` |
| 字符串 | 双引号定界 | `"hello.png"`, `"path/output.jpg"` |
| 颜色 3 位 | `#RGB` | `#f00` = (255,0,0), `#ffd` |
| 颜色 6 位 | `#RRGGBB` | `#ff0000`, `#ffd966` |
| 颜色 4 位 | `#RGBA` | `#f00f` = (255,0,0,255) |
| 颜色 8 位 | `#RRGGBBAA` | `#ff0000ff`, `#ff000080`（半透明） |
| bool | 关键字 | `true`, `false` |
| null | 关键字 | `null` |

字符串转义序列：`\n`（换行）、`\t`（制表）、`\r`（回车）、`\\`（反斜杠）、`\"`（双引号）、`\0`（空字符）。未知转义保留原样。

### 2.3 注释

```
// 单行注释 至行尾
/* 块注释，不支持嵌套 */
```

### 2.4 运算符与分隔符

**单字符**：`+` `-` `*` `/` `%` `=` `<` `>` `(` `)` `{` `}` `[` `]` `,` `:` `.`

**双字符**：`..`（范围） `==` `!=` `<=` `>=` `+=` `-=` `*=` `/=` `%=` `<<` `>>` `=>`（match 箭头）

**关键字形式**：`and` `or` `not`

**位运算符**：`&` `|` `^` `~` `<<` `>>`（对整数操作，f64→i64→运算→f64）

**自增自减**：`x++` → `x = x + 1`（语句级语法糖），`x--` → `x = x - 1`

---

## 三、语法结构（类 EBNF）

### 3.1 程序结构

```
程序       = 画布声明, 背景声明?, { 语句 }, 渲染声明
画布声明   = 'canvas', 整数, 'x', 整数
背景声明   = 'bg', 颜色字面量
渲染声明   = 'render', 字符串
```

### 3.2 语句总览

```
语句 =
    变量声明    | 常量声明   | 赋值语句    | 复合赋值
  | if 语句     | for 循环   | while 循环  | for-in 遍历
  | break 语句  | continue   | 函数定义    | return 语句
  | match 语句  | pixel 绘制 | stroke 绘制 | seed 随机种子
  | struct 定义 | enum 定义  | class 定义  | material 定义
  | layer 定义  | field 定义 | import 导入 | from 导入
  | module 定义 | 索引赋值   | 字段赋值    | 表达式语句
```

### 3.3 完整语句定义

```
<变量声明>      ::= 'let' <标识符> '=' <表达式>
<元组解构>      ::= 'let' '(' <标识符> { ',' <标识符> } ')' '=' <表达式>   // v1.0 元组解构
<常量声明>      ::= 'const' <标识符> '=' <表达式>        // 不可修改
<可变声明>      ::= 'var' <标识符> '=' <表达式>           // let 别名
<赋值语句>      ::= <标识符> '=' <表达式>
<复合赋值>      ::= <标识符> ('+=' | '-=' | '*=' | '/=' | '%=') <表达式>
<自增自减>      ::= <标识符> '++' | <标识符> '--'        // 语句级

<if 语句>       ::= 'if' <表达式> '{' { <语句> } '}' [ 'else' ( '{'{<语句>'}' | <if 语句> ) ]
<for 循环>      ::= 'for' <标识符> 'in' <表达式> '..' <表达式> [ 'step' <表达式> ] '{' { <语句> } '}'   // v1.0 step
<while 循环>    ::= 'while' <表达式> '{' { <语句> } '}'
<for-in 遍历>   ::= 'for' <标识符> 'in' <表达式> '{' { <语句> } '}'   // 遍历数组/元组

<break>         ::= 'break' [ <标识符> ]                             // 可选标签
<continue>      ::= 'continue'

<函数定义>      ::= 'fn' <标识符> '(' [ <参数列表> ] ')' '{' { <语句> } '}'
<参数列表>      ::= <参数> { ',' <参数> }
<参数>          ::= <标识符> [ '=' <表达式> ]                // v1.0 默认参数值
<调用实参>      ::= [ <表达式> { ',' <表达式> } ] { <标识符> ':' <表达式> }   // v1.0 先位置后命名
<return>        ::= 'return' <表达式>

<match 语句>    ::= 'match' <表达式> '{' { <case> } [ 'default' '=>' '{' <语句块> '}' ] '}'
<case>          ::= 'case' <模式> '=>' '{' { <语句> } '}'
<模式>          ::= <字面量> | <标识符> | '_' | '(' <模式> { ',' <模式> } ')'

<pixel 绘制>    ::= 'pixel' '(' 'x' ':' <表达式> ',' 'y' ':' <表达式> ',' 'rgb' ':' <表达式> ')'
<stroke 绘制>   ::= 'stroke' '{' { <字段名> ':' <表达式> } '}'
<seed>          ::= 'seed' <整数>

<struct 定义>   ::= 'struct' <标识符> '{' { <字段名> ':' <表达式> [','] } '}'
<enum 定义>     ::= 'enum' <标识符> '{' { <标识符> [ '(' [ <参数列表> ] ')' ] [','] } '}'
<class 定义>    ::= 'class' <标识符> [ ':' <标识符> ] '{' { <字段声明> | <方法定义> } '}'
<字段声明>      ::= <标识符> ':' <类型> '=' <表达式>
<方法定义>      ::= 'fn' <标识符> '(' 'self' [',' <参数列表>] ')' '{' { <语句> } '}'

<material 定义> ::= 'material' <标识符> '{' { <关键字参数> } '}'
<layer 定义>    ::= 'layer' <标识符> '{' { <语句> } '}'
<field 定义>    ::= 'field' <标识符> '(' [ <参数列表> ] ')' '{' { <语句> } '}'

<import 导入>   ::= 'import' <字符串>
<from 导入>     ::= 'from' <字符串> 'import' ( <标识符> { ',' <标识符> } | '*' )
<module 定义>   ::= 'module' <标识符> '{' { <语句> } '}'

<索引赋值>      ::= <标识符> '[' <表达式> ']' '=' <表达式>
<字段赋值>      ::= <标识符> '.' <标识符> '=' <表达式>
<表达式语句>    ::= <表达式>
```

---

## 四、类型系统

### 4.1 运行时类型全表

| 类型 | 表示 | 用途 |
|------|------|------|
| `number` | f64 | 数值（整数/浮点统一） |
| `bool` | true/false | 条件判断 |
| `string` | UTF-8 字符串 | 文件路径、文本 |
| `color` | (r,g,b) 或 (r,g,b,a) | 颜色，各分量 0-255 |
| `tuple` | 有序异构列表 | 坐标点、颜色、返回值 |
| `array` | 可变有序容器 | 动态集合 |
| `dict` | 键值映射 | 数据结构 |
| `struct` | 自定义字段集合 | 状态对象 |
| `enum` | 带标签的变体 | 枚举类型 |
| `class` | 类实例 | 面向对象 |
| `path` | 不透明绘图对象 | stroke 的 path 字段 |
| `material` | 材质对象 | stroke 材质属性 |
| `layer` | 离屏缓冲区 | 图层合成 |
| `image` | 加载的图片 | load() 返回值 |
| `closure` | 函数＋捕获环境 | 函数/闭包 |
| `module` | 命名空间 | 模块导入 |
| `none` | null | 函数无返回值 |

### 4.2 隐式类型转换

- **number ↔ bool**：非 0 为 true，0 为 false；true→1，false→0
- **color ↔ tuple(3)**：运行时同构，`(255,0,0)` 即红色
- **color ↔ tuple(4)**：`(255,0,0,255)` 即红色（带 alpha）
- 无自动字符串转换，无隐式跨类型算术

### 4.3 元组广播运算

简化几何计算的关键设计：

```
(1,2) + (3,4) → (4,6)       // 逐元素，长度必须相同
(10,20) * 2   → (20,40)      // 标量广播
2 * (10,20)   → (20,40)      // 交换律
(20,40) / 2   → (10,20)      // 标量除法
(10,20) + 5   → TypeError    // 非法：歧义
(10,20) * (3,4) → TypeError  // 非法：点积用 dot()
```

---

## 五、运算符优先级与表达式

### 5.1 优先级表（高→低）

| 优先级 | 运算符 | 结合性 | 说明 |
|--------|--------|--------|------|
| 10 | `.` `[ ]` `( )` | 左结合 | 字段访问、索引、调用 |
| 9 | `~` `-`（一元）`not` | 右结合 | 位非、负号、逻辑非 |
| 8 | `*` `/` `%` | 左结合 | 乘法级 |
| 7 | `<<` `>>` | 左结合 | 移位 |
| 6 | `+` `-` | 左结合 | 加法级 |
| 5 | `&` | 左结合 | 位与 |
| 4 | `^` | 左结合 | 位异或 |
| 3 | `|` | 左结合 | 位或 |
| 2 | `<` `>` `<=` `>=` `==` `!=` | 无结合 | 比较 |
| 1 | `and` | 左结合 | 逻辑与（短路） |
| 0 | `or` | 左结合 | 逻辑或（短路） |

### 5.2 表达式节点

```
<表达式> =
    <数字> | <字符串> | <颜色> | <bool> | <null> | <标识符>
  | '(' <表达式> ')'                                // 分组
  | '(' <表达式> ',' { <表达式> } ')'               // 元组
  | '[' [ <表达式> { ',' <表达式> } ] ']'           // 数组
  | <表达式> <二元运算符> <表达式>                   // 二元运算
  | ('-' | 'not' | '~') <表达式>                    // 一元运算
  | <表达式> '[' <表达式> ']'                        // 索引
  | <表达式> '.' <标识符>                            // 字段访问
  | <表达式> '(' [ <参数列表> ] ')'                  // 函数调用
  | <表达式> 'as' <类型名>                           // 类型转换
```

**类型名**：`int` `float` `bool` `str` `color`

**类型转换示例**：
```
3.14 as int      → 3        // 截断
255 as float     → 255.0
0 as bool        → false
42 as str        → "42"
"#ff0000" as color → (255, 0, 0)
```

---

## 六、语句详解

### 6.1 变量/常量/赋值

```
let count = 0            // 可变声明
const PI = 3.14159       // 不可变常量，后续赋值报错
var total = 100          // 显式可变（let 别名）

count = 5                // 赋值（沿作用域链查找）
count += 1               // 复合赋值：count = count + 1
total -= 10              // total = total - 10
total++                  // 自增：total = total + 1
total--                  // 自减：total = total - 1
```

### 6.2 循环

```
// 范围 for 循环（推荐用于遍历坐标）
for x in 0..256 {                  // [0, 256) 步长 1
    pixel(x: x, y: y, rgb: ...)
}

// 带 step 的范围 for 循环（v1.0）
for x in 0..256 step 4 {           // [0, 256) 步长 4
    fill_rect(x, 0, 4, 4, #ffffff)
}

// 带标签的 for（用于 break 跳出外层）
outer: for i in 0..10 {
    for j in 0..10 {
        if i == 3 and j == 3 { break outer }
    }
}

// while 循环（适合不确定次数）
let x = 0
while x < 400 and x > 0 {
    x = x + int(rand(-2, 3))
}

// for-in 遍历数组/元组
let colors = [#ff0000, #00ff00, #0000ff]
for c in colors {
    // 遍历每个颜色
}
```

### 6.3 条件

```
// if-else
if x - 100 {
    // x ≠ 100 时为真
} else {
    // x == 100 时执行
}

// else-if 链
if x > 200 {
    // 大
} else if x > 100 {
    // 中
} else {
    // 小
}

// match/case 模式匹配（v0.9）
match color {
    case #ff0000 => {
        stroke { ... }
    }
    case #00ff00 => {
        stroke { ... }
    }
    default => {
        // 未匹配的默认分支
    }
}
```

**模式支持**：
- 字面量匹配：`case 42 =>`, `case "hello" =>`, `case #ff0000 =>`
- 变量绑定：`case x =>`（匹配任意值并绑定到 x）
- 通配符：`case _ =>`（匹配任意值不绑定）
- tuple 解构：`case (a, b) =>`, `case (x, y, z) =>`
- enum 匹配：`case Color.Red =>`, `case Color.RGB(r, g, b) =>`

### 6.4 函数

```
fn add(a, b) {
    return a + b
}

// 默认参数值（v1.0）
fn draw_dot(x, y, r = 5, fill = color(255, 255, 255)) {
    fill_circle(x, y, r, fill)
}
draw_dot(100, 100)                       // 全部使用默认值
draw_dot(100, 100, 10)                   // 位置参数覆盖
draw_dot(100, 100, fill: #ff0000)        // 命名参数（用 ':' 而非 '='）
draw_dot(100, 100, 8, fill: #00ff00)     // 混合：位置参数在前，命名参数在后
// 注意：命名参数名必须与形参名一致，否则触发运行时错误

// 元组解构（v1.0）
let (h, s, l) = rgb_to_hsl(255, 100, 50)

// 闭包（捕获外层变量）
fn make_counter() {
    let count = 0
    fn inc() {
        count = count + 1       // 可变捕获
        return count
    }
    return inc
}
let c = make_counter()
c()  // → 1
c()  // → 2

// 可递归
fn factorial(n) {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}
```

### 6.5 数组/字典/struct

```
// 数组
let arr = [1, 2, 3]
arr[0] = 100           // 索引赋值
push(arr, 99)          // 追加
let v = pop(arr)       // 弹出
len(arr)               // 长度

// 字典
let d = dict("name", "Alice", "age", 30)
d["extra"] = 100       // 创建新键
has(d, "name")         // → true
keys(d)                // → ["name", "age", "extra"]

// struct
struct Point { x: 0, y: 0 }
let p = Point(x: 10, y: 20)    // 关键字参数
let q = Point(30, 40)           // 位置参数
p.x = 15                        // 字段赋值
let vx = p.x                    // 字段访问
```

### 6.6 enum 枚举

```
enum ColorMode {
    RGB,                    // 无关联值
    HSL = (h, s, l)        // 带关联值
}

let mode = ColorMode.RGB
let hsl = ColorMode.HSL(0.5, 0.8, 0.3)

match mode {
    case ColorMode.RGB => { /* 处理 RGB */ }
    case ColorMode.HSL(h, s, l) => { /* 解构关联值 */ }
}
```

### 6.7 class 面向对象

```
class Animal {
    name: str = ""
    age: int = 0
    fn speak(self) {
        print("...")
    }
}

class Dog : Animal {            // 继承
    breed: str = "unknown"
    fn speak(self) {            // 方法重写
        print("Woof")
    }
}

let d = Dog(name: "Rex", age: 3, breed: "Labrador")
d.speak()                       // 方法调用 → "Woof"
let n = d.name                  // 字段访问 → "Rex"
```

### 6.8 命名空间导入

```
// 选择性导入
from "lib.vgl" import sin, cos, pi

// 通配符导入（导入全部）
from "lib.vgl" import *

// 模块定义
module Math {
    fn clamp(x, lo, hi) {
        if x < lo { return lo }
        if x > hi { return hi }
        return x
    }
}
import Math                     // 整体导入
let v = Math.clamp(5, 0, 3)    // → 3
```

---

## 七、标准库函数

### 7.1 数学

```
rand(a, b)        → [a, b) 随机浮点数（要求 a < b）
int(x)            → 截断取整
abs(x)            → 绝对值
floor(x)          → 向下取整（返回 float）
ceil(x)           → 向上取整（返回 float）
round(x)          → 四舍五入
sign(x)           → 符号（-1, 0, 1）
min(a, b)         → 较小值
max(a, b)         → 较大值
clamp(x, lo, hi)  → 约束到 [lo, hi]
lerp(a, b, t)     → a + (b-a)*t 线性插值
smoothstep(e0, e1, x) → Hermite 平滑插值

sin(x) cos(x) tan(x)       → 三角函数（弧度）
asin(x) acos(x) atan(x)    → 反三角函数
atan2(y, x)                → atan(y/x) 带象限

log(x)  log2(x)  log10(x)  → 对数
exp(x)  pow(a, b)  sqrt(x) → 指数/幂/开方

radians(deg)  degrees(rad) → 角度转换
pi()  e()                  → 数学常量
```

### 7.2 几何

```
line(p1, p2)               → path（线段）
circle(cx, cy, r)          → path（圆轮廓）
bezier(p1, p2, p3, p4)    → path（三次贝塞尔）
qbezier(p1, p2, p3)       → path（二次贝塞尔）
path(points)               → path（折线）
rect(x, y, w, h)           → path（矩形）
ellipse(cx, cy, rx, ry)    → path（椭圆）
arc(cx, cy, r, start, end) → path（弧线）
polygon(points)            → path（多边形）
triangle(p1, p2, p3)       → path（三角形）

dot(a, b)                  → 点积
length(p1, p2)             → 欧氏距离
```

### 7.3 噪声

```
perlin(x, y)               → [-1, 1] Perlin 梯度噪声
worley(x, y)               → [0, ~32] 细胞噪声距离
fbm(x, y, octaves)         → [-1, 1] 分形布朗运动
```

### 7.4 颜色

```
color(r, g, b [, a])       → 构造颜色 (r,g,b,a)；alpha 默认 255（v1.0）
rgb_to_hsl(r, g, b)        → (h, s, l) 元组
hsl_to_rgb(h, s, l)        → (r, g, b) 元组
lerp_color(c1, c2, t)      → 颜色插值
brighten(color, amt)       → 提亮（amt: 0-255）
darken(color, amt)         → 变暗
saturate(color, amt)       → 饱和度调整
```

### 7.5 后处理（直接作用于画布）

```
grain(intensity)           → 胶片颗粒（intensity: 0-255）
vignette(strength, radius) → 四角暗角
blur(radius)               → 盒模糊
sharpen(amount)            → 拉普拉斯锐化
```

### 7.6 字符串

```
str(x)                     → 任意值转字符串
concat(s1, s2, ...)        → 字符串拼接
substr(s, start, len)      → 子串
upper(s)  lower(s)         → 大小写转换
find(s, sub)               → 返回索引或 -1
```

### 7.7 图像/图层

```
load(path)                 → image 对象
pixel_at(img, x, y)        → (r, g, b) 读取像素
width()                    → 画布宽度（像素）（v1.0）
height()                   → 画布高度（像素）（v1.0）
compose(name, blend)       → 图层合成（副作用）
fill(name)                 → 颜色场填充（副作用）
```

### 7.8 混合模式

| 模式 | 说明 |
|------|------|
| `"over"` | 近似 Alpha 混合（默认） |
| `"add"` | 加法混合，亮色叠加 |
| `"mul"` | 乘法混合，暗色叠加 |
| `"screen"` | 滤色，提亮 |
| `"overlay"` | 叠加，增强对比 |
| `"soft_light"` | 柔光 |
| `"hard_light"` | 强光 |
| `"color_dodge"` | 颜色减淡 |
| `"color_burn"` | 颜色加深 |
| `"linear_burn"` | 线性加深 |
| `"difference"` | 差值 |
| `"exclusion"` | 排除 |

---

## 八、绘图原语与填充

### 8.1 stroke 绘制

```
stroke {
    path: line((0, 0), (100, 100))    // 必填：路径
    width: 2                           // 线宽（像素）
    color: #ff0000                     // 颜色（被 material 覆盖时忽略）
    material: gold                     // 可选：材质引用
    samples: 64                        // 可选：贝塞尔采样数
    join: "miter"                      // 可选：折线连接方式
}
```

**join 选项**：`"round"`（默认，圆头）、`"miter"`（尖角）、`"bevel"`（切角）

### 8.2 pixel 绘制

```
pixel(x: 10, y: 20, rgb: (255, 0, 0))   // 参数必须使用关键字形式
pixel(x: x, y: y, rgb: #ff0000)
pixel(x: x, y: y, rgb: my_color)
```

### 8.3 填充函数

```
fill_rect(x, y, w, h, (r, g, b))         // 矩形填充
fill_circle(cx, cy, radius, (r, g, b))   // 圆形填充
fill_ellipse(cx, cy, rx, ry, (r, g, b)) // 椭圆填充
fill_polygon(points, (r, g, b))          // 多边形填充（子像素精度）
flood_fill(x, y, (r, g, b))             // 泛洪填充（同色区域替换）
```

### 8.4 渐变填充

```
fill_linear_gradient(x1, y1, x2, y2, c1, c2)
    // 线性渐变：沿 (x1,y1)→(x2,y2) 方向，从 c1 过渡到 c2

fill_radial_gradient(cx, cy, r, c1, c2)
    // 径向渐变：以 (cx,cy) 为中心，半径 r，从中心 c1 到边缘 c2
```

### 8.5 文本绘制

```
text(x, y, str, size, color)
    // 5×7 点阵字体，size 为缩放倍数
    // 支持 A-Z a-z 0-9 和常见标点（88 个 ASCII 字符）
```

### 8.6 2D 变换与裁剪

```
translate(tx, ty)           // 平移当前坐标系
rotate(rad)                 // 旋转（弧度）
scale(sx, sy)               // 缩放
push_transform()            // 保存当前变换矩阵到栈
pop_transform()             // 恢复上一个变换矩阵

clip_rect(x, y, w, h)      // 裁剪矩形（与栈顶求交集）
clip_clear()                // 清除裁剪栈
```

---

## 九、材质/图层/颜色场

### 9.1 材质

```
material gold {
    color: (255, 200, 50)    // 基色
    noise: 0.3               // 亮度扰动强度（0=无，1=全范围）
    alpha: 0.8               // 不透明度 [0, 1]
}
// 材质库预设
material water { color: preset("watercolor") }
material oil  { color: preset("oil_painting") }
```

**预设材质**：`"watercolor"` `"oil_painting"` `"neon"` `"pencil"` `"crayon"`

### 9.2 图层

```
layer ringlayer {
    for t in 0..360 {
        let a = t * pi() / 180
        pixel(x: 100 + int(60 * cos(a)),
              y: 100 + int(60 * sin(a)),
              rgb: #ff4040)
    }
}
compose("ringlayer", "add")     // 合成到主画布
```

### 9.3 颜色场

```
field gradient(x, y) {
    let t = x / 200.0
    return (int(255 * t), 0, int(255 * (1 - t)))
}
fill("gradient")                // 遍历每个像素调用 field
```

---

## 十、高级特性

### 10.1 sRGB 线性工作流

颜色合成在线性空间进行，避免颜色变暗/变脏：

```
// 输入颜色（sRGB）→ 线性空间 → 混合 → 输出（sRGB）
// 自动完成，无需手动处理
```

### 10.2 图像复刻工具链

```
vgl replicate --mode pixel <input.png> <output.vgl>
vgl replicate --mode block <input.png> <output.vgl> [--block-size 16]
vgl replicate --mode progressive <input.png> <output.vgl> [--layers 32,8,1] [--threshold 30]
```

### 10.3 CLI 选项

```
vgl [--continue-on-error] <file.vgl>    // 错误后继续执行
```

### 10.4 类型转换（as）

```
x as int      // 截断取整
x as float    // 保持 f64
x as bool     // 非 0 为 true
x as str      // 转字符串
s as color    // 解析 "#RRGGBB" 字符串为颜色
```

---

## 十一、编码建议与最佳实践

### 11.1 让 AI 生成高质量 VGL 代码的原则

1. **充分利用元组广播**：用 `(p1 + p2) / 2` 代替 `((p1[0]+p2[0])/2, (p1[1]+p2[1])/2)`，代码量减少 5-10 倍
2. **函数封装复杂逻辑**：用函数抽象重复绘制模式，参数化位置/颜色/尺寸
3. **field + fill 实现连续渐变**：比硬编码像素级 for 循环效率高
4. **layer 分层合成**：不同的视觉元素放到不同图层，用 compose 组合
5. **利用随机 seed 控制可复现性**：调试时固定 seed，正式运行去掉
6. **stroke + path 替代大量 pixel**：复杂几何图案用路径而非逐像素 fill
7. **合理设置 canvas 尺寸**：太大的画布渲染慢（像素级运算 O(W×H)）
8. **利用 perlin/worley 噪声生成有机纹理**：比数学公式更自然
9. **使用完整函数名**：例如用 `sin(a)`，函数名是统一的
10. **颜色尽量用 #RRGGBB 字面量**：比三元组写法更直观

### 11.2 sRGB 颜色使用

所有颜色字面量（`#ff0000`）和三元组（`(255,0,0)`）都视为 sRGB 空间的值。合成引擎会自动做 sRGB→线性→混合→sRGB 转换。

### 11.3 常见模式模板

```
// 基本纹理生成
fn make_texture(cx, cy, seed_val) {
    seed seed_val
    for y in 0..cy {
        for x in 0..cx {
            let n = perlin(x * 0.02, y * 0.02)
            let v = int((n + 1) * 127.5)
            pixel(x: x, y: y, rgb: (v, v, v))
        }
    }
}

// 径向渐变场
field radial(x, y) {
    let dx = x - 200
    let dy = y - 200
    let d = sqrt(dx * dx + dy * dy)
    let t = (d / 200.0).min(1.0)
    return lerp_color(#ff6600, #003366, t)
}

// 分形噪声叠加
fn fbm_noise(x, y) {
    return fbm(x * 0.01, y * 0.01, 4)
}
```

---

## 十二、完整示例

### 示例 1：RGB 渐变

```
canvas 256x256
bg #000000
for y in 0..256 {
    for x in 0..256 {
        pixel(x: x, y: y, rgb: (x, y, 128))
    }
}
render "gradient.png"
```

### 示例 2：五角星

```
canvas 400x300
bg #112233
fn star(cx, cy, r) {
    for i in 0..5 {
        let a = i * 1.25663706
        let x = int(cx + r * cos(a))
        let y = int(cy + r * sin(a))
        stroke {
            path: line((int(cx), int(cy)), (x, y))
            width: 1
            color: #ffff00
        }
    }
}
star(200, 150, 50)
render "star.png"
```

### 示例 3：随机草地（高密度绘制）

```
canvas 800x600
bg #1a1a2e
fn grass_blade(x, y, h) {
    let dx = int(rand(-2, 3))
    let dy = int(rand(3, 15))
    stroke {
        path: line((x, y), (x + dx, y - dy))
        width: 1
        color: (int(rand(30, 80)), int(rand(120, 200)), int(rand(30, 60)))
    }
}
for i in 0..50000 {
    let x = int(rand(0, 800))
    let y = int(rand(300, 600))
    grass_blade(x, y, 10)
}
render "grass_field.png"
```

### 示例 4：颜色场 + 噪声

```
canvas 400x300
bg #000000
field cloud(x, y) {
    let n = perlin(x * 0.02, y * 0.02)
    let v = int((n + 1) * 127.5)
    return (v, v, int(v * 1.5))
}
fill("cloud")
render "cloud.png"
```

### 示例 5：变换 + 裁剪 + 文本

```
canvas 640x480
bg #1a1a2e

// 平移坐标系到中心
translate(320, 240)
// 旋转并绘制矩形
rotate(pi() / 4)
fill_rect(-50, -50, 100, 100, (200, 80, 80))
pop_transform()

// 裁剪视窗
clip_rect(100, 100, 200, 150)
fill_circle(200, 175, 60, (80, 160, 200))
clip_clear()

// 文本
text(20, 20, "Hello VGL", 3, (255, 255, 255))
render "transform_demo.png"
```

### 示例 6：match + enum + class（v0.9 特性）

```
canvas 400x200
bg #ffffff

enum Shape {
    Circle,
    Square
}

class Drawable {
    x: int = 0
    y: int = 0
    color: color = #000000
    shape: str = "circle"

    fn draw(self) {
        match self.shape {
            case "circle" => {
                fill_circle(self.x, self.y, 20, self.color)
            }
            case "square" => {
                fill_rect(self.x - 20, self.y - 20, 40, 40, self.color)
            }
        }
    }
}

let red_circle = Drawable(x: 100, y: 100, color: #ff0000, shape: "circle")
let blue_square = Drawable(x: 300, y: 100, color: #0000ff, shape: "square")
red_circle.draw()
blue_square.draw()

render "oop_demo.png"
```

---

## 附录：保留字全表

```
canvas  bg  let  const  var  for  in  if  else  fn  return
pixel  stroke  render  while  break  continue  and  or  not
seed  true  false  null  struct  import  material  layer  field
as  match  case  default  enum  class  from  module  step
```

全部关键字均为小写，v1.0 起所有保留字均已实现。
