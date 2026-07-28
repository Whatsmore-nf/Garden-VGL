#ifndef VGL_HPP
#define VGL_HPP

#include <string>
#include <vector>
#include <map>
#include <variant>
#include <memory>
#include <optional>
#include <functional>
#include <cmath>
#include <stdexcept>
#include <sstream>
#include <iomanip>

namespace vgl {

// ==================== 基础类型 ====================

// Token 类型
enum class TokenType {
    Number,
    String,
    Color,
    Ident,
    Keyword,
    LParen, RParen,
    LBrace, RBrace,
    LBracket, RBracket,
    Comma, Colon, Dot, DotDot,
    Op,
    Eof
};

struct Token {
    TokenType type;
    std::variant<
        double,                     // Number
        std::string,                // String
        std::tuple<uint8_t,uint8_t,uint8_t,uint8_t>, // Color (RGBA)
        std::string                 // Ident/Keyword/Op
    > value;
    size_t pos;
    
    Token(TokenType t = TokenType::Eof, size_t p = 0) : type(t), pos(p) {}
    
    // Helper to set string value for String type
    void setString(const std::string& s) {
        if (type == TokenType::String) {
            value = s;
        } else {
            value = s;  // For Ident/Keyword/Op
        }
    }
};

// ==================== 错误处理 ====================

class VglError : public std::runtime_error {
public:
    std::string msg;
    size_t pos;
    std::string filename;
    
    VglError(const std::string& m, size_t p = 0, const std::string& f = "")
        : std::runtime_error(m), msg(m), pos(p), filename(f) {}
};

class VglWarning {
public:
    std::string msg;
    size_t pos;
    std::string filename;
    
    VglWarning(const std::string& m, size_t p = 0, const std::string& f = "")
        : msg(m), pos(p), filename(f) {}
};

std::string format_error(const std::string& msg, const std::string& src, 
                         size_t pos, const std::string& filename);

// ==================== AST 节点 ====================

struct Expr;
struct Stmt;

using ExprPtr = std::shared_ptr<Expr>;
using StmtPtr = std::shared_ptr<Stmt>;

// 表达式类型
struct Expr {
    virtual ~Expr() = default;
    size_t pos = 0;
};

struct NumberExpr : Expr {
    double value;
    NumberExpr(double v, size_t p) : value(v) { pos = p; }
};

struct StringExpr : Expr {
    std::string value;
    StringExpr(const std::string& v, size_t p) : value(v) { pos = p; }
};

struct ColorExpr : Expr {
    uint8_t r, g, b, a;
    ColorExpr(uint8_t r_, uint8_t g_, uint8_t b_, uint8_t a_, size_t p)
        : r(r_), g(g_), b(b_), a(a_) { pos = p; }
};

struct IdentExpr : Expr {
    std::string name;
    IdentExpr(const std::string& n, size_t p) : name(n) { pos = p; }
};

struct BinaryExpr : Expr {
    ExprPtr left, right;
    std::string op;
    BinaryExpr(ExprPtr l, const std::string& o, ExprPtr r, size_t p)
        : left(l), right(r), op(o) { pos = p; }
};

struct UnaryExpr : Expr {
    ExprPtr operand;
    std::string op;
    UnaryExpr(const std::string& o, ExprPtr op_, size_t p)
        : operand(op_), op(o) { pos = p; }
};

struct CallExpr : Expr {
    std::string callee;
    std::vector<ExprPtr> args;
    std::map<std::string, ExprPtr> kwargs;
    CallExpr(const std::string& c, size_t p) : callee(c) { pos = p; }
};

struct IndexExpr : Expr {
    ExprPtr object, index;
    IndexExpr(ExprPtr obj, ExprPtr idx, size_t p)
        : object(obj), index(idx) { pos = p; }
};

struct FieldExpr : Expr {
    ExprPtr object;
    std::string field;
    FieldExpr(ExprPtr obj, const std::string& f, size_t p)
        : object(obj), field(f) { pos = p; }
};

struct TupleExpr : Expr {
    std::vector<ExprPtr> elements;
    TupleExpr(size_t p) { pos = p; }
};

struct ArrayExpr : Expr {
    std::vector<ExprPtr> elements;
    ArrayExpr(size_t p) { pos = p; }
};

// 语句类型
struct Stmt {
    virtual ~Stmt() = default;
    size_t pos = 0;
};

struct CanvasStmt : Stmt {
    uint32_t width, height;
    CanvasStmt(uint32_t w, uint32_t h, size_t p) : width(w), height(h) { pos = p; }
};

struct BgStmt : Stmt {
    ExprPtr color;
    BgStmt(ExprPtr c, size_t p) : color(c) { pos = p; }
};

struct LetStmt : Stmt {
    std::string name;
    ExprPtr value;
    LetStmt(const std::string& n, ExprPtr v, size_t p) : name(n), value(v) { pos = p; }
};

struct ForStmt : Stmt {
    std::string var;
    ExprPtr start, end, step;
    std::vector<StmtPtr> body;
    ForStmt(const std::string& v, ExprPtr s, ExprPtr e, ExprPtr st, size_t p)
        : var(v), start(s), end(e), step(st) { pos = p; }
};

struct WhileStmt : Stmt {
    ExprPtr cond;
    std::vector<StmtPtr> body;
    WhileStmt(ExprPtr c, size_t p) : cond(c) { pos = p; }
};

struct IfStmt : Stmt {
    ExprPtr cond;
    std::vector<StmtPtr> then_body;
    std::optional<std::vector<StmtPtr>> else_body;
    IfStmt(ExprPtr c, size_t p) : cond(c) { pos = p; }
};

struct FnDefStmt : Stmt {
    std::string name;
    std::vector<std::pair<std::string, ExprPtr>> params; // name, default
    std::vector<StmtPtr> body;
    FnDefStmt(const std::string& n, size_t p) : name(n) { pos = p; }
};

struct ReturnStmt : Stmt {
    ExprPtr value;
    ReturnStmt(ExprPtr v, size_t p) : value(v) { pos = p; }
};

struct PixelStmt : Stmt {
    ExprPtr x, y, rgb;
    PixelStmt(ExprPtr x_, ExprPtr y_, ExprPtr rgb_, size_t p)
        : x(x_), y(y_), rgb(rgb_) { pos = p; }
};

struct StrokeStmt : Stmt {
    std::map<std::string, ExprPtr> fields;
    StrokeStmt(size_t p) { pos = p; }
};

struct RenderStmt : Stmt {
    std::string filename;
    RenderStmt(const std::string& f, size_t p) : filename(f) { pos = p; }
};

struct ImportStmt : Stmt {
    std::string path;
    ImportStmt(const std::string& p, size_t ph) : path(p) { pos = ph; }
};

struct ExprStmt : Stmt {
    ExprPtr expr;
    ExprStmt(ExprPtr e, size_t p) : expr(e) { pos = p; }
};

struct BreakStmt : Stmt {
    std::optional<std::string> label;
    BreakStmt(size_t p) { pos = p; }
};

struct ContinueStmt : Stmt {
    ContinueStmt(size_t p) { pos = p; }
};

struct SeedStmt : Stmt {
    uint64_t seed;
    SeedStmt(uint64_t s, size_t p) : seed(s) { pos = p; }
};

} // namespace vgl

#endif // VGL_HPP
