# VGL v2.0 — Quick Reference (for AI)

VGL is a procedural language that **outputs SVG**. A script is a living generation blueprint: variables, functions, loops and deterministic noise describe *how* to draw; the interpreter emits a clean SVG scene graph (shapes, gradients, filters, transform groups).

- CLI: `vgl <file.vgl>` → writes SVG file(s) relative to the script's directory.
- Single Rust binary, zero dependencies.

## Program skeleton

```vgl
use "../lib/sky.vgl"        // include library (path relative to current file)

canvas 1000x700             // width x height (the 'x' is a separator)
seed 42                     // deterministic PRNG + Perlin permutation

// ... draw statements ...

render "out.svg"            // serialize scene graph to SVG (path relative to main script dir)
```

## Types & literals

- number `42  3.14  -0.5` · string `"#ff6b6b"` · bool `true false`
- `none` — null-like literal; as paint means "no fill/stroke"
- color — `color(r, g, b)` or `color(r, g, b, a)` (0-255, alpha 0-1); CSS strings accepted where a paint is expected
- array — `[1, 2, 3]`, index `a[i]` (0-based, floats truncate), `len(a)`, `push(a, v)`
- gradient — first-class value from `linear_gradient` / `radial_gradient`, usable as any fill/stroke
- `TAU` constant

## Statements

```vgl
let x = expr                        // define variable (no bare assignment; re-let to shadow)
fn f(a, b = 1) { ... }              // function with default params; return via `return expr`
if cond { } else if cond { } else { }
for i in 0..10 { }                  // exclusive end; float bounds ok
for x in 0..800 step 40 { }
while cond { }
break / continue
group(translate: [x, y], rotate: deg, scale: [sx, sy], opacity: o, blur: r) { ... }  // nested transform group
use "path/file.vgl"                 // include-once, executes in current env
```

Comments: `// line` and `/* block */`.

## Operators

`+ - * / %` · comparisons `== != < <= > >=` · `and or not` · index `a[i]`

## Shapes (common named args: `fill: stroke: stroke_width:/width: opacity: blur: cap: join:`)

```vgl
rect(x, y, w, h, rx:)
circle(cx, cy, r)
ellipse(cx, cy, rx, ry)
line(x1, y1, x2, y2)                       // stroke-only; cap: "round"|"butt"|"square"
polygon([x1, y1, x2, y2, ...])             // auto-closed, filled by default
polyline([x1, y1, ...])                    // open, stroked
path(d_string)                             // raw SVG path data
text(x, y, "str", size:, anchor:, weight:, font:)
background(paint)                          // full-canvas rect
```

## Gradients & path utility

```vgl
linear_gradient([c1, c2, c3], x1:, y1:, x2:, y2:)   // default: top → bottom of canvas
radial_gradient([c1, c2], cx:, cy:, r:)             // default: canvas center
smooth([x1, y1, x2, y2, ...], closed: false)        // → Catmull-Rom smooth path d-string
```

## Math / noise / color builtins

```vgl
sin cos tan atan2 abs floor ceil round sqrt exp log pow min max clamp lerp
rand(a, b)  rand_int(a, b)  perlin(x, y)  fbm(x, y, octaves)
lighten(c, t)  darken(c, t)  lerp_color(c1, c2, t)  alpha(c, a)  red(c) green(c) blue(c)
width()  height()  len(a)  push(a, v)  print(...)
```

Determinism: after `seed N`, every `rand`/`perlin`/`fbm` sequence is reproducible.

## Semantic library `lib/` (written in VGL itself)

| Module | Functions |
|--------|-----------|
| palette | `palette("dawn"\|"noon"\|"dusk"\|"night"\|"forest"\|"ocean"\|"pastel"\|"mono")` → 4 colors; `palette_sky(name)` → gradient; `palette_pick(cols, i)` |
| sky | `sky(top, bottom)` `sun(x,y,r,col)` `moon(x,y,r,col)` `stars(count, y_ratio, max_b)` `meteor(x,y,len,angle,col)` `clouds(count, col, opacity)` `cloud_band(y, h, col, opacity)` `aurora(bands, col1, col2, intensity)` |
| terrain | `ridge(y_base, amp, freq, col, off, octaves)` `mountains(layers, y_base, far, near, amp)` `snow_cap(y_base, amp, freq, col, off, depth)` `dunes(y_base, amp, col, off)` |
| water | `water(y, col_top, col_deep)` `waves(y, rows, span, amp, col)` `shimmer(x, y, w, col)` |
| vegetation | `tree(x, ground, s, kind, leaf, trunk)` `broadleaf(...)` `conifer(...)` `forest(count, y_min, y_max, kind, leaf)` (kind: 0 broadleaf, 1 conifer, 2 mixed) `grass(y, count, col, len)` |
| atmosphere | `fog(y, h_band, col, density)` `ground_fog(y, col, density)` `rain(count, slant, col)` `snow(count, col)` `light_rays(x, y, n, len, col, spread, intensity)` `vignette(strength, col)` `tint(col, strength)` |

## Minimal scene

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

Change `seed` for a new composition; change palette for a new time of day; every parameter is editable — that is the point of VGL.
