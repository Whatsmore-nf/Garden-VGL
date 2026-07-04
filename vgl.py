#!/usr/bin/env python3
"""
VGL 最小解释器 — 单文件，仅依赖标准库
支持: canvas / bg / let / = / for / if / fn / return / pixel / stroke / render
      v0.3: while / break / seed / 比较 < > <= >= == != / 逻辑 and or not / bool
            tuple 索引 / tuple 广播 / bezier / qbezier / path / dot / length
            pow / sqrt / 闭包（可变捕获）
      v0.4: 块作用域（for/if/while/stroke 子 Environment）
            continue / 带标签 break（label: for ... break label）
      v0.5: 字符串转义 \n \t \\ \" \r \0
            struct 类型（定义/构造/字段访问/赋值）
            array 数组（字面量/索引/可变/push/pop/len）
            dict 字典（dict() 构造/索引/可变/keys/values/has）
            索引赋值 arr[i] = v / d[k] = v / obj.field = v
            import 模块导入 / 错误定位（filename:line:col）
            stroke 反走样（Wu 细线 + Bresenham+笔刷粗线 + 中点圆）
      v0.5 批次 C: material 材质（定义/stroke material 字段 + noise 扰动）
            layer 图层（离屏缓冲区 + compose 合成 over/add/mul/screen）
            field 颜色场（(x,y)→color 函数 + fill 遍历填充）
            perlin / worley / fbm 噪声函数
      表达式: + - * /  元组  变量  函数调用  颜色字面量 #rgb  true/false  tuple[i]
              array[i]  dict[k]  obj.field
      内建函数: rand(a,b)  int(x)  abs(x)  floor(x)  ceil(x)  sin(x)  cos(x)
                min(a,b)  max(a,b)  bool(x)  pow(a,b)  sqrt(x)
                line(p1,p2)  circle(cx,cy,r)
                bezier(p1,p2,p3,p4)  qbezier(p1,p2,p3)  path(pts)
                dot(a,b)  length(p1,p2)
                len(x)  push(arr,v)  pop(arr)  array(...)  dict(...)
                keys(d)  values(d)  has(d,k)
                perlin(x,y)  worley(x,y)  fbm(x,y,octaves)
                compose(name,blend)  fill(name)
用法: python vgl.py <file.vgl>
"""

import sys
import os
import struct
import zlib
import random
import math


# ============================================================
# PNG 输出（仅用标准库 zlib + struct）
# ============================================================

def write_png(filename, width, height, pixels):
    """pixels: bytearray, 每像素3字节RGB, 行优先"""
    # 自动创建输出目录
    out_dir = os.path.dirname(filename)
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)

    def chunk(ctype, data):
        c = ctype + data
        crc = struct.pack('>I', zlib.crc32(c) & 0xffffffff)
        return struct.pack('>I', len(data)) + c + crc

    sig = b'\x89PNG\r\n\x1a\n'
    ihdr = struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)
    raw = bytearray()
    for y in range(height):
        raw.append(0)  # 无滤镜
        raw.extend(pixels[y * width * 3:(y + 1) * width * 3])
    compressed = zlib.compress(bytes(raw), 9)
    with open(filename, 'wb') as f:
        f.write(sig + chunk(b'IHDR', ihdr) + chunk(b'IDAT', compressed) + chunk(b'IEND', b''))


# ============================================================
# 词法分析
# ============================================================

class Token:
    def __init__(self, t, v, p):
        self.type, self.value, self.pos = t, v, p

    def __repr__(self):
        return f'Token({self.type},{self.value!r})'


KEYWORDS = {'canvas', 'bg', 'let', 'for', 'in', 'if', 'else', 'fn', 'return',
            'pixel', 'stroke', 'render',
            # v0.3 新增
            'while', 'break', 'and', 'or', 'not', 'seed', 'true', 'false',
            # v0.4 新增
            'continue',
            # v0.5 新增
            'struct',
            # v0.5 批次 B 新增
            'import',
            # v0.5 批次 C 新增
            'material', 'layer', 'field'}


# ============================================================
# v0.5 错误定位工具（§8.2 行号/列号/caret）
# ============================================================

def pos_to_linecol(src, pos):
    """字符偏移 pos -> (行号, 列号), 均 1-based。pos 为 None 时返回 (None, None)。"""
    if pos is None or pos < 0:
        return (None, None)
    line = src.count('\n', 0, pos) + 1
    last_nl = src.rfind('\n', 0, pos)
    col = pos - last_nl  # last_nl=-1 时 col=pos+1（行首为 1）
    return (line, col)


def format_error(msg, src, pos, filename):
    """格式化错误信息: filename:line:col: msg\n  <源码行>\n  <caret>"""
    line, col = pos_to_linecol(src, pos)
    if line is None:
        return f'{filename}: {msg}'
    lines = src.split('\n')
    src_line = lines[line - 1] if line - 1 < len(lines) else ''
    caret = ' ' * (col - 1) + '^'
    return f'{filename}:{line}:{col}: {msg}\n  {src_line}\n  {caret}'


def vgl_error(exc_cls, msg, pos):
    """构造带 vgl_pos 属性的异常（词法/语法/运行时通用）。"""
    e = exc_cls(msg)
    e.vgl_pos = pos
    return e


def tokenize(src):
    toks = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        # 空白
        if c in ' \t\r\n':
            i += 1
            continue
        # 行注释 //
        if c == '/' and i + 1 < n and src[i + 1] == '/':
            while i < n and src[i] != '\n':
                i += 1
            continue
        # 块注释 /* */
        if c == '/' and i + 1 < n and src[i + 1] == '*':
            i += 2
            while i < n - 1 and not (src[i] == '*' and src[i + 1] == '/'):
                i += 1
            i += 2
            continue
        # 颜色字面量 #rrggbb 或 #rgb
        if c == '#':
            j = i + 1
            while j < n and src[j] in '0123456789abcdefABCDEF':
                j += 1
            h = src[i + 1:j]
            if len(h) == 6:
                toks.append(Token('COLOR',
                    (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)), i))
            elif len(h) == 3:
                toks.append(Token('COLOR',
                    (int(h[0] * 2, 16), int(h[1] * 2, 16), int(h[2] * 2, 16)), i))
            else:
                raise vgl_error(SyntaxError, f'非法颜色 #{h}', i)
            i = j
            continue
        # 字符串（v0.5 支持 \n \t \\ \" \r \0 转义，§2.4.4）
        if c == '"':
            j = i + 1
            chars = []
            special = {'n': '\n', 't': '\t', 'r': '\r', '\\': '\\',
                       '"': '"', '0': '\0'}
            while j < n and src[j] != '"':
                if src[j] == '\\' and j + 1 < n:
                    nxt = src[j + 1]
                    if nxt in special:
                        chars.append(special[nxt])
                        j += 2
                    else:
                        # 未知转义保留原样（含反斜杠）
                        chars.append('\\')
                        chars.append(nxt)
                        j += 2
                else:
                    chars.append(src[j])
                    j += 1
            if j >= n:
                raise vgl_error(SyntaxError, '未终止的字符串', i)
            toks.append(Token('STRING', ''.join(chars), i))
            i = j + 1
            continue
        # 数字（注意不吞掉 .. 范围运算符）
        if c.isdigit() or (c == '.' and i + 1 < n and src[i + 1].isdigit()):
            j = i
            while j < n:
                if src[j].isdigit():
                    j += 1
                elif src[j] == '.' and j + 1 < n and src[j + 1] != '.':
                    j += 1
                else:
                    break
            s = src[i:j]
            toks.append(Token('NUMBER', float(s) if '.' in s else int(s), i))
            i = j
            continue
        # 范围运算符 ..
        if c == '.' and i + 1 < n and src[i + 1] == '.':
            toks.append(Token('DOTDOT', '..', i))
            i += 2
            continue
        # v0.5 字段访问 .（单点，区别于 .. 范围）
        if c == '.':
            toks.append(Token('DOT', '.', i))
            i += 1
            continue
        # 标识符 / 关键字
        if c.isalpha() or c == '_':
            j = i
            while j < n and (src[j].isalnum() or src[j] == '_'):
                j += 1
            w = src[i:j]
            toks.append(Token('KEYWORD' if w in KEYWORDS else 'IDENT', w, i))
            i = j
            continue
        # 标点
        simple = {'(': 'LPAREN', ')': 'RPAREN', '{': 'LBRACE', '}': 'RBRACE',
                  '[': 'LBRACKET', ']': 'RBRACKET',
                  ',': 'COMMA', ':': 'COLON'}
        if c in simple:
            toks.append(Token(simple[c], c, i))
            i += 1
            continue
        # 多字符运算符: <= >= == !=
        if c in '<>=!' and i + 1 < n and src[i + 1] == '=':
            toks.append(Token('OP', c + '=', i))
            i += 2
            continue
        # 单字符运算符 (注意: '!' 单独非法，仅 '!=' 合法)
        if c in '+-*/=<>':
            toks.append(Token('OP', c, i))
            i += 1
            continue
        raise vgl_error(SyntaxError, f'非法字符 {c!r}', i)
    toks.append(Token('EOF', None, i))
    return toks


# ============================================================
# AST 节点
# ============================================================

class Num:
    def __init__(self, v): self.value = v
class Str:
    def __init__(self, v): self.value = v
class ColorLit:
    def __init__(self, r, g, b): self.r, self.g, self.b = r, g, b
class VarRef:
    def __init__(self, name): self.name = name
class TupleLit:
    def __init__(self, el): self.elements = el
class BinOp:
    def __init__(self, op, l, r): self.op, self.left, self.right = op, l, r
class BoolLit:
    def __init__(self, v): self.value = v  # Python bool
