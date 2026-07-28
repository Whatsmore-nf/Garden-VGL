#include "vgl.hpp"
#include "lexer.hpp"
#include "parser.hpp"
#include "canvas.hpp"
#include <iostream>
#include <fstream>
#include <sstream>
#include <memory>
#include <random>
#include <unordered_map>
#include <cmath>

// Simple interpreter implementation inline for main.cpp
namespace vgl {

struct Value {
    enum Type { None, Number, String, Color, Tuple, Array } type;
    double number = 0.0;
    std::string str;
    std::tuple<uint8_t, uint8_t, uint8_t, uint8_t> color;
    std::vector<Value> elements;
    
    Value() : type(None) {}
    Value(double n) : type(Number), number(n) {}
    Value(const std::string& s) : type(String), str(s) {}
    Value(uint8_t r, uint8_t g, uint8_t b, uint8_t a) : type(Color), color{r,g,b,a} {}
};

class Environment {
public:
    std::unordered_map<std::string, Value> vars;
    std::shared_ptr<Environment> parent;
    
    Environment(std::shared_ptr<Environment> p = nullptr) : parent(p) {}
    
    Value get(const std::string& name) {
        auto it = vars.find(name);
        if (it != vars.end()) return it->second;
        if (parent) return parent->get(name);
        throw VglError("Undefined variable: " + name, 0);
    }
    
    void define(const std::string& name, const Value& val) { vars[name] = val; }
};

class Interpreter {
public:
    std::shared_ptr<Environment> global_env;
    Canvas canvas;
    std::mt19937 rng;
    std::string current_filename;
    std::string current_src;
    
    Interpreter() : global_env(std::make_shared<Environment>()) { rng.seed(42); }
    
