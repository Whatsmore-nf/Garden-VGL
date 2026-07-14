# VGL (Visual Graphics Language) Quick Reference

> **Complete Language Definition for AI** — A minimal domain-specific language for procedural image generation
> Version v0.9 · 2026-07-14

---

## Table of Contents

- [1. Language Overview](#1-language-overview)
- [2. Lexical Specification](#2-lexical-specification)
- [3. Syntax (EBNF-like)](#3-syntax-ebnf-like)
- [4. Type System](#4-type-system)
- [5. Operator Precedence & Expressions](#5-operator-precedence--expressions)
- [6. Statements in Detail](#6-statements-in-detail)
- [7. Standard Library](#7-standard-library)
- [8. Drawing Primitives & Fill](#8-drawing-primitives--fill)
- [9. Material / Layer / Field](#9-material--layer--field)
- [10. Advanced Features](#10-advanced-features)
- [11. Coding Tips & Best Practices](#11-coding-tips--best-practices)
- [12. Complete Examples](#12-complete-examples)
- [Appendix: Reserved Words](#appendix-reserved-words)

---

## 1. Language Overview

VGL is a domain-specific scripting language for procedural image generation. A VGL program's core structure:

```
set canvas → declare background → draw content → render output
```

**In one sentence**: Write a `.vgl` script, run `vgl script.vgl`, get a PNG image.

### Design Constraints

1. Plain text source, UTF-8 encoding, extension `.vgl`
2. Dynamic typing, runtime type tags
3. Sequential execution, canvas as global state
4. Positional and keyword arguments cannot be mixed

### Program Skeleton

```
canvas 800x600          // REQUIRED: set canvas width×height (pixels)
bg #1a1a2e              // OPTIONAL: background color (recommended)

// ... drawing statements ...

render "output.png"     // REQUIRED: output as PNG file
```

---

## 2. Lexical Specification

### 2.1 Identifiers

```
identifier = letter | underscore, followed by letters/digits/underscores
regex: [a-zA-Z_][a-zA-Z0-9_]*
case-sensitive, suggested ≤64 chars
examples: x, my_var, _temp, strokeCount
```

### 2.2 Literals

| Type | Syntax | Examples |
|------|--------|----------|
| Integer | plain digits | `0`, `123`, `42` |
| Float | decimal point | `3.14`, `0.5`, `.75` |
| String | double-quoted | `"hello.png"`, `"path/output.jpg"` |
| Color 3-digit | `#RGB` | `#f00` = (255,0,0), `#ffd` |
| Color 6-digit | `#RRGGBB` | `#ff0000`, `#ffd966` |
| Color 4-digit | `#RGBA` | `#f00f` = (255,0,0,255) |
| Color 8-digit | `#RRGGBBAA` | `#ff0000ff`, `#ff000080` (semi-transparent) |
| bool | keyword | `true`, `false` |
| null | keyword | `null` |

String escape sequences: `\n` (newline), `\t` (tab), `\r` (carriage return), `\\` (backslash), `\"` (double quote), `\0` (null). Unknown escapes are left as-is.

### 2.3 Comments

```
// single-line comment to end of line
/* block comment, no nesting */
```

### 2.4 Operators & Delimiters

**Single char**: `+` `-` `*` `/` `%` `=` `<` `>` `(` `)` `{` `}` `[` `]` `,` `:` `.`

**Double char**: `..` (range) `==` `!=` `<=` `>=` `+=` `-=` `*=` `/=` `%=` `<<` `>>` `=>` (match arrow)

**Keyword form**: `and` `or` `not`

**Bitwise**: `&` `|` `^` `~` `<<` `>>` (on integers, f64→i64→op→f64)

**Increment/Decrement**: `x++` → `x = x + 1` (statement sugar), `x--` → `x = x - 1`

---

## 3. Syntax (EBNF-like)

### 3.1 Program Structure

```
program       = canvasDecl, bgDecl?, { statement }, renderDecl
canvasDecl    = 'canvas', integer, 'x', integer
bgDecl        = 'bg', colorLiteral
renderDecl    = 'render', string
```

### 3.2 Statement Overview

```
statement =
    varDecl       | constDecl  | assignStmt    | compoundAssign
  | ifStmt        | forLoop    | whileLoop     | forInLoop
  | breakStmt     | continue   | fnDecl        | returnStmt
  | matchStmt     | pixelStmt  | strokeStmt    | seedStmt
  | structDef     | enumDef    | classDef      | materialDef
  | layerDef      | fieldDef   | importStmt    | fromImport
  | moduleDef     | indexAssign| fieldAssign   | exprStmt
```

### 3.3 Full Statement Definitions

```
<varDecl>       ::= 'let' <ident> '=' <expr>
<constDecl>     ::= 'const' <ident> '=' <expr>            // immutable
<varDeclAlt>    ::= 'var' <ident> '=' <expr>               // let alias
<assignStmt>    ::= <ident> '=' <expr>
<compoundAssign>::= <ident> ('+='|'-='|'*='|'/='|'%=') <expr>
<incDec>        ::= <ident> '++' | <ident> '--'            // statement level

<ifStmt>        ::= 'if' <expr> '{' { <stmt> } '}' [ 'else' ( '{'{<stmt>'}' | <ifStmt> ) ]
<forLoop>       ::= 'for' <ident> 'in' <expr> '..' <expr> '{' { <stmt> } '}'
<whileLoop>     ::= 'while' <expr> '{' { <stmt> } '}'
<forInLoop>     ::= 'for' <ident> 'in' <expr> '{' { <stmt> } '}'  // array/tuple

<breakStmt>     ::= 'break' [ <ident> ]                   // optional label
<continueStmt>  ::= 'continue'

<fnDecl>        ::= 'fn' <ident> '(' [ <params> ] ')' '{' { <stmt> } '}'
<returnStmt>    ::= 'return' <expr>

<matchStmt>     ::= 'match' <expr> '{' { <case> } [ 'default' '=>' '{' <stmts> '}' ] '}'
<case>          ::= 'case' <pattern> '=>' '{' { <stmt> } '}'
<pattern>       ::= <literal> | <ident> | '_' | '(' <pattern> { ',' <pattern> } ')'

<pixelStmt>     ::= 'pixel' '(' 'x' ':' <expr> ',' 'y' ':' <expr> ',' 'rgb' ':' <expr> ')'
<strokeStmt>    ::= 'stroke' '{' { <field> ':' <expr> } '}'
<seedStmt>      ::= 'seed' <integer>

<structDef>     ::= 'struct' <ident> '{' { <fieldName> ':' <expr> [','] } '}'
<enumDef>       ::= 'enum' <ident> '{' { <ident> [ '(' [ <params> ] ')' ] [','] } '}'
<classDef>      ::= 'class' <ident> [ ':' <ident> ] '{' { <fieldDecl> | <methodDef> } '}'
<fieldDecl>     ::= <ident> ':' <type> '=' <expr>
<methodDef>     ::= 'fn' <ident> '(' 'self' [',' <params>] ')' '{' { <stmt> } '}'

<materialDef>   ::= 'material' <ident> '{' { <keywordArg> } '}'
<layerDef>      ::= 'layer' <ident> '{' { <stmt> } '}'
<fieldDef>      ::= 'field' <ident> '(' [ <params> ] ')' '{' { <stmt> } '}'

<importStmt>    ::= 'import' <string>
<fromImport>    ::= 'from' <string> 'import' ( <ident> { ',' <ident> } | '*' )
<moduleDef>     ::= 'module' <ident> '{' { <stmt> } '}'

<indexAssign>   ::= <ident> '[' <expr> ']' '=' <expr>
<fieldAssign>   ::= <ident> '.' <ident> '=' <expr>
<exprStmt>      ::= <expr>
```

---

## 4. Type System

### 4.1 Runtime Types

| Type | Representation | Usage |
|------|---------------|-------|
| `number` | f64 | Numeric values (int/float unified) |
| `bool` | true/false | Conditionals |
| `string` | UTF-8 string | File paths, text |
| `color` | (r,g,b) or (r,g,b,a) | Colors, each 0-255 |
| `tuple` | Ordered heterogeneous list | Coordinates, colors, return values |
| `array` | Mutable ordered container | Dynamic collections |
| `dict` | Key-value mapping | Data structures |
| `struct` | Custom field collection | State objects |
| `enum` | Tagged variants | Enumerations |
| `class` | Class instance | OOP |
| `path` | Opaque drawing object | stroke path field |
| `material` | Material object | stroke material properties |
| `layer` | Off-screen buffer | Layer compositing |
| `image` | Loaded image | load() return value |
| `closure` | Function + captured env | Functions/closures |
| `module` | Namespace | Module imports |
| `none` | null | No return value |

### 4.2 Implicit Type Coercion

- **number ↔ bool**: non-zero → true, 0 → false; true→1, false→0
- **color ↔ tuple(3)**: runtime isomorphic, `(255,0,0)` equals red
- **color ↔ tuple(4)**: `(255,0,0,255)` equals red with alpha
- No automatic string conversion, no implicit cross-type arithmetic

### 4.3 Tuple Broadcasting

Key design for simplifying geometry calculations:

```
(1,2) + (3,4) → (4,6)       // element-wise, same length required
(10,20) * 2   → (20,40)      // scalar broadcast
2 * (10,20)   → (20,40)      // commutative
(20,40) / 2   → (10,20)      // scalar division
(10,20) + 5   → TypeError    // illegal: ambiguous
(10,20) * (3,4) → TypeError  // illegal: use dot() for dot product
```

---

## 5. Operator Precedence & Expressions

### 5.1 Precedence Table (high→low)

| Precedence | Operators | Associativity | Description |
|-----------|-----------|---------------|-------------|
| 10 | `.` `[ ]` `( )` | left | Field access, index, call |
| 9 | `~` `-`(unary) `not` | right | Bitwise NOT, negation, logical NOT |
| 8 | `*` `/` `%` | left | Multiplication level |
| 7 | `<<` `>>` | left | Shift |
| 6 | `+` `-` | left | Addition level |
| 5 | `&` | left | Bitwise AND |
| 4 | `^` | left | Bitwise XOR |
| 3 | `|` | left | Bitwise OR |
| 2 | `<` `>` `<=` `>=` `==` `!=` | none | Comparison |
| 1 | `and` | left | Logical AND (short-circuit) |
| 0 | `or` | left | Logical OR (short-circuit) |

### 5.2 Expression Nodes

```
<expr> =
    <number> | <string> | <color> | <bool> | <null> | <ident>
  | '(' <expr> ')'                                    // grouping
  | '(' <expr> ',' { <expr> } ')'                     // tuple
  | '[' [ <expr> { ',' <expr> } ] ']'                 // array
  | <expr> <binop> <expr>                             // binary op
  | ('-' | 'not' | '~') <expr>                        // unary op
  | <expr> '[' <expr> ']'                             // index
  | <expr> '.' <ident>                                 // field access
  | <expr> '(' [ <args> ] ')'                         // function call
  | <expr> 'as' <typeName>                             // type cast
```

**Type names**: `int` `float` `bool` `str` `color`

**Cast examples**:
```
3.14 as int      → 3        // truncation
255 as float     → 255.0
0 as bool        → false
42 as str        → "42"
"#ff0000" as color → (255, 0, 0)
```

---

## 6. Statements in Detail

### 6.1 Variables / Constants / Assignment

```
let count = 0            // mutable declaration
const PI = 3.14159       // immutable constant, reassignment errors
var total = 100          // explicitly mutable (let alias)

count = 5                // assignment (looks up scope chain)
count += 1               // compound: count = count + 1
total -= 10              // total = total - 10
total++                  // increment: total = total + 1
total--                  // decrement: total = total - 1
```

### 6.2 Loops

```
// Range for-loop (recommended for coordinate traversal)
for x in 0..256 {                  // [0, 256), step 1
    pixel(x: x, y: y, rgb: ...)
}

// Labeled for (for breaking outer loop)
outer: for i in 0..10 {
    for j in 0..10 {
        if i == 3 and j == 3 { break outer }
    }
}

// While loop (for uncertain iterations)
let x = 0
while x < 400 and x > 0 {
    x = x + int(rand(-2, 3))
}

// For-in array/tuple traversal
let colors = [#ff0000, #00ff00, #0000ff]
for c in colors {
    // iterate each color
}
```

### 6.3 Conditionals

```
// if-else
if x - 100 {
    // true when x ≠ 100
} else {
    // executes when x == 100
}

// else-if chain
if x > 200 {
    // large
} else if x > 100 {
    // medium
} else {
    // small
}

// match/case pattern matching (v0.9)
match color {
    case #ff0000 => {
        stroke { ... }
    }
    case #00ff00 => {
        stroke { ... }
    }
    default => {
        // unmatched default branch
    }
}
```

**Pattern support**:
- Literal match: `case 42 =>`, `case "hello" =>`, `case #ff0000 =>`
- Variable binding: `case x =>` (matches any value, binds to x)
- Wildcard: `case _ =>` (matches any value, no binding)
- Tuple destructuring: `case (a, b) =>`, `case (x, y, z) =>`
- Enum matching: `case Color.Red =>`, `case Color.RGB(r, g, b) =>`

### 6.4 Functions

```
fn add(a, b) {
    return a + b
}

// Closures (capture outer variables)
fn make_counter() {
    let count = 0
    fn inc() {
        count = count + 1       // mutable capture
        return count
    }
    return inc
}
let c = make_counter()
c()  // → 1
c()  // → 2

// Recursion
fn factorial(n) {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}
```

### 6.5 Array / Dict / Struct

```
// Array
let arr = [1, 2, 3]
arr[0] = 100           // index assignment
push(arr, 99)          // append
let v = pop(arr)       // pop
len(arr)               // length

// Dict
let d = dict("name", "Alice", "age", 30)
d["extra"] = 100       // create new key
has(d, "name")         // → true
keys(d)                // → ["name", "age", "extra"]

// Struct
struct Point { x: 0, y: 0 }
let p = Point(x: 10, y: 20)    // keyword args
let q = Point(30, 40)           // positional args
p.x = 15                        // field assignment
let vx = p.x                    // field access
```

### 6.6 Enum

```
enum ColorMode {
    RGB,                    // no associated value
    HSL = (h, s, l)        // with associated values
}

let mode = ColorMode.RGB
let hsl = ColorMode.HSL(0.5, 0.8, 0.3)

match mode {
    case ColorMode.RGB => { /* handle RGB */ }
    case ColorMode.HSL(h, s, l) => { /* destructure associated values */ }
}
```

### 6.7 Class (OOP)

```
class Animal {
    name: str = ""
    age: int = 0
    fn speak(self) {
        print("...")
    }
}

class Dog : Animal {            // inheritance
    breed: str = "unknown"
    fn speak(self) {            // method override
        print("Woof")
    }
}

let d = Dog(name: "Rex", age: 3, breed: "Labrador")
d.speak()                       // method call → "Woof"
let n = d.name                  // field access → "Rex"
```

### 6.8 Namespace Import

```
// Selective import
from "lib.vgl" import sin, cos, pi

// Wildcard import (import all)
from "lib.vgl" import *

// Module definition
module Math {
    fn clamp(x, lo, hi) {
        if x < lo { return lo }
        if x > hi { return hi }
        return x
    }
}
import Math                     // full import
let v = Math.clamp(5, 0, 3)    // → 3
```

---

## 7. Standard Library

### 7.1 Math

```
rand(a, b)        → [a, b) random float (requires a < b)
int(x)            → truncate to integer
abs(x)            → absolute value
floor(x)          → floor (returns float)
ceil(x)           → ceiling (returns float)
round(x)          → round to nearest
sign(x)           → sign (-1, 0, 1)
min(a, b)         → smaller value
max(a, b)         → larger value
clamp(x, lo, hi)  → constrain to [lo, hi]
lerp(a, b, t)     → a + (b-a)*t linear interpolation
smoothstep(e0, e1, x) → Hermite smooth interpolation

sin(x) cos(x) tan(x)       → trigonometric (radians)
asin(x) acos(x) atan(x)    → inverse trig
atan2(y, x)                → atan(y/x) with quadrant

log(x)  log2(x)  log10(x)  → logarithms
exp(x)  pow(a, b)  sqrt(x) → exponential/power/square root

radians(deg)  degrees(rad) → angle conversion
pi()  e()                  → math constants
```

### 7.2 Geometry

```
line(p1, p2)               → path (segment)
circle(cx, cy, r)          → path (circle outline)
bezier(p1, p2, p3, p4)    → path (cubic bezier)
qbezier(p1, p2, p3)       → path (quadratic bezier)
path(points)               → path (polyline)
rect(x, y, w, h)           → path (rectangle)
ellipse(cx, cy, rx, ry)    → path (ellipse)
arc(cx, cy, r, start, end) → path (arc)
polygon(points)            → path (polygon)
triangle(p1, p2, p3)       → path (triangle)

dot(a, b)                  → dot product
length(p1, p2)             → Euclidean distance
```

### 7.3 Noise

```
perlin(x, y)               → [-1, 1] Perlin gradient noise
worley(x, y)               → [0, ~32] cellular noise distance
fbm(x, y, octaves)         → [-1, 1] fractal Brownian motion
```

### 7.4 Color

```
rgb_to_hsl(r, g, b)        → (h, s, l) tuple
hsl_to_rgb(h, s, l)        → (r, g, b) tuple
lerp_color(c1, c2, t)      → color interpolation
brighten(color, amt)       → brighten (amt: 0-255)
darken(color, amt)         → darken
saturate(color, amt)       → saturation adjustment
```

### 7.5 Post-processing (direct canvas effect)

```
grain(intensity)           → film grain (intensity: 0-255)
vignette(strength, radius) → corner darkening
blur(radius)               → box blur
sharpen(amount)            → Laplacian sharpen
```

### 7.6 String

```
str(x)                     → any value to string
concat(s1, s2, ...)        → string concatenation
substr(s, start, len)      → substring
upper(s)  lower(s)         → case conversion
find(s, sub)               → returns index or -1
```

### 7.7 Image / Layer

```
load(path)                 → image object
pixel_at(img, x, y)        → (r, g, b) read pixel
compose(name, blend)       → layer compositing (side effect)
fill(name)                 → color field fill (side effect)
```

### 7.8 Blend Modes

| Mode | Description |
|------|-------------|
| `"over"` | Approximate Alpha blending (default) |
| `"add"` | Additive, brighten |
| `"mul"` | Multiplicative, darken |
| `"screen"` | Screen, brighten |
| `"overlay"` | Overlay, contrast enhancement |
| `"soft_light"` | Soft light |
| `"hard_light"` | Hard light |
| `"color_dodge"` | Color dodge |
| `"color_burn"` | Color burn |
| `"linear_burn"` | Linear burn |
| `"difference"` | Difference |
| `"exclusion"` | Exclusion |

---

## 8. Drawing Primitives & Fill

### 8.1 Stroke

```
stroke {
    path: line((0, 0), (100, 100))    // REQUIRED: path
    width: 2                           // line width (pixels)
    color: #ff0000                     // color (ignored when material present)
    material: gold                     // OPTIONAL: material reference
    samples: 64                        // OPTIONAL: bezier sample count
    join: "miter"                      // OPTIONAL: polyline join type
}
```

**join options**: `"round"` (default, round cap), `"miter"` (sharp corner), `"bevel"` (chamfer)

### 8.2 Pixel

```
pixel(x: 10, y: 20, rgb: (255, 0, 0))   // keyword arguments required
pixel(x: x, y: y, rgb: #ff0000)
pixel(x: x, y: y, rgb: my_color)
```

### 8.3 Fill Functions

```
fill_rect(x, y, w, h, (r, g, b))         // rectangle fill
fill_circle(cx, cy, radius, (r, g, b))   // circle fill
fill_ellipse(cx, cy, rx, ry, (r, g, b)) // ellipse fill
fill_polygon(points, (r, g, b))          // polygon fill (sub-pixel precision)
flood_fill(x, y, (r, g, b))             // flood fill (replace same color area)
```

### 8.4 Gradient Fill

```
fill_linear_gradient(x1, y1, x2, y2, c1, c2)
    // Linear gradient: along direction (x1,y1)→(x2,y2), from c1 to c2

fill_radial_gradient(cx, cy, r, c1, c2)
    // Radial gradient: center (cx,cy), radius r, from center c1 to edge c2
```

### 8.5 Text

```
text(x, y, str, size, color)
    // 5×7 dot matrix font, size is scale factor
    // Supports A-Z a-z 0-9 and common punctuation (88 ASCII chars)
```

### 8.6 2D Transform & Clipping

```
translate(tx, ty)           // translate coordinate system
rotate(rad)                 // rotate (radians)
scale(sx, sy)               // scale
push_transform()            // save current transform matrix to stack
pop_transform()             // restore previous transform matrix

clip_rect(x, y, w, h)      // clip rectangle (intersect with stack top)
clip_clear()                // clear clip stack
```

---

## 9. Material / Layer / Field

### 9.1 Material

```
material gold {
    color: (255, 200, 50)    // base color
    noise: 0.3               // brightness perturbation (0=none, 1=full range)
    alpha: 0.8               // opacity [0, 1]
}
// Material library presets
material water { color: preset("watercolor") }
material oil  { color: preset("oil_painting") }
```

**Presets**: `"watercolor"` `"oil_painting"` `"neon"` `"pencil"` `"crayon"`

### 9.2 Layer

```
layer ringlayer {
    for t in 0..360 {
        let a = t * pi() / 180
        pixel(x: 100 + int(60 * cos(a)),
              y: 100 + int(60 * sin(a)),
              rgb: #ff4040)
    }
}
compose("ringlayer", "add")     // composite onto main canvas
```

### 9.3 Color Field

```
field gradient(x, y) {
    let t = x / 200.0
    return (int(255 * t), 0, int(255 * (1 - t)))
}
fill("gradient")                // iterate every pixel calling field
```

---

## 10. Advanced Features

### 10.1 sRGB Linear Workflow

Color compositing is performed in linear space to avoid darkening/dirtying:

```
// Input color (sRGB) → linear space → blend → output (sRGB)
// Automatic, no manual handling needed
```

### 10.2 Image Replication Toolchain

```
vgl replicate --mode pixel <input.png> <output.vgl>
vgl replicate --mode block <input.png> <output.vgl> [--block-size 16]
vgl replicate --mode progressive <input.png> <output.vgl> [--layers 32,8,1] [--threshold 30]
```

### 10.3 CLI Options

```
vgl [--continue-on-error] <file.vgl>    // continue after errors
```

### 10.4 Type Cast (as)

```
x as int      // truncate
x as float    // keep f64
x as bool     // non-zero → true
x as str      // to string
s as color    // parse "#RRGGBB" string to color
```

---

## 11. Coding Tips & Best Practices

### 11.1 Principles for High-Quality VGL Code

1. **Use tuple broadcasting**: `(p1 + p2) / 2` instead of `((p1[0]+p2[0])/2, (p1[1]+p2[1])/2)` → 5-10x less code
2. **Encapsulate complex logic in functions**: Abstract repeated drawing patterns with parameterized position/color/size
3. **Use field + fill for continuous gradients**: More efficient than pixel-level for-loops
4. **Layer-based compositing**: Put different visual elements into separate layers, combine with compose
5. **Fix seed for reproducibility**: Use fixed seed during debugging, remove for production
6. **Prefer stroke + path over pixel-by-pixel**: Complex geometry patterns are more efficient as paths
7. **Reasonable canvas size**: Large canvases render slowly (O(W×H) pixel operations)
8. **Use perlin/worley noise for organic textures**: More natural than mathematical formulas
9. **Use full function names**: e.g., `sin(a)` not variations; function names are unified
10. **Prefer #RRGGBB color literals**: More intuitive than tuple notation

### 11.2 sRGB Color Usage

All color literals (`#ff0000`) and triples (`(255,0,0)`) are treated as sRGB values. The compositing engine automatically handles sRGB→linear→blend→sRGB conversion.

### 11.3 Common Pattern Templates

```
// Basic texture generation
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

// Radial gradient field
field radial(x, y) {
    let dx = x - 200
    let dy = y - 200
    let d = sqrt(dx * dx + dy * dy)
    let t = (d / 200.0).min(1.0)
    return lerp_color(#ff6600, #003366, t)
}

// Fractal noise overlay
fn fbm_noise(x, y) {
    return fbm(x * 0.01, y * 0.01, 4)
}
```

---

## 12. Complete Examples

### Example 1: RGB Gradient

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

### Example 2: Star

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

### Example 3: Random Grass Field (High-Density Drawing)

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

### Example 4: Color Field + Noise

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

### Example 5: Transform + Clipping + Text

```
canvas 640x480
bg #1a1a2e

// Translate to center
translate(320, 240)
// Rotate and draw rectangle
rotate(pi() / 4)
fill_rect(-50, -50, 100, 100, (200, 80, 80))
pop_transform()

// Clip viewport
clip_rect(100, 100, 200, 150)
fill_circle(200, 175, 60, (80, 160, 200))
clip_clear()

// Text
text(20, 20, "Hello VGL", 3, (255, 255, 255))
render "transform_demo.png"
```

### Example 6: match + enum + class (v0.9 features)

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

## Appendix: Reserved Words

```
canvas  bg  let  const  var  for  in  if  else  fn  return
pixel  stroke  render  while  break  continue  and  or  not
seed  true  false  null  struct  import  material  layer  field
as  match  case  default  enum  class  from  module
```

All keywords are lowercase. As of v0.9, all reserved words are implemented.