class LogicOp:
    """and / or — 需要短路求值，不预求值右操作数"""
    def __init__(self, op, l, r): self.op, self.left, self.right = op, l, r
class UnaryNot:
    def __init__(self, expr): self.expr = expr
class IndexExpr:
    """tuple[i] / array[i] / dict[k] 索引表达式（§3.3.4，v0.5 扩展）"""
    def __init__(self, base, index): self.base, self.index = base, index
class IndexAssign:
    """arr[i] = v / d[k] = v 索引赋值（v0.5 新增）"""
    def __init__(self, base, index, expr):
        self.base, self.index, self.expr = base, index, expr
class ArrayLit:
    """[1, 2, 3] 数组字面量（v0.5 新增）"""
    def __init__(self, el): self.elements = el
class FieldAccess:
    """obj.field 字段访问（v0.5 struct）"""
    def __init__(self, obj, name): self.obj, self.name = obj, name
class FieldAssign:
    """obj.field = expr 字段赋值（v0.5 struct）"""
    def __init__(self, obj, name, expr):
        self.obj, self.name, self.expr = obj, name, expr
class StructDef:
    """struct 类型定义（v0.5 新增）
    fields: [(name, default_expr), ...]"""
    def __init__(self, name, fields):
        self.name, self.fields = name, fields
class Call:
    def __init__(self, name, args, kwargs):
        self.name, self.args, self.kwargs = name, args, kwargs
class CanvasStmt:
    def __init__(self, w, h): self.width, self.height = w, h
class BgStmt:
    def __init__(self, color): self.color = color
class LetStmt:
    def __init__(self, name, expr): self.name, self.expr = name, expr
class AssignStmt:
    """裸赋值: name = expr（name 必须已存在，§3.2.3）"""
    def __init__(self, name, expr): self.name, self.expr = name, expr
class ForStmt:
    def __init__(self, var, start, end, body, label=None):
        self.var, self.start, self.end, self.body = var, start, end, body
        self.label = label  # 带标签 break 用，§3.2.9.1
class IfStmt:
    def __init__(self, cond, then_body, else_body):
        self.cond, self.then_body, self.else_body = cond, then_body, else_body
class WhileStmt:
    def __init__(self, cond, body, label=None):
        self.cond, self.body, self.label = cond, body, label
class BreakStmt:
    """break [label]：无 label 终止最近循环；有 label 终止匹配标签的循环"""
    def __init__(self, label=None): self.label = label
class ContinueStmt:
    """continue：跳过当前循环体剩余部分，进入下一次迭代"""
    pass
class SeedStmt:
    def __init__(self, n): self.n = n
class FnDef:
    def __init__(self, name, params, body):
        self.name, self.params, self.body = name, params, body
class ReturnStmt:
    def __init__(self, expr): self.expr = expr
class PixelStmt:
    def __init__(self, x, y, rgb): self.x, self.y, self.rgb = x, y, rgb
class StrokeStmt:
    def __init__(self, fields): self.fields = fields
class RenderStmt:
    def __init__(self, filename): self.filename = filename
class ImportStmt:
    """v0.5 import "path" 模块导入（§3.2.14）。path 为相对当前文件的 .vgl 路径。"""
    def __init__(self, path): self.path = path
class MaterialDef:
    """v0.5 批次 C 材质定义（§4.8）。
    fields: dict {field_name: expr}，常用字段 color/alpha/blend/noise。"""
    def __init__(self, name, fields):
        self.name, self.fields = name, fields
class LayerDef:
    """v0.5 批次 C 图层定义（§7.4）。name + body（在离屏缓冲区执行的语句列表）。"""
    def __init__(self, name, body):
        self.name, self.body = name, body
class FieldDef:
    """v0.5 批次 C 颜色场定义（§7.5）。name + params（应含 x,y） + body。
    语义上为 (x, y) → color 的纯函数，注册为 Closure。"""
    def __init__(self, name, params, body):
        self.name, self.params, self.body = name, params, body
class ExprStmt:
    def __init__(self, expr): self.expr = expr


# ============================================================
# 语法分析（递归下降）
# ============================================================

