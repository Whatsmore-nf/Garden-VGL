#include "vgl.hpp"
#include "canvas.hpp"
#include <iostream>
#include <random>
#include <unordered_map>
#include <memory>
#include <cmath>

namespace vgl {

// Runtime value
struct Value {
    enum Type { None, Number, String, Color, Tuple, Array, Function, CanvasRef } type;
    double number = 0.0;
    std::string str;
    std::tuple<uint8_t, uint8_t, uint8_t, uint8_t> color;
    std::vector<Value> elements;
    size_t pos = 0;
    
    Value() : type(None) {}
    Value(double n) : type(Number), number(n) {}
    Value(const std::string& s) : type(String), str(s) {}
    Value(uint8_t r, uint8_t g, uint8_t b, uint8_t a) : type(Color), color(r,g,b,a) {}
};

// Environment for variable bindings
class Environment {
public:
    std::unordered_map<std::string, Value> vars;
    std::shared_ptr<Environment> parent;
    
    Environment(std::shared_ptr<Environment> p = nullptr) : parent(p) {}
    
    Value get(const std::string& name) {
        if (vars.count(name)) return vars[name];
        if (parent) return parent->get(name);
        throw VglError("Undefined variable: " + name, 0);
    }
    
    void set(const std::string& name, const Value& val) {
        if (vars.count(name)) {
            vars[name] = val;
            return;
        }
        if (parent) {
            parent->set(name, val);
            return;
        }
        vars[name] = val;
    }
    
    void define(const std::string& name, const Value& val) {
        vars[name] = val;
    }
};

// Interpreter state
class Interpreter {
public:
    std::shared_ptr<Environment> global_env;
    Canvas canvas;
    std::mt19937 rng;
    std::string current_filename;
    std::string current_src;
    std::vector<VglWarning> warnings;
    
    Interpreter() : global_env(std::make_shared<Environment>()) {
        rng.seed(42); // Default seed
    }
    