    void exec(const std::vector<StmtPtr>& stmts) {
        for (const auto& stmt : stmts) exec_stmt(stmt);
    }
    
private:
    void exec_stmt(StmtPtr stmt);
    Value eval(ExprPtr expr);
};

void Interpreter::exec_stmt(StmtPtr stmt) {
    if (auto s = std::dynamic_pointer_cast<CanvasStmt>(stmt)) {
        canvas = Canvas(s->width, s->height);
    }
    else if (auto s = std::dynamic_pointer_cast<BgStmt>(stmt)) {
        Value v = eval(s->color);
        if (v.type == Value::Color) {
            canvas.clear(std::get<0>(v.color), std::get<1>(v.color),
                        std::get<2>(v.color), std::get<3>(v.color));
        }
    }
    else if (auto s = std::dynamic_pointer_cast<LetStmt>(stmt)) {
        global_env->define(s->name, eval(s->value));
    }
    else if (auto s = std::dynamic_pointer_cast<ForStmt>(stmt)) {
        if (s->end) {
            double start = eval(s->start).number;
            double end = eval(s->end).number;
            double step = s->step ? eval(s->step).number : 1.0;
            for (double i = start; i < end; i += step) {
                global_env->define(s->var, Value(i));
                for (const auto& b : s->body) exec_stmt(b);
            }
        }
    }
    else if (auto s = std::dynamic_pointer_cast<IfStmt>(stmt)) {
        if (eval(s->cond).number != 0.0) {
            for (const auto& b : s->then_body) exec_stmt(b);
        } else if (s->else_body) {
            for (const auto& b : *s->else_body) exec_stmt(b);
        }
    }
    else if (auto s = std::dynamic_pointer_cast<SeedStmt>(stmt)) {
        rng.seed(s->seed);
    }
    else if (auto s = std::dynamic_pointer_cast<PixelStmt>(stmt)) {
        int x = static_cast<int>(eval(s->x).number);
        int y = static_cast<int>(eval(s->y).number);
        Value rgb = eval(s->rgb);
        if (rgb.type == Value::Color) {
            canvas.set_pixel(x, y, std::get<0>(rgb.color), std::get<1>(rgb.color),
                            std::get<2>(rgb.color), std::get<3>(rgb.color));
        } else if (rgb.type == Value::Tuple && rgb.elements.size() >= 3) {
            uint8_t r = static_cast<uint8_t>(rgb.elements[0].number);
            uint8_t g = static_cast<uint8_t>(rgb.elements[1].number);
            uint8_t b = static_cast<uint8_t>(rgb.elements[2].number);
            uint8_t a = rgb.elements.size() >= 4 ? static_cast<uint8_t>(rgb.elements[3].number) : 255;
            canvas.set_pixel(x, y, r, g, b, a);
        }
    }
    else if (auto s = std::dynamic_pointer_cast<RenderStmt>(stmt)) {
        if (canvas.save_png(s->filename)) {
            std::cout << "Rendered: " << s->filename << " (" 
                     << canvas.width << "x" << canvas.height << ")" << std::endl;
        } else {
            std::cerr << "Failed to save: " << s->filename << std::endl;
        }
    }
    else if (auto s = std::dynamic_pointer_cast<ExprStmt>(stmt)) {
        eval(s->expr);
    }
}

Value Interpreter::eval(ExprPtr expr) {
    if (auto e = std::dynamic_pointer_cast<NumberExpr>(expr)) return Value(e->value);
    if (auto e = std::dynamic_pointer_cast<ColorExpr>(expr)) return Value(e->r, e->g, e->b, e->a);
    if (auto e = std::dynamic_pointer_cast<IdentExpr>(expr)) return global_env->get(e->name);
    
    if (auto e = std::dynamic_pointer_cast<BinaryExpr>(expr)) {
        Value l = eval(e->left), r = eval(e->right);
        if (e->op == "+") return Value(l.number + r.number);
        if (e->op == "-") return Value(l.number - r.number);
        if (e->op == "*") return Value(l.number * r.number);
        if (e->op == "/") return Value(r.number != 0 ? l.number / r.number : 0);
        if (e->op == "<") return Value(l.number < r.number ? 1.0 : 0.0);
        if (e->op == ">") return Value(l.number > r.number ? 1.0 : 0.0);
    }
    
    if (auto e = std::dynamic_pointer_cast<UnaryExpr>(expr)) {
        Value v = eval(e->operand);
        if (e->op == "-") return Value(-v.number);
        if (e->op == "!") return Value(v.number == 0 ? 1.0 : 0.0);
    }
    
    if (auto e = std::dynamic_pointer_cast<CallExpr>(expr)) {
        std::vector<Value> args;
        for (const auto& a : e->args) args.push_back(eval(a));
        
        if (e->callee == "rand" && args.size() >= 2) {
            std::uniform_real_distribution<> dist(args[0].number, args[1].number);
            return Value(dist(rng));
        }
        if (e->callee == "int" && !args.empty()) 
            return Value(static_cast<double>(static_cast<int>(args[0].number)));
        if (e->callee == "cos" && !args.empty()) return Value(std::cos(args[0].number));
        if (e->callee == "sin" && !args.empty()) return Value(std::sin(args[0].number));
    }
    
    if (auto e = std::dynamic_pointer_cast<TupleExpr>(expr)) {
        Value v; v.type = Value::Tuple;
        for (const auto& elem : e->elements) v.elements.push_back(eval(elem));
        return v;
    }
    
    return Value(0.0);
}

} // namespace vgl

void print_usage() {
    std::cerr << "VGL v1.0 C++ Usage:\n";
    std::cerr << "  vgl <file.vgl>\n";
}

int main(int argc, char* argv[]) {
    if (argc < 2) {
        print_usage();
        return 1;
    }
    
    std::string filename = argv[1];
    std::ifstream file(filename);
    if (!file) {
        std::cerr << "Cannot open file: " << filename << std::endl;
        return 1;
    }
    
    std::stringstream buffer;
    buffer << file.rdbuf();
    std::string src = buffer.str();
    
    try {
        vgl::Lexer lexer(src);
        auto tokens = lexer.tokenize();
        
        vgl::Parser parser(tokens);
        auto ast = parser.parse_program();
        
        vgl::Interpreter interp;
        interp.current_filename = filename;
        interp.current_src = src;
        interp.exec(ast);
        
    } catch (const vgl::VglError& e) {
        std::cerr << vgl::format_error(e.msg, src, e.pos, filename);
        return 1;
    }
    
    return 0;
}