class Parser:
    def __init__(self, tokens):
        self.toks = tokens
        self.pos = 0
        self.loop_depth = 0  # 用于校验 break 必须在循环体内

    def peek(self, off=0):
        return self.toks[self.pos + off]

    def advance(self):
        t = self.toks[self.pos]
        self.pos += 1
        return t

    def expect(self, ttype, val=None):
        t = self.peek()
        if t.type != ttype or (val is not None and t.value != val):
            expected = ttype if val is None else f'{ttype} {val!r}'
            got = t.type if t.value is None else f'{t.type} {t.value!r}'
            raise vgl_error(SyntaxError, f'期望 {expected}, 得到 {got}', t.pos)
        return self.advance()

    def parse_program(self):
        stmts = []
        while self.peek().type != 'EOF':
            stmts.append(self.parse_stmt())
        return stmts

    def parse_stmt(self):
        # v0.5: 记录语句起始位置用于错误定位（§8.2 行号/列号/caret）
        start_pos = self.peek().pos
        stmt = self._parse_stmt_impl()
        try:
            stmt.pos = start_pos
        except Exception:
            pass
        return stmt

    def _parse_stmt_impl(self):
        t = self.peek()
        # 带标签循环: IDENT ':' for/while ...（§3.2.9.1 带标签 break）
        if t.type == 'IDENT' and self.peek(1).type == 'COLON' \
                and self.peek(2).type == 'KEYWORD' and self.peek(2).value in ('for', 'while'):
            label = self.advance().value
            self.advance()  # consume ':'
            stmt = self.parse_stmt()
            stmt.label = label  # ForStmt / WhileStmt 均有 label 字段
            return stmt
        if t.type == 'KEYWORD':
            dispatch = {
                'canvas': self.parse_canvas, 'bg': self.parse_bg,
                'let': self.parse_let, 'for': self.parse_for,
                'if': self.parse_if, 'fn': self.parse_fn,
                'return': self.parse_return, 'pixel': self.parse_pixel,
                'stroke': self.parse_stroke, 'render': self.parse_render,
                # v0.3 新增
                'while': self.parse_while, 'break': self.parse_break,
                'seed': self.parse_seed,
                # v0.4 新增
                'continue': self.parse_continue,
                # v0.5 新增
                'struct': self.parse_struct,
                # v0.5 批次 B 新增
                'import': self.parse_import,
                # v0.5 批次 C 新增
                'material': self.parse_material,
                'layer': self.parse_layer,
                'field': self.parse_field,
            }
            if t.value in dispatch:
                return dispatch[t.value]()
        # 裸赋值语句: IDENT '=' Expr  （需与 ExprStmt 区分：看 IDENT 后是否紧跟 '='）
        # 注意: '==' 是比较运算符，已在词法层合并为单个 OP token，不会误判
        if t.type == 'IDENT' and self.peek(1).type == 'OP' and self.peek(1).value == '=':
            return self.parse_assign()
        # v0.5 字段赋值: IDENT '.' IDENT '=' Expr  → obj.field = v
        if t.type == 'IDENT' and self.peek(1).type == 'DOT' \
                and self.peek(2).type == 'IDENT' \
                and self.peek(3).type == 'OP' and self.peek(3).value == '=':
            return self.parse_field_assign()
        # v0.5 索引赋值: IDENT '[' ... ']' '=' Expr  → arr[i] = v / d[k] = v
        if t.type == 'IDENT' and self.peek(1).type == 'LBRACKET':
            return self.parse_index_assign()
        return ExprStmt(self.parse_expr())

    def parse_assign(self):
        name = self.expect('IDENT').value
        self.expect('OP', '=')
        return AssignStmt(name, self.parse_expr())

    def parse_canvas(self):
        self.expect('KEYWORD', 'canvas')
        w = self.expect('NUMBER').value
        x_tok = self.advance()
        # 兼容 "400x300"（x300 被词法合并）和 "400 x 300"（分开）
        if x_tok.type == 'IDENT' and x_tok.value.startswith('x') and x_tok.value[1:].isdigit():
            h = int(x_tok.value[1:])
            return CanvasStmt(w, h)
        if x_tok.type == 'IDENT' and x_tok.value == 'x':
            h = self.expect('NUMBER').value
            return CanvasStmt(w, h)
        raise SyntaxError(f'canvas 声明格式应为 WxH, 得到 {x_tok.value!r}')

    def parse_bg(self):
        self.expect('KEYWORD', 'bg')
        return BgStmt(self.parse_color())

    def parse_color(self):
        t = self.peek()
        if t.type == 'COLOR':
            self.advance()
            return ColorLit(t.value[0], t.value[1], t.value[2])
        raise vgl_error(SyntaxError, f'期望颜色, 得到 {t.type}', t.pos)

    def parse_let(self):
        self.expect('KEYWORD', 'let')
        name = self.expect('IDENT').value
        self.expect('OP', '=')
        return LetStmt(name, self.parse_expr())

    def parse_struct(self):
        """v0.5 struct 定义: struct Name { field: default, field: default, ... }"""
        self.expect('KEYWORD', 'struct')
        name = self.expect('IDENT').value
        self.expect('LBRACE')
        fields = []  # [(field_name, default_expr), ...]
        while self.peek().type != 'RBRACE':
            fname = self.expect('IDENT').value
            self.expect('COLON')
            default = self.parse_expr()
            fields.append((fname, default))
            if self.peek().type == 'COMMA':
                self.advance()
            else:
                break
        self.expect('RBRACE')
        return StructDef(name, fields)

    def parse_field_assign(self):
        """v0.5 字段赋值: obj.field = expr（obj 为已存在的 IDENT 引用）"""
        name = self.expect('IDENT').value
        self.expect('DOT')
        field = self.expect('IDENT').value
        self.expect('OP', '=')
        return FieldAssign(VarRef(name), field, self.parse_expr())

    def parse_index_assign(self):
        """v0.5 索引赋值: arr[i] = v / d[k] = v
        通过解析为表达式后跟 '=' 识别。"""
        name = self.expect('IDENT').value
        self.expect('LBRACKET')
        idx = self.parse_expr()
        self.expect('RBRACKET')
        self.expect('OP', '=')
        return IndexAssign(VarRef(name), idx, self.parse_expr())

    def parse_for(self):
        self.expect('KEYWORD', 'for')
        var = self.expect('IDENT').value
        self.expect('KEYWORD', 'in')
        start = self.parse_expr()
        self.expect('DOTDOT')
        end = self.parse_expr()
        self.expect('LBRACE')
        self.loop_depth += 1
        body = []
        while self.peek().type != 'RBRACE':
            body.append(self.parse_stmt())
        self.expect('RBRACE')
        self.loop_depth -= 1
        return ForStmt(var, start, end, body)

    def parse_while(self):
        self.expect('KEYWORD', 'while')
        cond = self.parse_expr()
        self.expect('LBRACE')
        self.loop_depth += 1
        body = []
        while self.peek().type != 'RBRACE':
            body.append(self.parse_stmt())
        self.expect('RBRACE')
        self.loop_depth -= 1
        return WhileStmt(cond, body)

    def parse_break(self):
        self.expect('KEYWORD', 'break')
        if self.loop_depth == 0:
            raise SyntaxError('break 只能出现在 for / while 循环体内')
        # 可选标签: break label（§3.2.9.1）
        label = None
        if self.peek().type == 'IDENT':
            label = self.advance().value
        return BreakStmt(label)

    def parse_continue(self):
        self.expect('KEYWORD', 'continue')
        if self.loop_depth == 0:
            raise vgl_error(SyntaxError, 'continue 只能出现在 for / while 循环体内', self.peek().pos)
        return ContinueStmt()

    def parse_seed(self):
        self.expect('KEYWORD', 'seed')
        n = self.expect('NUMBER').value
        if not isinstance(n, int):
            raise SyntaxError(f'seed 要求整数参数, 得到 {n!r}')
        return SeedStmt(n)

    def parse_if(self):
        self.expect('KEYWORD', 'if')
        cond = self.parse_expr()
        self.expect('LBRACE')
        then_body = []
        while self.peek().type != 'RBRACE':
            then_body.append(self.parse_stmt())
        self.expect('RBRACE')
        else_body = None
        if self.peek().type == 'KEYWORD' and self.peek().value == 'else':
            self.advance()
            self.expect('LBRACE')
            else_body = []
            while self.peek().type != 'RBRACE':
                else_body.append(self.parse_stmt())
            self.expect('RBRACE')
        return IfStmt(cond, then_body, else_body)

    def parse_fn(self):
        self.expect('KEYWORD', 'fn')
        name = self.expect('IDENT').value
        self.expect('LPAREN')
        params = []
        if self.peek().type != 'RPAREN':
            params.append(self.expect('IDENT').value)
            while self.peek().type == 'COMMA':
                self.advance()
                params.append(self.expect('IDENT').value)
        self.expect('RPAREN')
        self.expect('LBRACE')
        body = []
        while self.peek().type != 'RBRACE':
            body.append(self.parse_stmt())
        self.expect('RBRACE')
        return FnDef(name, params, body)

    def parse_return(self):
        self.expect('KEYWORD', 'return')
        return ReturnStmt(self.parse_expr())

    def parse_pixel(self):
        self.expect('KEYWORD', 'pixel')
        self.expect('LPAREN')
        fields = self.parse_kwargs()
        self.expect('RPAREN')
        return PixelStmt(fields.get('x'), fields.get('y'), fields.get('rgb'))

    def parse_stroke(self):
        self.expect('KEYWORD', 'stroke')
        self.expect('LBRACE')
        fields = self.parse_kwargs()
        self.expect('RBRACE')
        return StrokeStmt(fields)

    def parse_render(self):
        self.expect('KEYWORD', 'render')
        return RenderStmt(self.expect('STRING').value)

    def parse_import(self):
        """v0.5 批次 B: import "path/file.vgl" 模块导入（§3.2.14）"""
        self.expect('KEYWORD', 'import')
        path_tok = self.expect('STRING')
        return ImportStmt(path_tok.value)

    def parse_material(self):
        """v0.5 批次 C: material Name { field: expr, ... }（§4.8）"""
        self.expect('KEYWORD', 'material')
        name = self.expect('IDENT').value
        self.expect('LBRACE')
        fields = self.parse_kwargs()
        self.expect('RBRACE')
        return MaterialDef(name, fields)

    def parse_layer(self):
        """v0.5 批次 C: layer Name { ... } 图层定义（§7.4）
        body 在离屏缓冲区执行，结束后用 compose() 合成到主画布。"""
        self.expect('KEYWORD', 'layer')
        name = self.expect('IDENT').value
        self.expect('LBRACE')
        body = []
        while self.peek().type != 'RBRACE':
            body.append(self.parse_stmt())
        self.expect('RBRACE')
        return LayerDef(name, body)

    def parse_field(self):
        """v0.5 批次 C: field Name(x, y) { ... return color } 颜色场（§7.5）
        语义等同 fn，但注册为 FieldClosure，配合 fill() 遍历画布。"""
        self.expect('KEYWORD', 'field')
        name = self.expect('IDENT').value
        self.expect('LPAREN')
        params = []
        if self.peek().type != 'RPAREN':
            params.append(self.expect('IDENT').value)
            while self.peek().type == 'COMMA':
                self.advance()
                params.append(self.expect('IDENT').value)
        self.expect('RPAREN')
        self.expect('LBRACE')
        body = []
        while self.peek().type != 'RBRACE':
            body.append(self.parse_stmt())
        self.expect('RBRACE')
        return FieldDef(name, params, body)

    def parse_kwargs(self):
        """解析 key: val, key: val, ... 形式
        v0.5 批次 C：允许 KEYWORD 作为 key（如 stroke { material: ... }，
        pixel/parse_material 等场景，关键字作为字段名不应被拒绝）"""
        fields = {}
        while self.peek().type != 'RPAREN' and self.peek().type != 'RBRACE':
            t = self.peek()
            if t.type in ('IDENT', 'KEYWORD'):
                key = self.advance().value
            else:
                raise vgl_error(SyntaxError, f'期望字段名(IDENT/KEYWORD), 得到 {t.type}', t.pos)
            self.expect('COLON')
            fields[key] = self.parse_expr()
            if self.peek().type == 'COMMA':
                self.advance()
        return fields

    # 表达式优先级链（低 → 高）:
    #   parse_or (2) → parse_and (3) → parse_compare (4) → parse_add (5)
    #   → parse_mul (6) → parse_unary (7) → parse_primary (8)
    # 对应 v0.3 规范 §3.3.1 优先级表
    def parse_expr(self):
        return self.parse_or()

    def parse_or(self):
        left = self.parse_and()
        while self.peek().type == 'KEYWORD' and self.peek().value == 'or':
            self.advance()
            left = LogicOp('or', left, self.parse_and())
        return left

    def parse_and(self):
        left = self.parse_compare()
        while self.peek().type == 'KEYWORD' and self.peek().value == 'and':
            self.advance()
            left = LogicOp('and', left, self.parse_compare())
        return left

    def parse_compare(self):
        left = self.parse_add()
        # 比较运算符无结合性: 仅允许单个比较（a < b < c 非法）
        if self.peek().type == 'OP' and self.peek().value in ('<', '>', '<=', '>=', '==', '!='):
            op = self.advance().value
            right = self.parse_add()
            return BinOp(op, left, right)
        return left

    def parse_add(self):
        left = self.parse_mul()
        while self.peek().type == 'OP' and self.peek().value in '+-':
            op = self.advance().value
            left = BinOp(op, left, self.parse_mul())
        return left

    def parse_mul(self):
        left = self.parse_unary()
        while self.peek().type == 'OP' and self.peek().value in '*/':
            op = self.advance().value
            left = BinOp(op, left, self.parse_unary())
        return left

    def parse_unary(self):
        if self.peek().type == 'OP' and self.peek().value == '-':
            self.advance()
            return BinOp('-', Num(0), self.parse_unary())
        if self.peek().type == 'KEYWORD' and self.peek().value == 'not':
            self.advance()
            return UnaryNot(self.parse_unary())
        return self.parse_primary()

    def parse_primary(self):
        t = self.peek()
        node = None
        if t.type == 'NUMBER':
            self.advance()
            node = Num(t.value)
        elif t.type == 'STRING':
            self.advance()
            node = Str(t.value)
        elif t.type == 'COLOR':
            self.advance()
            node = ColorLit(t.value[0], t.value[1], t.value[2])
        elif t.type == 'KEYWORD' and t.value in ('true', 'false'):
            self.advance()
            node = BoolLit(t.value == 'true')
        elif t.type == 'LPAREN':
            self.advance()
            first = self.parse_expr()
            if self.peek().type == 'COMMA':  # 元组
                el = [first]
                while self.peek().type == 'COMMA':
                    self.advance()
                    el.append(self.parse_expr())
                self.expect('RPAREN')
                node = TupleLit(el)
            else:
                self.expect('RPAREN')
                node = first
        elif t.type == 'LBRACKET':
            # v0.5 数组字面量: [a, b, c]（空数组 []）
            self.advance()
            el = []
            if self.peek().type != 'RBRACKET':
                el.append(self.parse_expr())
                while self.peek().type == 'COMMA':
                    self.advance()
                    el.append(self.parse_expr())
            self.expect('RBRACKET')
            node = ArrayLit(el)
        elif t.type == 'IDENT':
            self.advance()
            name = t.value
            if self.peek().type == 'LPAREN':  # 函数调用 / struct 构造
                self.advance()
                args, kwargs = [], {}
                if self.peek().type != 'RPAREN':
                    # 判断是关键字参数还是位置参数
                    if self.peek().type == 'IDENT' and self.peek(1).type == 'COLON':
                        kwargs = self.parse_kwargs()
                    else:
                        args.append(self.parse_expr())
                        while self.peek().type == 'COMMA':
                            self.advance()
                            args.append(self.parse_expr())
                self.expect('RPAREN')
                node = Call(name, args, kwargs)
            else:
                node = VarRef(name)
        else:
            raise vgl_error(SyntaxError, f'意外标记 {t.type} {t.value!r}', t.pos)
        # 后缀索引: base[i]（§3.3.4），支持连续索引 a[i][j]
        while self.peek().type == 'LBRACKET':
            self.advance()
            idx = self.parse_expr()
            self.expect('RBRACKET')
            node = IndexExpr(node, idx)
        # v0.5 后缀字段访问: obj.field（支持连续 obj.a.b）
        while self.peek().type == 'DOT':
            self.advance()
            field = self.expect('IDENT').value
            node = FieldAccess(node, field)
        return node