    void exec(const std::vector<StmtPtr>& stmts);
    void exec_stmt(StmtPtr stmt);
    Value eval(ExprPtr expr);
    
private:
    Value eval_binary(const BinaryExpr* binop);
    Value eval_unary(const UnaryExpr* unary);
    Value eval_call(const CallExpr* call);
};

void Interpreter::exec(const std::vector<StmtPtr>& stmts) {
    for (const auto& stmt : stmts) {
        exec_stmt(stmt);
    }
}

void Interpreter::exec_stmt(StmtPtr stmt) {
    try {
        if (auto s = std::dynamic_pointer_cast<CanvasStmt>(stmt)) {
            canvas = Canvas(s->width, s->height);
        }
        else if (auto s = std::dynamic_pointer_cast<BgStmt>(stmt)) {
            Value v = eval(s->color);
            if (v.type == Value::Color) {
                canvas.clear(std::get<0>(v.color), std::get<1>(v.color),
                            std::get<2>(v.color), std::get<3>(v.color));
            } else if (v.type == Value::Number) {
                uint8_t c = static_cast<uint8_t>(v.number);
                canvas.clear(c, c, c, 255);
            }
        }
        else if (auto s = std::dynamic_pointer_cast<LetStmt>(stmt)) {
            Value val = eval(s->value);
            global_env->define(s->name, val);
        }
        else if (auto s = std::dynamic_pointer_cast<ForStmt>(stmt)) {
            Value start_val = eval(s->start);
            if (s->end) {
                // Range for: for i in start..end [step k]
                Value end_val = eval(s->end);
                double step = s->step ? eval(s->step).number : 1.0;
                
                for (double i = start_val.number; i < end_val.number; i += step) {
                    global_env->define(s->var, Value(i));
                    for (const auto& body_stmt : s->body) {
                        exec_stmt(body_stmt);
                    }
                }
            } else {
                // Array for: for i in array
                Value arr = eval(s->start);
                if (arr.type == Value::Array) {
                    for (const auto& elem : arr.elements) {
                        global_env->define(s->var, elem);
                        for (const auto& body_stmt : s->body) {
                            exec_stmt(body_stmt);
                        }
                    }
                }
            }
        }
        else if (auto s = std::dynamic_pointer_cast<WhileStmt>(stmt)) {
            while (eval(s->cond).number != 0.0) {
                for (const auto& body_stmt : s->body) {
                    exec_stmt(body_stmt);
                }
            }
        }
        else if (auto s = std::dynamic_pointer_cast<IfStmt>(stmt)) {
            if (eval(s->cond).number != 0.0) {
                for (const auto& body_stmt : s->then_body) {
                    exec_stmt(body_stmt);
                }
            } else if (s->else_body) {
                for (const auto& body_stmt : *s->else_body) {
                    exec_stmt(body_stmt);
                }
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
                uint8_t a = (rgb.elements.size() >= 4) ? 
                           static_cast<uint8_t>(rgb.elements[3].number) : 255;
                canvas.set_pixel(x, y, r, g, b, a);
            }
        }
        else if (auto s = std::dynamic_pointer_cast<StrokeStmt>(stmt)) {
            // Simplified stroke handling
            if (s->fields.count("path") && s->fields.count("color")) {
                Value color = eval(s->fields["color"]);
                uint8_t r, g, b, a;
                if (color.type == Value::Color) {
                    r = std::get<0>(color.color);
                    g = std::get<1>(color.color);
                    b = std::get<2>(color.color);
                    a = std::get<3>(color.color);
                } else {
                    r = g = b = 128; a = 255;
                }
                
                int width = 1;
                if (s->fields.count("width")) {
                    width = static_cast<int>(eval(s->fields["width"]).number);
                }
                
                // Parse path (simplified - just handle line and circle)
                // Full implementation would parse the path expression
            }
        }
        else if (auto s = std::dynamic_pointer_cast<RenderStmt>(stmt)) {
            if (!canvas.save_png(s->filename)) {
                std::cerr << "Failed to save: " << s->filename << std::endl;
            } else {
                std::cout << "Rendered: " << s->filename << " (" 
                         << canvas.width << "x" << canvas.height << ")" << std::endl;
            }
        }
        else if (auto s = std::dynamic_pointer_cast<FnDefStmt>(stmt)) {
            // Store function definition (simplified)
            global_env->define(s->name, Value(0.0)); // Placeholder
        }
        else if (auto s = std::dynamic_pointer_cast<ExprStmt>(stmt)) {
            eval(s->expr); // Evaluate and discard result
        }
    } catch (const VglError& e) {
        std::cerr << format_error(e.msg, current_src, e.pos, current_filename) << std::endl;
        throw;
    }
}

Value Interpreter::eval(ExprPtr expr) {
    if (auto e = std::dynamic_pointer_cast<NumberExpr>(expr)) {
        return Value(e->value);
    }
    if (auto e = std::dynamic_pointer_cast<StringExpr>(expr)) {
        return Value(e->value);
    }
    if (auto e = std::dynamic_pointer_cast<ColorExpr>(expr)) {
        return Value(e->r, e->g, e->b, e->a);
    }
    if (auto e = std::dynamic_pointer_cast<IdentExpr>(expr)) {
        return global_env->get(e->name);
    }
    if (auto e = std::dynamic_pointer_cast<BinaryExpr>(expr)) {
        return eval_binary(e.get());
    }
    if (auto e = std::dynamic_pointer_cast<UnaryExpr>(expr)) {
        return eval_unary(e.get());
    }
    if (auto e = std::dynamic_pointer_cast<CallExpr>(expr)) {
        return eval_call(e.get());
    }
    if (auto e = std::dynamic_pointer_cast<TupleExpr>(expr)) {
        Value v;
        v.type = Value::Tuple;
        for (const auto& elem : e->elements) {
            v.elements.push_back(eval(elem));
        }
        return v;
    }
    if (auto e = std::dynamic_pointer_cast<ArrayExpr>(expr)) {
        Value v;
        v.type = Value::Array;
        for (const auto& elem : e->elements) {
            v.elements.push_back(eval(elem));
        }
        return v;
    }
    
    return Value(0.0);
}

Value Interpreter::eval_binary(const BinaryExpr* binop) {
    Value left = eval(binop->left);
    Value right = eval(binop->right);
    
    double l = left.number;
    double r = right.number;
    
    if (binop->op == "+") return Value(l + r);
    if (binop->op == "-") return Value(l - r);
    if (binop->op == "*") return Value(l * r);
    if (binop->op == "/") return Value(r != 0 ? l / r : 0);
    if (binop->op == "%") return Value(r != 0 ? std::fmod(l, r) : 0);
    if (binop->op == "==") return Value(l == r ? 1.0 : 0.0);
    if (binop->op == "!=") return Value(l != r ? 1.0 : 0.0);
    if (binop->op == "<") return Value(l < r ? 1.0 : 0.0);
    if (binop->op == ">") return Value(l > r ? 1.0 : 0.0);
    if (binop->op == "<=") return Value(l <= r ? 1.0 : 0.0);
    if (binop->op == ">=") return Value(l >= r ? 1.0 : 0.0);
    
    return Value(0.0);
}

Value Interpreter::eval_unary(const UnaryExpr* unary) {
    Value operand = eval(unary->operand);
    
    if (unary->op == "-") return Value(-operand.number);
    if (unary->op == "!") return Value(operand.number == 0.0 ? 1.0 : 0.0);
    if (unary->op == "not") return Value(operand.number == 0.0 ? 1.0 : 0.0);
    
    return operand;
}

Value Interpreter::eval_call(const CallExpr* call) {
    std::vector<Value> args;
    for (const auto& arg : call->args) {
        args.push_back(eval(arg));
    }
    
    // Built-in functions
    if (call->callee == "rand") {
        if (args.size() >= 2) {
            double lo = args[0].number;
            double hi = args[1].number;
            std::uniform_real_distribution<> dist(lo, hi);
            return Value(dist(rng));
        }
        std::uniform_real_distribution<> dist(0.0, 1.0);
        return Value(dist(rng));
    }
    if (call->callee == "int") {
        if (!args.empty()) return Value(static_cast<double>(static_cast<int>(args[0].number)));
        return Value(0.0);
    }
    if (call->callee == "cos") {
        if (!args.empty()) return Value(std::cos(args[0].number));
        return Value(1.0);
    }
    if (call->callee == "sin") {
        if (!args.empty()) return Value(std::sin(args[0].number));
        return Value(0.0);
    }
    if (call->callee == "tan") {
        if (!args.empty()) return Value(std::tan(args[0].number));
        return Value(0.0);
    }
    if (call->callee == "sqrt") {
        if (!args.empty()) return Value(std::sqrt(args[0].number));
        return Value(0.0);
    }
    if (call->callee == "abs") {
        if (!args.empty()) return Value(std::abs(args[0].number));
        return Value(0.0);
    }
    if (call->callee == "pow") {
        if (args.size() >= 2) return Value(std::pow(args[0].number, args[1].number));
        return Value(1.0);
    }
    if (call->callee == "min") {
        if (args.size() >= 2) return Value(std::min(args[0].number, args[1].number));
        return Value(0.0);
    }
    if (call->callee == "max") {
        if (args.size() >= 2) return Value(std::max(args[0].number, args[1].number));
        return Value(0.0);
    }
    if (call->callee == "floor") {
        if (!args.empty()) return Value(std::floor(args[0].number));
        return Value(0.0);
    }
    if (call->callee == "ceil") {
        if (!args.empty()) return Value(std::ceil(args[0].number));
        return Value(0.0);
    }
    if (call->callee == "round") {
        if (!args.empty()) return Value(std::round(args[0].number));
        return Value(0.0);
    }
    
    // Unknown function
    return Value(0.0);
}

} // namespace vgl

std::string vgl::format_error(const std::string& msg, const std::string& src, 
                              size_t pos, const std::string& filename) {
    // Calculate line and column
    size_t line = 1, col = 1;
    for (size_t i = 0; i < pos && i < src.size(); ++i) {
        if (src[i] == '\n') {
            line++;
            col = 1;
        } else {
            col++;
        }
    }
    
    std::ostringstream oss;
    oss << filename << ":" << line << ":" << col << ": error: " << msg << "\n";
    
    // Show the line with caret
    size_t line_start = 0;
    for (size_t i = 0; i < pos && i < src.size(); ++i) {
        if (src[i] == '\n') line_start = i + 1;
    }
    size_t line_end = src.find('\n', line_start);
    if (line_end == std::string::npos) line_end = src.size();
    
    oss << std::string(src.begin() + line_start, src.begin() + line_end) << "\n";
    oss << std::string(col - 1, ' ') << "^\n";
    
    return oss.str();
}
