#!/usr/bin/env python3
"""
VGL 最小解释器 — 单文件，仅依赖标准库
支持: canvas / bg / let / for / if / fn / return / pixel / stroke / render
      v0.3: while / break / seed / 比较 < > <= >= == != / 逻辑 and or not / bool
      表达式: + - * /  元组  变量  函数调用  颜色字面量 #rgb  true/false
      内建函数: rand(a,b)  int(x)  abs(x)  floor(x)  ceil(x)  sin(x)  cos(x)
                min(a,b)  max(a,b)  bool(x)
                line(p1,p2)  circle(cx,cy,r)
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
            'while', 'break', 'and', 'or', 'not', 'seed', 'true', 'false'}


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
                raise SyntaxError(f'非法颜色 #{h} 于位置 {i}')
            i = j
            continue
        # 字符串
        if c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 1
            toks.append(Token('STRING', src[i + 1:j], i))
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
        raise SyntaxError(f'非法字符 {c!r} 于位置 {i}')
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
    def __init__(self, var, start, end, body):
        self.var, self.start, self.end, self.body = var, start, end, body
class IfStmt:
    def __init__(self, cond, then_body, else_body):
        self.cond, self.then_body, self.else_body = cond, then_body, else_body
class WhileStmt:
    def __init__(self, cond, body): self.cond, self.body = cond, body
class BreakStmt:
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
            raise SyntaxError(f'期望 {ttype} {val}, 得到 {t.type} {t.value!r} 于位置 {t.pos}')
        return self.advance()

    def parse_program(self):
        stmts = []
        while self.peek().type != 'EOF':
            stmts.append(self.parse_stmt())
        return stmts

    def parse_stmt(self):
        t = self.peek()
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
            }
            if t.value in dispatch:
                return dispatch[t.value]()
        # 裸赋值语句: IDENT '=' Expr  （需与 ExprStmt 区分：看 IDENT 后是否紧跟 '='）
        # 注意: '==' 是比较运算符，已在词法层合并为单个 OP token，不会误判
        if t.type == 'IDENT' and self.peek(1).type == 'OP' and self.peek(1).value == '=':
            return self.parse_assign()
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
        raise SyntaxError(f'期望颜色, 得到 {t.type} 于位置 {t.pos}')

    def parse_let(self):
        self.expect('KEYWORD', 'let')
        name = self.expect('IDENT').value
        self.expect('OP', '=')
        return LetStmt(name, self.parse_expr())

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
        return BreakStmt()

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

    def parse_kwargs(self):
        """解析 key: val, key: val, ... 形式"""
        fields = {}
        while self.peek().type != 'RPAREN' and self.peek().type != 'RBRACE':
            key = self.expect('IDENT').value
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
        if t.type == 'NUMBER':
            self.advance()
            return Num(t.value)
        if t.type == 'STRING':
            self.advance()
            return Str(t.value)
        if t.type == 'COLOR':
            self.advance()
            return ColorLit(t.value[0], t.value[1], t.value[2])
        if t.type == 'KEYWORD' and t.value in ('true', 'false'):
            self.advance()
            return BoolLit(t.value == 'true')
        if t.type == 'LPAREN':
            self.advance()
            first = self.parse_expr()
            if self.peek().type == 'COMMA':  # 元组
                el = [first]
                while self.peek().type == 'COMMA':
                    self.advance()
                    el.append(self.parse_expr())
                self.expect('RPAREN')
                return TupleLit(el)
            self.expect('RPAREN')
            return first
        if t.type == 'IDENT':
            self.advance()
            name = t.value
            if self.peek().type == 'LPAREN':  # 函数调用
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
                return Call(name, args, kwargs)
            return VarRef(name)
        raise SyntaxError(f'意外标记 {t.type} {t.value!r} 于位置 {t.pos}')


# ============================================================
# 解释器（树遍历）
# ============================================================

class ReturnSignal(Exception):
    def __init__(self, val): self.value = val


class BreakSignal(Exception):
    """break 语句的信号，由 for/while 循环体捕获"""
    pass


def _bool_fn(x):
    """bool(x) 内建: 0/0.0/false → False, 否则 True (§6.1, §4.2)"""
    if isinstance(x, bool):
        return x
    if isinstance(x, (int, float)):
        return x != 0
    return bool(x)


class Interpreter:
    def __init__(self):
        self.cw = 0
        self.ch = 0
        self.buf = None  # bytearray
        self.globals = {}
        self.funcs = {}
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
            'line': lambda p1, p2: ('line', p1, p2),
            'circle': lambda cx, cy, r: ('circle', cx, cy, r),
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

    # --- 语句执行 ---

    def exec(self, stmt, env):
        if isinstance(stmt, CanvasStmt):
            self.cw, self.ch = stmt.width, stmt.height
            self.buf = bytearray(self.cw * self.ch * 3)
        elif isinstance(stmt, BgStmt):
            r, g, b = self.eval(stmt.color, env)
            for i in range(0, len(self.buf), 3):
                self.buf[i] = r; self.buf[i + 1] = g; self.buf[i + 2] = b
        elif isinstance(stmt, LetStmt):
            env[stmt.name] = self.eval(stmt.expr, env)
        elif isinstance(stmt, AssignStmt):
            # 裸赋值（§3.2.3）：仅修改已存在绑定，不创建新绑定
            # 当前实现为单 env 模型（无作用域链），先查 env 再查 globals
            if stmt.name in env:
                env[stmt.name] = self.eval(stmt.expr, env)
            elif stmt.name in self.globals:
                self.globals[stmt.name] = self.eval(stmt.expr, env)
            else:
                raise NameError(f'赋值给未声明变量: {stmt.name}（应使用 let 声明）')
        elif isinstance(stmt, ForStmt):
            start = self.eval(stmt.start, env)
            end = self.eval(stmt.end, env)
            i = start
            while i < end:
                env[stmt.var] = i
                try:
                    for s in stmt.body:
                        self.exec(s, env)
                except BreakSignal:
                    break
                i += 1
        elif isinstance(stmt, WhileStmt):
            while self.truthy(self.eval(stmt.cond, env)):
                try:
                    for s in stmt.body:
                        self.exec(s, env)
                except BreakSignal:
                    break
        elif isinstance(stmt, BreakStmt):
            raise BreakSignal()
        elif isinstance(stmt, SeedStmt):
            random.seed(stmt.n)
        elif isinstance(stmt, IfStmt):
            if self.truthy(self.eval(stmt.cond, env)):
                for s in stmt.then_body:
                    self.exec(s, env)
            elif stmt.else_body:
                for s in stmt.else_body:
                    self.exec(s, env)
        elif isinstance(stmt, FnDef):
            self.funcs[stmt.name] = (stmt.params, stmt.body)
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
        elif isinstance(stmt, ExprStmt):
            self.eval(stmt.expr, env)

    def put_pixel(self, x, y, rgb):
        if 0 <= x < self.cw and 0 <= y < self.ch:
            idx = (y * self.cw + x) * 3
            self.buf[idx] = int(rgb[0]) & 0xff
            self.buf[idx + 1] = int(rgb[1]) & 0xff
            self.buf[idx + 2] = int(rgb[2]) & 0xff

    def exec_stroke(self, stmt, env):
        f = {k: self.eval(v, env) for k, v in stmt.fields.items()}
        path = f.get('path')
        width = int(f.get('width', 1))
        color = f.get('color', (0, 0, 0))
        if not isinstance(color, tuple):
            color = (0, 0, 0)
        r, g, b = color[0], color[1], color[2]

        if path and path[0] == 'line':
            p1, p2 = path[1], path[2]
            self.draw_line(int(p1[0]), int(p1[1]), int(p2[0]), int(p2[1]), width, r, g, b)
        elif path and path[0] == 'circle':
            cx, cy, rad = int(path[1]), int(path[2]), int(path[3])
            self.draw_circle(cx, cy, rad, width, r, g, b)

    def draw_line(self, x0, y0, x1, y1, w, r, g, b):
        dx, dy = abs(x1 - x0), abs(y1 - y0)
        length = max(dx, dy, 1)
        half = w // 2
        for i in range(length + 1):
            t = i / length
            x = int(x0 + (x1 - x0) * t)
            y = int(y0 + (y1 - y0) * t)
            for ox in range(-half, half + 1):
                for oy in range(-half, half + 1):
                    self.put_pixel(x + ox, y + oy, (r, g, b))

    def draw_circle(self, cx, cy, rad, w, r, g, b):
        x, y, err = rad, 0, 0
        half = w // 2
        while x >= y:
            for ox in range(-half, half + 1):
                for oy in range(-half, half + 1):
                    for px, py in [(cx + x, cy + y), (cx + y, cy + x), (cx - y, cy + x),
                                   (cx - x, cy + y), (cx - x, cy - y), (cx - y, cy - x),
                                   (cx + y, cy - x), (cx + x, cy - y)]:
                        self.put_pixel(px + ox, py + oy, (r, g, b))
            y += 1
            if err <= 0:
                err += 2 * y + 1
            if err > 0:
                x -= 1
                err -= 2 * x + 1

    # --- 表达式求值 ---

    def eval(self, expr, env):
        if isinstance(expr, Num):
            return expr.value
        if isinstance(expr, Str):
            return expr.value
        if isinstance(expr, ColorLit):
            return (expr.r, expr.g, expr.b)
        if isinstance(expr, VarRef):
            if expr.name in env:
                return env[expr.name]
            if expr.name in self.globals:
                return self.globals[expr.name]
            raise NameError(f'未定义变量: {expr.name}')
        if isinstance(expr, TupleLit):
            return tuple(self.eval(e, env) for e in expr.elements)
        if isinstance(expr, BinOp):
            l = self.eval(expr.left, env)
            r = self.eval(expr.right, env)
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
        if isinstance(expr, Call):
            name = expr.name
            arg_vals = [self.eval(a, env) for a in expr.args]
            kw_vals = {k: self.eval(v, env) for k, v in expr.kwargs.items()}
            # 用户函数
            if name in self.funcs:
                params, body = self.funcs[name]
                call_env = {}
                for i, p in enumerate(params):
                    if i < len(arg_vals):
                        call_env[p] = arg_vals[i]
                call_env.update(kw_vals)
                try:
                    for s in body:
                        self.exec(s, call_env)
                except ReturnSignal as ret:
                    return ret.value
                return None
            # 内建函数
            if name in self.builtins:
                return self.builtins[name](*arg_vals)
            raise NameError(f'未定义函数: {name}')
        raise RuntimeError(f'未知表达式类型: {type(expr).__name__}')


# ============================================================
# 主入口
# ============================================================

def main():
    if len(sys.argv) < 2:
        print('用法: python vgl.py <file.vgl>')
        sys.exit(1)
    with open(sys.argv[1], encoding='utf-8') as f:
        src = f.read()
    toks = tokenize(src)
    ast = Parser(toks).parse_program()
    Interpreter().run(ast)


if __name__ == '__main__':
    main()