# ============================================================
# 解释器（树遍历）
# ============================================================

class ReturnSignal(Exception):
    def __init__(self, val): self.value = val


class BreakSignal(Exception):
    """break 语句的信号，由 for/while 循环体捕获。label 为 None 表示无标签 break。"""
    def __init__(self, label=None): self.label = label


class ContinueSignal(Exception):
    """continue 语句的信号，由 for/while 循环体捕获"""
    pass


class Environment:
    """词法作用域环境链（§5.1, §5.2）。vars 为本层绑定，parent 指向外层。
    顶层全局环境的 parent 为 None。"""
    __slots__ = ('vars', 'parent')

    def __init__(self, parent=None):
        self.vars = {}
        self.parent = parent

    def find_env(self, name):
        """返回 name 所在的 Environment；未找到返回 None。"""
        env = self
        while env is not None:
            if name in env.vars:
                return env
            env = env.parent
        return None


class Closure:
    """函数闭包对象（§5.3）。捕获定义时的词法环境，支持可变捕获。"""
    __slots__ = ('name', 'params', 'body', 'def_env')

    def __init__(self, name, params, body, def_env):
        self.name = name
        self.params = params
        self.body = body
        self.def_env = def_env

    def __repr__(self):
        return f'<closure {self.name}({",".join(self.params)})>'


class StructDefn:
    """struct 类型定义对象（v0.5 §4.4）。
    fields: [(name, default_value), ...]，default_value 为已求值的 Python 值。"""
    __slots__ = ('name', 'fields')

    def __init__(self, name, fields):
        self.name = name
        self.fields = fields  # list of (name, default_value)

    def __repr__(self):
        return f'<struct {self.name}>'


class StructInstance:
    """struct 实例对象（v0.5 §4.4）。fields 为 dict。"""
    __slots__ = ('struct_name', 'fields')

    def __init__(self, struct_name, fields):
        self.struct_name = struct_name
        self.fields = fields  # dict

    def __repr__(self):
        items = ', '.join(f'{k}={v!r}' for k, v in self.fields.items())
        return f'<{self.struct_name} {items}>'


class MaterialInstance:
    """材质实例（v0.5 批次 C §4.8）。fields 为 dict。
    常用字段: color / alpha / blend / noise / gradient。"""
    __slots__ = ('name', 'fields')

    def __init__(self, name, fields):
        self.name = name
        self.fields = fields  # dict

    def __repr__(self):
        return f'<material {self.name}>'


class LayerInstance:
    """图层对象（v0.5 批次 C §7.4）。存储离屏缓冲区。"""
    __slots__ = ('name', 'width', 'height', 'buf')

    def __init__(self, name, width, height, buf):
        self.name = name
        self.width = width
        self.height = height
        self.buf = buf  # bytearray, 每像素 3 字节 RGB

    def __repr__(self):
        return f'<layer {self.name} {self.width}x{self.height}>'


def _bool_fn(x):
    """bool(x) 内建: 0/0.0/false → False, 否则 True (§6.1, §4.2)"""
    if isinstance(x, bool):
        return x
    if isinstance(x, (int, float)):
        return x != 0
    return bool(x)


def _dot(a, b):
    """dot(a, b) 点积 (§6.2)。要求 a, b 为同长度 tuple(2/3)。"""
    if not (isinstance(a, tuple) and isinstance(b, tuple)):
        raise TypeError(f'dot 要求两个 tuple 参数, 得到 {type(a).__name__}/{type(b).__name__}')
    if len(a) != len(b):
        raise TypeError(f'dot 要求同长度 tuple: {len(a)} vs {len(b)}')
    if len(a) not in (2, 3):
        raise TypeError(f'dot 仅支持 tuple(2) 或 tuple(3), 得到 tuple({len(a)})')
    return sum(x * y for x, y in zip(a, b))


def _length(p1, p2):
    """length(p1, p2) 欧氏距离 (§6.2)。"""
    if not (isinstance(p1, tuple) and isinstance(p2, tuple)):
        raise TypeError(f'length 要求两个 tuple 参数, 得到 {type(p1).__name__}/{type(p2).__name__}')
    if len(p1) != len(p2):
        raise TypeError(f'length 要求同长度 tuple: {len(p1)} vs {len(p2)}')
    if len(p1) not in (2, 3):
        raise TypeError(f'length 仅支持 tuple(2) 或 tuple(3), 得到 tuple({len(p1)})')
    return (sum((a - b) ** 2 for a, b in zip(p1, p2))) ** 0.5


# v0.5 复合数据内建函数（§6.3）
def _len_fn(x):
    """len(x): tuple / array / dict / string 的长度"""
    if isinstance(x, (tuple, list, dict, str)):
        return len(x)
    raise TypeError(f'len 不支持 {type(x).__name__}')


def _push_fn(arr, v):
    """push(arr, v): array 末尾追加 v（原地修改，返回 None）"""
    if not isinstance(arr, list):
        raise TypeError(f'push 要求 array 参数, 得到 {type(arr).__name__}')
    arr.append(v)
    return None


def _pop_fn(arr):
    """pop(arr): array 末尾弹出"""
    if not isinstance(arr, list):
        raise TypeError(f'pop 要求 array 参数, 得到 {type(arr).__name__}')
    if not arr:
        raise ValueError('pop 空数组')
    return arr.pop()


def _array_fn(*args):
    """array() → 空数组；array(n) → n 个 None 的数组；array(a, b, c) → [a, b, c]"""
    if len(args) == 0:
        return []
    if len(args) == 1 and isinstance(args[0], (int, float)) and not isinstance(args[0], bool):
        return [None] * int(args[0])
    return list(args)


def _dict_fn(*args):
    """dict(k1, v1, k2, v2, ...): 交替键值构造字典"""
    if len(args) % 2 != 0:
        raise TypeError(f'dict 要求偶数个参数 (k, v 交替), 得到 {len(args)} 个')
    d = {}
    for i in range(0, len(args), 2):
        d[args[i]] = args[i + 1]
    return d


def _keys_fn(d):
    """keys(d): 返回 dict 键的 array"""
    if not isinstance(d, dict):
        raise TypeError(f'keys 要求 dict 参数, 得到 {type(d).__name__}')
    return list(d.keys())


def _values_fn(d):
    """values(d): 返回 dict 值的 array"""
    if not isinstance(d, dict):
        raise TypeError(f'values 要求 dict 参数, 得到 {type(d).__name__}')
    return list(d.values())


def _has_fn(d, k):
    """has(d, k): dict 是否含键 k"""
    if not isinstance(d, dict):
        raise TypeError(f'has 要求 dict 参数, 得到 {type(d).__name__}')
    return k in d


def _bezier(p1, p2, p3, p4):
    """三次贝塞尔 (§6.2)。返回 ('bezier', p1, p2, p3, p4) 标记元组。"""
    for name, p in [('p1', p1), ('p2', p2), ('p3', p3), ('p4', p4)]:
        if not (isinstance(p, tuple) and len(p) == 2):
            raise TypeError(f'bezier 控制点 {name} 必须为 tuple(2), 得到 {type(p).__name__}')
    return ('bezier', p1, p2, p3, p4)


def _qbezier(p1, p2, p3):
    """二次贝塞尔 (§6.2)。返回 ('qbezier', p1, p2, p3)。"""
    for name, p in [('p1', p1), ('p2', p2), ('p3', p3)]:
        if not (isinstance(p, tuple) and len(p) == 2):
            raise TypeError(f'qbezier 控制点 {name} 必须为 tuple(2), 得到 {type(p).__name__}')
    return ('qbezier', p1, p2, p3)


def _path(points):
    """折线 path (§6.2)。points: tuple of tuple(2)。"""
    if not isinstance(points, tuple):
        raise TypeError(f'path 要求 tuple 参数, 得到 {type(points).__name__}')
    if len(points) < 2:
        raise TypeError(f'path 至少需要 2 个点, 得到 {len(points)}')
    for i, p in enumerate(points):
        if not (isinstance(p, tuple) and len(p) == 2):
            raise TypeError(f'path 第 {i} 个点必须为 tuple(2), 得到 {type(p).__name__}')
    return ('polyline', points)


# ============================================================
# v0.5 批次 C 噪声函数（§6.3 perlin / worley / fbm）
# ============================================================

# Perlin 置换表：基于固定种子的 256 排列表，避免每次调用随机
_PERLIN_PERM = list(range(256))
random.Random(20250705).shuffle(_PERLIN_PERM)
_PERLIN_PERM = _PERLIN_PERM + _PERLIN_PERM  # 扩展到 512 以省去 & 255


def _perlin_grad2(hash_val, x, y):
    """2D 梯度向量：根据 hash 选择 (x,y), (-x,y), (x,-y), (-x,-y), (y,x), ..."""
    h = hash_val & 7
    u = x if h < 4 else y
    v = y if h < 4 else x
    return (u if (h & 1) == 0 else -u) + (v if (h & 2) == 0 else -v)


def _perlin_fade(t):
    """缓和曲线 fade(t) = 6t^5 - 15t^4 + 10t^3（C1 连续导数）"""
    return t * t * t * (t * (t * 6 - 15) + 10)


def perlin(x, y):
    """2D Perlin 噪声（§6.3）。返回 [-1, 1] 区间浮点数。"""
    xi = int(math.floor(x)) & 255
    yi = int(math.floor(y)) & 255
    xf = x - math.floor(x)
    yf = y - math.floor(y)
    u = _perlin_fade(xf)
    v = _perlin_fade(yf)
    p = _PERLIN_PERM
    aa = p[p[xi] + yi]
    ab = p[p[xi] + yi + 1]
    ba = p[p[xi + 1] + yi]
    bb = p[p[xi + 1] + yi + 1]
    x1 = _lerp_scalar(_perlin_grad2(aa, xf, yf),
                      _perlin_grad2(ba, xf - 1, yf), u)
    x2 = _lerp_scalar(_perlin_grad2(ab, xf, yf - 1),
                      _perlin_grad2(bb, xf - 1, yf - 1), u)
    return _lerp_scalar(x1, x2, v)


def _lerp_scalar(a, b, t):
    return a + t * (b - a)


def worley(x, y):
    """2D Worley 噪声（cellular noise，§6.3）。
    返回最近特征点的欧氏距离（[0, ~cell_size] 区间）。"""
    cell_size = 32
    cx = int(math.floor(x / cell_size))
    cy = int(math.floor(y / cell_size))
    min_dist = float('inf')
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            ncx = cx + dx
            ncy = cy + dy
            # 用 hash 在每个 cell 内确定一个固定特征点
            h = (ncx * 374761393 + ncy * 668265263) & 0xffffffff
            h = ((h ^ (h >> 13)) * 1274126177) & 0xffffffff
            h = (h ^ (h >> 16)) & 0xffffffff
            px = ncx * cell_size + (h % cell_size)
            py = ncy * cell_size + ((h >> 8) % cell_size)
            d = math.sqrt((x - px) ** 2 + (y - py) ** 2)
            if d < min_dist:
                min_dist = d
    return min_dist


def fbm(x, y, octaves=4):
    """分形布朗运动（Fractal Brownian Motion，§6.3）。
    多倍频 Perlin 叠加，振幅减半频率倍增。返回 [-1, 1] 区间。"""
    if not isinstance(octaves, (int, float)) or isinstance(octaves, bool):
        raise TypeError(f'fbm octaves 要求 number, 得到 {type(octaves).__name__}')
    octaves = int(octaves)
    if octaves < 1:
        octaves = 1
    if octaves > 8:
        octaves = 8
    total = 0.0
    amp = 1.0
    freq = 1.0
    norm = 0.0
    for _ in range(octaves):
        total += perlin(x * freq, y * freq) * amp
        norm += amp
        amp *= 0.5
        freq *= 2.0
    return total / norm if norm > 0 else 0.0


class Interpreter:
    def __init__(self):
        self.cw = 0
        self.ch = 0
        self.buf = None  # bytearray
        self.globals = Environment(parent=None)  # v0.3: 词法作用域链根
        # funcs 不再单独存储：fn 定义在全局 env 中创建 Closure 绑定，
        # 闭包自带 def_env。保留 self.funcs 仅用于向后兼容旧式顶层 fn 调用检测。
        self.funcs = {}
        # v0.5 struct 类型注册表：name -> StructDefn
        self.structs = {}
        # v0.5 批次 C 材质注册表：name -> MaterialInstance
        self.materials = {}
        # v0.5 批次 C 图层注册表：name -> LayerInstance（含离屏缓冲区）
        self.layers = {}
        # v0.5 §8.2 错误定位：当前执行语句的字符偏移
        self.current_pos = None
        # v0.5 批次 B 模块导入（§3.2.14）
        self.current_dir = None      # 当前文件所在目录（解析相对路径）
        self.current_filename = None  # 当前文件名（错误信息）
        self.current_src = None      # 当前源码（错误定位）
        self.imported = set()        # 已导入文件绝对路径集合（去重 + 防循环）
        self.builtins = {
            'rand': lambda a, b: random.uniform(a, b),
            'int': int,
            'abs': abs,
            'floor': math.floor,
            'ceil': math.ceil,
            'sin': math.sin,
            'cos': math.cos,
            'min': min,
            'max': max,
            'bool': _bool_fn,  # v0.3
            'pow': math.pow,   # v0.3
            'sqrt': math.sqrt,  # v0.3
            'line': lambda p1, p2: ('line', p1, p2),
            'circle': lambda cx, cy, r: ('circle', cx, cy, r),
            # v0.3 几何扩展
            'bezier': _bezier,
            'qbezier': _qbezier,
            'path': _path,
            'dot': _dot,
            'length': _length,
            # v0.5 复合数据
            'len': _len_fn,        # tuple/array/dict/str 长度
            'push': _push_fn,      # array 末尾追加（原地修改）
            'pop': _pop_fn,        # array 末尾弹出
            'array': _array_fn,    # 构造空数组或指定大小数组
            'dict': _dict_fn,      # 构造字典 dict(k1, v1, k2, v2, ...)
            'keys': _keys_fn,      # dict 键数组
            'values': _values_fn,  # dict 值数组
            'has': _has_fn,        # dict 是否含键
            # v0.5 批次 C 噪声（§6.3）
            'perlin': perlin,      # 2D Perlin，[-1,1]
            'worley': worley,      # 2D Worley，最近特征点距离
            'fbm': fbm,            # 分形布朗运动，[-1,1]
            # v0.5 批次 C 图层/颜色场（§7.4, §7.5）
            'compose': self.compose_layer,  # compose(name, blend) 图层合成
            'fill': self.fill_field,        # fill(name) 颜色场填充画布
        }

    def run(self, ast):
        for stmt in ast:
            self.exec(stmt, self.globals)

    @staticmethod
    def truthy(v):
        """条件求值: number 非零为真, bool 直接, None 为假 (§3.2.9, §4.2)"""
        if isinstance(v, bool):
            return v
        if isinstance(v, (int, float)):
            return v != 0
        if v is None:
            return False
        return bool(v)

    @staticmethod
    def _is_num(v):
        """number 判定（bool 在 Python 中是 int 子类，需排除）"""
        return isinstance(v, (int, float)) and not isinstance(v, bool)

    def _binop_arith(self, op, l, r):
        """§4.3 元组广播运算。返回 None 表示两侧均非 tuple，调用方走普通算术。"""
        l_is_tup = isinstance(l, tuple)
        r_is_tup = isinstance(r, tuple)
        if not l_is_tup and not r_is_tup:
            return None  # 普通数值算术，交给后续分支

        # tuple ± tuple（要求同长度）
        if l_is_tup and r_is_tup:
            if len(l) != len(r):
                raise TypeError(f'元组长度不匹配: {len(l)} vs {len(r)}')
            if op == '+':
                return tuple(a + b for a, b in zip(l, r))
            if op == '-':
                return tuple(a - b for a, b in zip(l, r))
            if op == '*':
                raise TypeError('tuple * tuple 非法（使用 dot() 计算点积）')
            if op == '/':
                raise TypeError('tuple / tuple 非法')

        # tuple * number / number * tuple（标量广播）
        if op == '*':
            if l_is_tup and self._is_num(r):
                return tuple(a * r for a in l)
            if r_is_tup and self._is_num(l):
                return tuple(l * a for a in r)
            raise TypeError(f'tuple * <非 number> 非法: {type(r if l_is_tup else l).__name__}')

        # tuple / number（仅 tuple 在左）
        if op == '/':
            if l_is_tup and self._is_num(r):
                if r == 0:
                    raise ZeroDivisionError('tuple 除以零')
                return tuple(a / r for a in l)
            # number / tuple 或 tuple / tuple 已在前面处理
            raise TypeError('仅支持 tuple / number')

        # tuple ± number（非法，§4.3 表格明确禁止）
        if op in ('+', '-'):
            raise TypeError(f'tuple {op} number 非法（歧义：标量广播？拼接？）')

        return None

    # --- 语句执行 ---

    def exec(self, stmt, env):
        # v0.5 §8.2: 更新当前语句位置（用于运行时错误定位）
        pos = getattr(stmt, 'pos', None)
        if pos is not None:
            self.current_pos = pos
        if isinstance(stmt, CanvasStmt):
            self.cw, self.ch = stmt.width, stmt.height
            self.buf = bytearray(self.cw * self.ch * 3)
            self._bg_color = (0, 0, 0)  # v0.5 批次 C：图层离屏缓冲区初始化用
        elif isinstance(stmt, BgStmt):
            r, g, b = self.eval(stmt.color, env)
            for i in range(0, len(self.buf), 3):
                self.buf[i] = r; self.buf[i + 1] = g; self.buf[i + 2] = b
            self._bg_color = (r, g, b)  # v0.5 批次 C：记录背景色供图层用
        elif isinstance(stmt, LetStmt):
            env.vars[stmt.name] = self.eval(stmt.expr, env)
        elif isinstance(stmt, AssignStmt):
            # 裸赋值（§3.2.3）：仅修改已存在绑定，沿词法作用域链查找
            target = env.find_env(stmt.name)
            if target is None:
                raise NameError(f'赋值给未声明变量: {stmt.name}（应使用 let 声明）')
            target.vars[stmt.name] = self.eval(stmt.expr, env)
        elif isinstance(stmt, StructDef):
            # v0.5 struct 定义：求值字段默认值，注册到 self.structs
            fields = [(fname, self.eval(default, env)) for fname, default in stmt.fields]
            self.structs[stmt.name] = StructDefn(stmt.name, fields)
        elif isinstance(stmt, FieldAssign):
            # v0.5 obj.field = expr
            obj = self.eval(stmt.obj, env)
            if not isinstance(obj, StructInstance):
                raise TypeError(f'字段赋值要求 struct 实例, 得到 {type(obj).__name__}')
            if stmt.name not in obj.fields:
                raise AttributeError(f'struct {obj.struct_name} 无字段 {stmt.name}')
            obj.fields[stmt.name] = self.eval(stmt.expr, env)
        elif isinstance(stmt, IndexAssign):
            # v0.5 arr[i] = v / d[k] = v
            base = self.eval(stmt.base, env)
            idx = self.eval(stmt.index, env)
            val = self.eval(stmt.expr, env)
            if isinstance(base, list):
                if not isinstance(idx, (int, float)) or isinstance(idx, bool):
                    raise TypeError(f'array 索引必须为整数, 得到 {type(idx).__name__}')
                idx = int(idx)
                if idx < 0 or idx >= len(base):
                    raise IndexError(f'array 索引越界: {idx} (长度 {len(base)})')
                base[idx] = val
            elif isinstance(base, dict):
                base[idx] = val  # dict 不存在则创建
            else:
                raise TypeError(f'索引赋值要求 array 或 dict, 得到 {type(base).__name__}')
        elif isinstance(stmt, ForStmt):
            start = self.eval(stmt.start, env)
            end = self.eval(stmt.end, env)
            i = start
            while i < end:
                # v0.4 块作用域：每次迭代创建子 Environment（§5.1）
                block_env = Environment(parent=env)
                block_env.vars[stmt.var] = i
                try:
                    for s in stmt.body:
                        self.exec(s, block_env)
                except BreakSignal as bs:
                    # 无标签 break 终止本循环；带标签 break 匹配则终止，否则向上传播
                    if bs.label is None or bs.label == stmt.label:
                        break
                    raise
                except ContinueSignal:
                    pass  # continue 进入下一次迭代
                i += 1
        elif isinstance(stmt, WhileStmt):
            while self.truthy(self.eval(stmt.cond, env)):
                # v0.4 块作用域：每次迭代创建子 Environment（§5.1）
                block_env = Environment(parent=env)
                try:
                    for s in stmt.body:
                        self.exec(s, block_env)
                except BreakSignal as bs:
                    if bs.label is None or bs.label == stmt.label:
                        break
                    raise
                except ContinueSignal:
                    pass
        elif isinstance(stmt, BreakStmt):
            raise BreakSignal(stmt.label)
        elif isinstance(stmt, ContinueStmt):
            raise ContinueSignal()
        elif isinstance(stmt, SeedStmt):
            random.seed(stmt.n)
        elif isinstance(stmt, IfStmt):
            # v0.4 块作用域：if/else 体创建子 Environment（§5.1）
            if self.truthy(self.eval(stmt.cond, env)):
                block_env = Environment(parent=env)
                for s in stmt.then_body:
                    self.exec(s, block_env)
            elif stmt.else_body:
                block_env = Environment(parent=env)
                for s in stmt.else_body:
                    self.exec(s, block_env)
        elif isinstance(stmt, FnDef):
            # v0.3 闭包：捕获定义时环境 def_env（§5.3）
            closure = Closure(stmt.name, stmt.params, stmt.body, env)
            env.vars[stmt.name] = closure
            # 兼容：顶层 fn 也注册到 self.funcs（保留旧路径，便于 Call 优先查闭包）
            self.funcs[stmt.name] = (stmt.params, stmt.body, env)
        elif isinstance(stmt, ReturnStmt):
            raise ReturnSignal(self.eval(stmt.expr, env) if stmt.expr else None)
        elif isinstance(stmt, PixelStmt):
            x = int(self.eval(stmt.x, env))
            y = int(self.eval(stmt.y, env))
            rgb = self.eval(stmt.rgb, env)
            self.put_pixel(x, y, rgb)
        elif isinstance(stmt, StrokeStmt):
            self.exec_stroke(stmt, env)
        elif isinstance(stmt, RenderStmt):
            write_png(stmt.filename, self.cw, self.ch, self.buf)
            print(f'已渲染: {stmt.filename} ({self.cw}x{self.ch})')
        elif isinstance(stmt, ImportStmt):
            # v0.5 批次 B §3.2.14 模块导入
            self._do_import(stmt.path)
        elif isinstance(stmt, MaterialDef):
            # v0.5 批次 C §4.8 材质定义：求值字段并注册到 self.materials + env
            fields = {k: self.eval(v, env) for k, v in stmt.fields.items()}
            mat = MaterialInstance(stmt.name, fields)
            self.materials[stmt.name] = mat
            env.vars[stmt.name] = mat  # 允许 material: myMat 通过 VarRef 解析
        elif isinstance(stmt, LayerDef):
            # v0.5 批次 C §7.4 图层定义：在离屏缓冲区执行 body，
            # 完成后存入 self.layers，等待 compose() 合成到主画布
            self._exec_layer(stmt, env)
        elif isinstance(stmt, FieldDef):
            # v0.5 批次 C §7.5 颜色场定义：注册为 Closure（与 fn 等价）
            closure = Closure(stmt.name, stmt.params, stmt.body, env)
            env.vars[stmt.name] = closure
            self.funcs[stmt.name] = (stmt.params, stmt.body, env)
        elif isinstance(stmt, ExprStmt):
            self.eval(stmt.expr, env)

    def _do_import(self, path):
        """v0.5 批次 B §3.2.14 模块导入：读取并执行目标 .vgl 文件，
        将其顶层 fn/struct/let 定义注入当前全局环境。
        路径相对当前文件目录；按绝对路径去重，自动防止循环导入。"""
        full = os.path.abspath(os.path.join(self.current_dir or '.', path))
        if full in self.imported:
            return  # 已导入，跳过（去重 + 防循环）
        self.imported.add(full)
        try:
            with open(full, encoding='utf-8') as f:
                sub_src = f.read()
        except OSError as e:
            raise vgl_error(IOError, f'无法导入模块 {path}: {e}', self.current_pos)
        # 切换文件上下文（错误定位 + 嵌套 import 路径解析）
        old_dir, old_fn, old_src, old_pos = (
            self.current_dir, self.current_filename, self.current_src, self.current_pos)
        self.current_dir = os.path.dirname(full)
        self.current_filename = full
        self.current_src = sub_src
        try:
            sub_toks = tokenize(sub_src)
            sub_ast = Parser(sub_toks).parse_program()
            for s in sub_ast:
                self.exec(s, self.globals)
        except Exception:
            # 异常时保留子文件上下文（current_src/filename 为子文件），
            # 供 main 错误定位使用，不恢复到主文件
            raise
        # 正常完成，恢复主文件上下文
        self.current_dir = old_dir
        self.current_filename = old_fn
        self.current_src = old_src
        self.current_pos = old_pos

    def _exec_layer(self, stmt, env):
        """v0.5 批次 C §7.4 图层执行：分配离屏缓冲区，切换 cw/ch/buf 后
        执行 body，结束后恢复主画布缓冲区并存入 self.layers。"""
        layer_w = self.cw
        layer_h = self.ch
        layer_buf = bytearray(len(self.buf)) if self.buf else bytearray(layer_w * layer_h * 3)
        # 初始化为背景色（若未设 bg 则黑色）
        for i in range(0, len(layer_buf), 3):
            layer_buf[i] = self._bg_color[0] if hasattr(self, '_bg_color') else 0
            layer_buf[i + 1] = self._bg_color[1] if hasattr(self, '_bg_color') else 0
            layer_buf[i + 2] = self._bg_color[2] if hasattr(self, '_bg_color') else 0
        # 切换缓冲区
        old_cw, old_ch, old_buf = self.cw, self.ch, self.buf
        self.cw, self.ch, self.buf = layer_w, layer_h, layer_buf
        # 块作用域
        block_env = Environment(parent=env)
        for s in stmt.body:
            self.exec(s, block_env)
        # 恢复主画布
        self.cw, self.ch, self.buf = old_cw, old_ch, old_buf
        self.layers[stmt.name] = LayerInstance(stmt.name, layer_w, layer_h, layer_buf)

    def compose_layer(self, name, blend='over'):
        """v0.5 批次 C §7.4 compose(name, blend) 图层合成。
        blend 模式：'over'（默认，alpha 混合）/ 'add'（加法）/ 'mul'（乘法）/
        'screen'（滤色）。alpha 由图层像素与主画布像素的亮度差近似。"""
        if name not in self.layers:
            raise NameError(f'未定义图层: {name}')
        layer = self.layers[name]
        if layer.width != self.cw or layer.height != self.ch:
            raise ValueError(f'图层尺寸 {layer.width}x{layer.height} 与画布 {self.cw}x{self.ch} 不匹配')
        lb = layer.buf
        mb = self.buf
        for i in range(0, len(lb), 3):
            lr, lg, lb_ = lb[i], lb[i + 1], lb[i + 2]
            mr, mg, mb_ = mb[i], mb[i + 1], mb[i + 2]
            if blend == 'add':
                mb[i] = min(255, mr + lr) & 0xff
                mb[i + 1] = min(255, mg + lg) & 0xff
                mb[i + 2] = min(255, mb_ + lb_) & 0xff
            elif blend == 'mul':
                mb[i] = (mr * lr // 255) & 0xff
                mb[i + 1] = (mg * lg // 255) & 0xff
                mb[i + 2] = (mb_ * lb_ // 255) & 0xff
            elif blend == 'screen':
                mb[i] = (255 - ((255 - mr) * (255 - lr) // 255)) & 0xff
                mb[i + 1] = (255 - ((255 - mg) * (255 - lg) // 255)) & 0xff
                mb[i + 2] = (255 - ((255 - mb_) * (255 - lb_) // 255)) & 0xff
            else:  # 'over' — 简单覆盖（图层非透明像素覆盖主画布）
                # 用图层像素亮度作为 alpha 近似（更亮的像素更不透明）
                alpha = (lr + lg + lb_) / (3 * 255)
                mb[i] = int(mr * (1 - alpha) + lr * alpha) & 0xff
                mb[i + 1] = int(mg * (1 - alpha) + lg * alpha) & 0xff
                mb[i + 2] = int(mb_ * (1 - alpha) + lb_ * alpha) & 0xff

    def fill_field(self, name):
        """v0.5 批次 C §7.5 fill(name) 颜色场填充：对画布每个像素 (x, y)
        调用 field 函数，用返回的 color 写入。"""
        # 优先从 env 查 Closure（field 定义时已注册）
        target_env = self.globals.find_env(name)
        if target_env is not None and isinstance(target_env.vars[name], Closure):
            closure = target_env.vars[name]
        elif name in self.funcs:
            params, body, def_env = self.funcs[name]
            closure = Closure(name, params, body, def_env)
        else:
            raise NameError(f'未定义颜色场: {name}')
        for y in range(self.ch):
            for x in range(self.cw):
                color = self._invoke_closure(closure, [x, y], {})
                if isinstance(color, tuple) and len(color) >= 3:
                    self.put_pixel(x, y, (int(color[0]) & 0xff,
                                          int(color[1]) & 0xff,
                                          int(color[2]) & 0xff))

    def put_pixel(self, x, y, rgb):
        if 0 <= x < self.cw and 0 <= y < self.ch:
            idx = (y * self.cw + x) * 3
            self.buf[idx] = int(rgb[0]) & 0xff
            self.buf[idx + 1] = int(rgb[1]) & 0xff
            self.buf[idx + 2] = int(rgb[2]) & 0xff

    def put_pixel_aa(self, x, y, r, g, b, alpha):
        """v0.5 批次 B 抗锯齿像素：alpha∈[0,1]，与现有像素线性混合（§7.3 Wu）"""
        if alpha <= 0:
            return
        if 0 <= x < self.cw and 0 <= y < self.ch:
            idx = (y * self.cw + x) * 3
            a = max(0.0, min(1.0, alpha))
            self.buf[idx] = int(self.buf[idx] * (1 - a) + r * a) & 0xff
            self.buf[idx + 1] = int(self.buf[idx + 1] * (1 - a) + g * a) & 0xff
            self.buf[idx + 2] = int(self.buf[idx + 2] * (1 - a) + b * a) & 0xff

    def exec_stroke(self, stmt, env):
        # v0.4 块作用域：stroke 块创建子 Environment（§5.1，与 for/if 一致）
        block_env = Environment(parent=env)
        f = {k: self.eval(v, block_env) for k, v in stmt.fields.items()}
        path = f.get('path')
        width = int(f.get('width', 1))
        # v0.5 批次 C：material 字段优先（覆盖 color），支持 noise 扰动
        mat = f.get('material')
        if isinstance(mat, MaterialInstance):
            color = mat.fields.get('color', (0, 0, 0))
            noise = mat.fields.get('noise', 0.0)
            if noise:
                # 沿路径长度方向叠加 Perlin 扰动（影响亮度，非位移）
                n = perlin(f.get('width', 1.0) * 10, 0.0) * float(noise)
                color = (max(0, min(255, int(color[0] + n * 255))),
                         max(0, min(255, int(color[1] + n * 255))),
                         max(0, min(255, int(color[2] + n * 255))))
        else:
            color = f.get('color', (0, 0, 0))
        if not isinstance(color, tuple):
            color = (0, 0, 0)
        r, g, b = color[0], color[1], color[2]
        samples = int(f.get('samples', 0))  # 0 表示用默认采样数

        if path and path[0] == 'line':
            p1, p2 = path[1], path[2]
            self.draw_line(int(p1[0]), int(p1[1]), int(p2[0]), int(p2[1]), width, r, g, b)
        elif path and path[0] == 'circle':
            cx, cy, rad = int(path[1]), int(path[2]), int(path[3])
            self.draw_circle(cx, cy, rad, width, r, g, b)
        elif path and path[0] == 'bezier':
            # 三次贝塞尔 (§6.2)：de Casteljau 采样 N 个点，相邻点连线
            p1, p2, p3, p4 = path[1], path[2], path[3], path[4]
            n = samples if samples > 0 else 64
            pts = self._sample_bezier3(p1, p2, p3, p4, n)
            self._draw_polyline(pts, width, r, g, b)
        elif path and path[0] == 'qbezier':
            # 二次贝塞尔：de Casteljau 二次形式
            p1, p2, p3 = path[1], path[2], path[3]
            n = samples if samples > 0 else 32
            pts = self._sample_bezier2(p1, p2, p3, n)
            self._draw_polyline(pts, width, r, g, b)
        elif path and path[0] == 'polyline':
            # 折线：连接所有点
            points = path[1]
            self._draw_polyline(points, width, r, g, b)

    @staticmethod
    def _lerp(p1, p2, t):
        """线性插值两点"""
        return (p1[0] + (p2[0] - p1[0]) * t, p1[1] + (p2[1] - p1[1]) * t)

    def _sample_bezier2(self, p1, p2, p3, n):
        """二次贝塞尔 de Casteljau 采样"""
        pts = []
        for i in range(n + 1):
            t = i / n
            q0 = self._lerp(p1, p2, t)
            q1 = self._lerp(p2, p3, t)
            pts.append(self._lerp(q0, q1, t))
        return pts

    def _sample_bezier3(self, p1, p2, p3, p4, n):
        """三次贝塞尔 de Casteljau 采样"""
        pts = []
        for i in range(n + 1):
            t = i / n
            q0 = self._lerp(p1, p2, t)
            q1 = self._lerp(p2, p3, t)
            q2 = self._lerp(p3, p4, t)
            r0 = self._lerp(q0, q1, t)
            r1 = self._lerp(q1, q2, t)
            pts.append(self._lerp(r0, r1, t))
        return pts

    def _draw_polyline(self, points, width, r, g, b):
        """连接一系列点为折线段"""
        for i in range(len(points) - 1):
            p1, p2 = points[i], points[i + 1]
            self.draw_line(int(p1[0]), int(p1[1]), int(p2[0]), int(p2[1]), width, r, g, b)

    def draw_line(self, x0, y0, x1, y1, w, r, g, b):
        """v0.5 批次 B §7.3：w<=1 用 Xiaolin Wu 抗锯齿直线；
        w>1 用 Bresenham 主线 + 圆形笔刷（填充圆，粗线圆润）。"""
        if w <= 1:
            self._wu_line(x0, y0, x1, y1, r, g, b)
        else:
            for px, py in self._bresenham_points(x0, y0, x1, y1):
                self._brush(px, py, w, r, g, b)

    def draw_circle(self, cx, cy, rad, w, r, g, b):
        """v0.5 批次 B §7.3：中点圆算法画轮廓；w<=1 单像素，w>1 圆形笔刷。"""
        x, y, err = rad, 0, 0
        while x >= y:
            for px, py in [(cx + x, cy + y), (cx + y, cy + x), (cx - y, cy + x),
                           (cx - x, cy + y), (cx - x, cy - y), (cx - y, cy - x),
                           (cx + y, cy - x), (cx + x, cy - y)]:
                if w <= 1:
                    self.put_pixel(px, py, (r, g, b))
                else:
                    self._brush(px, py, w, r, g, b)
            y += 1
            if err <= 0:
                err += 2 * y + 1
            if err > 0:
                x -= 1
                err -= 2 * x + 1

    @staticmethod
    def _bresenham_points(x0, y0, x1, y1):
        """Bresenham 整数直线算法，返回点列表（§7.3）。"""
        points = []
        dx = abs(x1 - x0)
        dy = -abs(y1 - y0)
        sx = 1 if x0 < x1 else -1
        sy = 1 if y0 < y1 else -1
        err = dx + dy
        while True:
            points.append((x0, y0))
            if x0 == x1 and y0 == y1:
                break
            e2 = 2 * err
            if e2 >= dy:
                err += dy
                x0 += sx
            if e2 <= dx:
                err += dx
                y0 += sy
        return points

    def _brush(self, cx, cy, w, r, g, b):
        """以 (cx,cy) 为中心画直径 w 的填充圆笔刷（粗线圆润边缘）。"""
        rad = w / 2.0
        r2 = rad * rad
        ri = int(rad) + 1
        for dy in range(-ri, ri + 1):
            for dx in range(-ri, ri + 1):
                if dx * dx + dy * dy <= r2:
                    self.put_pixel(cx + dx, cy + dy, (r, g, b))

    def _wu_line(self, x0, y0, x1, y1, r, g, b):
        """Xiaolin Wu 抗锯齿直线算法（width=1，§7.3 反走样）。"""
        def ipart(x):
            return int(x)
        def fpart(x):
            return x - int(x)
        def rfpart(x):
            return 1 - fpart(x)
        steep = abs(y1 - y0) > abs(x1 - x0)
        if steep:
            x0, y0 = y0, x0
            x1, y1 = y1, x1
        if x0 > x1:
            x0, x1 = x1, x0
            y0, y1 = y1, y0
        dx = x1 - x0
        dy = y1 - y0
        grad = dy / dx if dx != 0 else 1.0
        # 第一个端点
        xend = round(x0)
        yend = y0 + grad * (xend - x0)
        xgap = rfpart(x0 + 0.5)
        xpxl1 = int(xend)
        ypxl1 = ipart(yend)
        if steep:
            self.put_pixel_aa(ypxl1, xpxl1, r, g, b, rfpart(yend) * xgap)
            self.put_pixel_aa(ypxl1 + 1, xpxl1, r, g, b, fpart(yend) * xgap)
        else:
            self.put_pixel_aa(xpxl1, ypxl1, r, g, b, rfpart(yend) * xgap)
            self.put_pixel_aa(xpxl1, ypxl1 + 1, r, g, b, fpart(yend) * xgap)
        intery = yend + grad
        # 第二个端点
        xend = round(x1)
        yend = y1 + grad * (xend - x1)
        xgap = fpart(x1 + 0.5)
        xpxl2 = int(xend)
        ypxl2 = ipart(yend)
        if steep:
            self.put_pixel_aa(ypxl2, xpxl2, r, g, b, rfpart(yend) * xgap)
            self.put_pixel_aa(ypxl2 + 1, xpxl2, r, g, b, fpart(yend) * xgap)
        else:
            self.put_pixel_aa(xpxl2, ypxl2, r, g, b, rfpart(yend) * xgap)
            self.put_pixel_aa(xpxl2, ypxl2 + 1, r, g, b, fpart(yend) * xgap)
        # 主循环
        if steep:
            for x in range(xpxl1 + 1, xpxl2):
                self.put_pixel_aa(ipart(intery), x, r, g, b, rfpart(intery))
                self.put_pixel_aa(ipart(intery) + 1, x, r, g, b, fpart(intery))
                intery += grad
        else:
            for x in range(xpxl1 + 1, xpxl2):
                self.put_pixel_aa(x, ipart(intery), r, g, b, rfpart(intery))
                self.put_pixel_aa(x, ipart(intery) + 1, r, g, b, fpart(intery))
                intery += grad

    # --- 表达式求值 ---

    def eval(self, expr, env):
        if isinstance(expr, Num):
            return expr.value
        if isinstance(expr, Str):
            return expr.value
        if isinstance(expr, ColorLit):
            return (expr.r, expr.g, expr.b)
        if isinstance(expr, VarRef):
            # v0.3: 沿词法作用域链查找（§5.2）
            target = env.find_env(expr.name)
            if target is None:
                raise NameError(f'未定义变量: {expr.name}')
            return target.vars[expr.name]
        if isinstance(expr, TupleLit):
            return tuple(self.eval(e, env) for e in expr.elements)
        if isinstance(expr, ArrayLit):
            # v0.5 数组字面量（Python list，可变）
            return [self.eval(e, env) for e in expr.elements]
        if isinstance(expr, BinOp):
            l = self.eval(expr.left, env)
            r = self.eval(expr.right, env)
            # v0.3 §4.3 元组广播运算
            if expr.op in ('+', '-', '*', '/'):
                result = self._binop_arith(expr.op, l, r)
                if result is not None:
                    return result
                # 落到此处说明非法组合
            if expr.op == '+': return l + r
            if expr.op == '-': return l - r
            if expr.op == '*': return l * r
            if expr.op == '/': return l / r
            # v0.3 比较运算符（返回 bool）
            if expr.op == '<': return l < r
            if expr.op == '>': return l > r
            if expr.op == '<=': return l <= r
            if expr.op == '>=': return l >= r
            if expr.op == '==': return l == r
            if expr.op == '!=': return l != r
        if isinstance(expr, BoolLit):
            return expr.value
        if isinstance(expr, LogicOp):
            # v0.3 短路求值：左操作数先求值，按需求值右操作数
            l = self.eval(expr.left, env)
            if expr.op == 'and':
                if not self.truthy(l):
                    return False
                return bool(self.truthy(self.eval(expr.right, env)))
            if expr.op == 'or':
                if self.truthy(l):
                    return True
                return bool(self.truthy(self.eval(expr.right, env)))
        if isinstance(expr, UnaryNot):
            return not self.truthy(self.eval(expr.expr, env))
        if isinstance(expr, IndexExpr):
            # v0.5: tuple[i] / array[i] / dict[k] / str[i] 通用索引
            base = self.eval(expr.base, env)
            idx = self.eval(expr.index, env)
            if isinstance(base, (tuple, list, str)):
                if not isinstance(idx, (int, float)) or isinstance(idx, bool):
                    raise TypeError(f'索引必须为整数, 得到 {type(idx).__name__}')
                idx = int(idx)
                if idx < 0 or idx >= len(base):
                    raise IndexError(f'索引越界: {idx} (长度 {len(base)})')
                return base[idx]
            if isinstance(base, dict):
                if idx not in base:
                    raise KeyError(f'dict 无键: {idx!r}')
                return base[idx]
            raise TypeError(f'不支持索引的类型: {type(base).__name__}')
        if isinstance(expr, FieldAccess):
            # v0.5 struct 字段访问
            obj = self.eval(expr.obj, env)
            if not isinstance(obj, StructInstance):
                raise TypeError(f'字段访问要求 struct 实例, 得到 {type(obj).__name__}')
            if expr.name not in obj.fields:
                raise AttributeError(f'struct {obj.struct_name} 无字段 {expr.name}')
            return obj.fields[expr.name]
        if isinstance(expr, Call):
            name = expr.name
            arg_vals = [self.eval(a, env) for a in expr.args]
            kw_vals = {k: self.eval(v, env) for k, v in expr.kwargs.items()}
            # v0.5 struct 构造：name 是已注册的 struct 类型
            if name in self.structs:
                return self._construct_struct(self.structs[name], arg_vals, kw_vals)
            # v0.3 优先：沿词法作用域链查找闭包（支持 let walk = ...; walk()）
            target_env = env.find_env(name)
            if target_env is not None and isinstance(target_env.vars[name], Closure):
                closure = target_env.vars[name]
                return self._invoke_closure(closure, arg_vals, kw_vals)
            # 兼容路径：顶层 fn 注册的 self.funcs（元组形式 params/body/def_env）
            if name in self.funcs:
                params, body, def_env = self.funcs[name]
                closure = Closure(name, params, body, def_env)
                return self._invoke_closure(closure, arg_vals, kw_vals)
            # 内建函数
            if name in self.builtins:
                return self.builtins[name](*arg_vals)
            raise NameError(f'未定义函数: {name}')
        raise RuntimeError(f'未知表达式类型: {type(expr).__name__}')

    def _invoke_closure(self, closure, arg_vals, kw_vals):
        """调用闭包：创建新 call_env，parent 指向闭包的 def_env（§5.3）。
        参数绑定后执行函数体，捕获 ReturnSignal 返回值。"""
        call_env = Environment(parent=closure.def_env)
        for i, p in enumerate(closure.params):
            if i < len(arg_vals):
                call_env.vars[p] = arg_vals[i]
        for k, v in kw_vals.items():
            call_env.vars[k] = v
        try:
            for s in closure.body:
                self.exec(s, call_env)
        except ReturnSignal as ret:
            return ret.value
        return None

    def _construct_struct(self, struct_defn, arg_vals, kw_vals):
        """v0.5 struct 构造：先按字段定义顺序填充默认值，再用位置参数
        和关键字参数覆盖。位置参数按字段定义顺序对应。"""
        fields = {}
        # 1. 默认值
        for fname, default in struct_defn.fields:
            fields[fname] = default
        # 2. 位置参数覆盖（按定义顺序）
        for i, val in enumerate(arg_vals):
            if i >= len(struct_defn.fields):
                raise TypeError(f'struct {struct_defn.name} 字段数 {len(struct_defn.fields)}, 得到 {len(arg_vals)} 个位置参数')
            fields[struct_defn.fields[i][0]] = val
        # 3. 关键字参数覆盖
        for k, v in kw_vals.items():
            if k not in fields:
                raise TypeError(f'struct {struct_defn.name} 无字段 {k}')
            fields[k] = v
        return StructInstance(struct_defn.name, fields)


# ============================================================
# 主入口
# ============================================================

def main():
    if len(sys.argv) < 2:
        print('用法: python vgl.py <file.vgl>')
        sys.exit(1)
    filename = sys.argv[1]
    with open(filename, encoding='utf-8') as f:
        src = f.read()
    interp = Interpreter()
    interp.current_dir = os.path.dirname(os.path.abspath(filename))
    interp.current_filename = filename
    interp.current_src = src
    interp.imported.add(os.path.abspath(filename))
    try:
        toks = tokenize(src)
        ast = Parser(toks).parse_program()
        interp.run(ast)
    except (BreakSignal, ContinueSignal, ReturnSignal):
        # 控制流信号不应泄漏到顶层（解析期已拦截 break/continue 循环外用法）
        print(f'{filename}: 控制流信号泄漏到顶层（不应发生）', file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        pos = getattr(e, 'vgl_pos', None)
        if pos is None:
            pos = interp.current_pos
        # 用当前文件上下文（可能是 import 的子文件）定位错误
        print('VGL 错误: ' + format_error(str(e), interp.current_src, pos,
                                          interp.current_filename), file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
