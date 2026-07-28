#include "parser.hpp"
#include <stdexcept>
#include <algorithm>

namespace vgl {

Parser::Parser(const std::vector<Token>& tokens) : tokens_(tokens), pos_(0) {}

const Token& Parser::peek() const {
    if (pos_ >= tokens_.size()) return tokens_.back();
    return tokens_[pos_];
}

size_t Parser::peek_pos() const {
    if (pos_ >= tokens_.size()) return tokens_.back().pos;
    return tokens_[pos_].pos;
}

Token Parser::advance() {
    if (pos_ >= tokens_.size()) return tokens_.back();
    return tokens_[pos_++];
}

void Parser::expect(TokenType type, const std::string& msg) {
    if (peek().type != type) {
        throw VglError(msg.empty() ? "Unexpected token" : msg, peek_pos());
    }
    advance();
}

void Parser::expect_keyword(const std::string& kw) {
    if (peek().type != TokenType::Keyword || 
        std::get<std::string>(peek().value) != kw) {
        throw VglError("Expected keyword '" + kw + "'", peek_pos());
    }
    advance();
}

void Parser::expect_op(const std::string& op) {
    if (peek().type != TokenType::Op || 
        std::get<std::string>(peek().value) != op) {
        throw VglError("Expected operator '" + op + "'", peek_pos());
    }
    advance();
}

std::vector<StmtPtr> Parser::parse_program() {
    std::vector<StmtPtr> stmts;
    while (peek().type != TokenType::Eof) {
        stmts.push_back(parse_stmt());
    }
    return stmts;
}

StmtPtr Parser::parse_stmt() {
    size_t start_pos = peek_pos();
    
    if (peek().type == TokenType::Keyword) {
        std::string kw = std::get<std::string>(peek().value);
        
        if (kw == "canvas") {
            advance();
            if (peek().type != TokenType::Number) {
                throw VglError("canvas requires width", peek_pos());
            }
            uint32_t w = static_cast<uint32_t>(std::get<double>(peek().value));
            advance();
            
            uint32_t h;
            if (peek().type == TokenType::Ident) {
                std::string s = std::get<std::string>(peek().value);
                if (!s.empty() && s[0] == 'x') {
                    advance();
                    h = std::stoul(s.substr(1));
                } else {
                    throw VglError("canvas expects WxH format", peek_pos());
                }
            } else if (peek().type == TokenType::Number) {
                h = static_cast<uint32_t>(std::get<double>(peek().value));
                advance();
            } else {
                throw VglError("canvas expects WxH format", peek_pos());
            }
            
            return std::make_shared<CanvasStmt>(w, h, start_pos);
        }
        
        if (kw == "bg") {
            advance();
            return std::make_shared<BgStmt>(parse_expr(), start_pos);
        }
        
        if (kw == "let" || kw == "var") {
            advance();
            if (peek().type != TokenType::Ident) {
                throw VglError("let/var requires identifier", peek_pos());
            }
            std::string name = std::get<std::string>(peek().value);
            advance();
            expect_op("=");
            return std::make_shared<LetStmt>(name, parse_expr(), start_pos);
        }
        
        if (kw == "for") {
            advance();
            if (peek().type != TokenType::Ident) {
                throw VglError("for requires iteration variable", peek_pos());
            }
            std::string var = std::get<std::string>(peek().value);
            advance();
            expect_keyword("in");
            
            ExprPtr start = parse_expr();
            
            if (peek().type == TokenType::DotDot) {
                advance();
                ExprPtr end = parse_expr();
                
                ExprPtr step;
                if (peek().type == TokenType::Keyword && 
                    std::get<std::string>(peek().value) == "step") {
                    advance();
                    step = parse_expr();
                }
                
                expect(TokenType::LBrace);
                loop_depth_++;
                std::vector<StmtPtr> body;
                while (peek().type != TokenType::RBrace) {
                    body.push_back(parse_stmt());
                }
                loop_depth_--;
                expect(TokenType::RBrace);
                
                auto stmt = std::make_shared<ForStmt>(var, start, end, step, start_pos);
                stmt->body = body;
                return stmt;
            }
        }
        
        if (kw == "if") {
            advance();
            ExprPtr cond = parse_expr();
            expect(TokenType::LBrace);
            std::vector<StmtPtr> then_body;
            while (peek().type != TokenType::RBrace) {
                then_body.push_back(parse_stmt());
            }
            expect(TokenType::RBrace);
            
            auto stmt = std::make_shared<IfStmt>(cond, start_pos);
            stmt->then_body = then_body;
            
            if (peek().type == TokenType::Keyword && 
                std::get<std::string>(peek().value) == "else") {
                advance();
                expect(TokenType::LBrace);
                std::vector<StmtPtr> else_body;
                while (peek().type != TokenType::RBrace) {
                    else_body.push_back(parse_stmt());
                }
                expect(TokenType::RBrace);
                stmt->else_body = else_body;
            }
            
            return stmt;
        }
        
        if (kw == "seed") {
            advance();
            if (peek().type != TokenType::Number) {
                throw VglError("seed requires integer", peek_pos());
            }
            uint64_t s = static_cast<uint64_t>(std::get<double>(peek().value));
            advance();
            return std::make_shared<SeedStmt>(s, start_pos);
        }
        
        if (kw == "pixel") {
            advance();
            expect(TokenType::LParen);
            auto fields = parse_kwargs_block();
            expect(TokenType::RParen);
            
            ExprPtr x, y, rgb;
            if (fields.count("x")) x = fields["x"];
            if (fields.count("y")) y = fields["y"];
            if (fields.count("rgb")) rgb = fields["rgb"];
            
            return std::make_shared<PixelStmt>(
                x ? x : std::make_shared<NumberExpr>(0.0, start_pos),
                y ? y : std::make_shared<NumberExpr>(0.0, start_pos),
                rgb ? rgb : std::make_shared<ColorExpr>(0, 0, 0, 255, start_pos),
                start_pos
            );
        }
        
        if (kw == "render") {
            advance();
            if (peek().type != TokenType::String) {
                throw VglError("render requires string filename", peek_pos());
            }
            std::string fname = std::get<std::string>(peek().value);
            advance();
            return std::make_shared<RenderStmt>(fname, start_pos);
        }
    }
    
    // Expression statement
    return std::make_shared<ExprStmt>(parse_expr(), start_pos);
}

std::map<std::string, ExprPtr> Parser::parse_kwargs_block() {
    std::map<std::string, ExprPtr> kwargs;
    
    while (true) {
        if (peek().type == TokenType::RBrace || peek().type == TokenType::RParen || 
            peek().type == TokenType::Eof) {
            break;
        }
        
        if (peek().type == TokenType::Ident) {
            std::string key = std::get<std::string>(peek().value);
            advance();
            
            if (peek().type == TokenType::Colon) {
                advance();
                kwargs[key] = parse_expr();
            } else {
                break;
            }
            
            if (peek().type == TokenType::Comma) {
                advance();
            }
        } else {
            break;
        }
    }
    
    return kwargs;
}

ExprPtr Parser::parse_expr() {
    return parse_logical();
}

ExprPtr Parser::parse_logical() {
    ExprPtr left = parse_comparison();
    
    while (peek().type == TokenType::Keyword) {
        std::string kw = std::get<std::string>(peek().value);
        if (kw == "and" || kw == "or") {
            std::string op = kw;
            advance();
            left = std::make_shared<BinaryExpr>(left, op, parse_comparison(), left->pos);
        } else if (kw == "not") {
            advance();
            left = std::make_shared<UnaryExpr>("not", parse_primary(), left->pos);
        } else {
            break;
        }
    }
    
    return left;
}

ExprPtr Parser::parse_comparison() {
    ExprPtr left = parse_additive();
    
    while (peek().type == TokenType::Op) {
        std::string op = std::get<std::string>(peek().value);
        if (op == "==" || op == "!=" || op == "<" || op == ">" || 
            op == "<=" || op == ">=") {
            advance();
            left = std::make_shared<BinaryExpr>(left, op, parse_additive(), left->pos);
        } else {
            break;
        }
    }
    
    return left;
}

ExprPtr Parser::parse_additive() {
    ExprPtr left = parse_multiplicative();
    
    while (peek().type == TokenType::Op) {
        std::string op = std::get<std::string>(peek().value);
        if (op == "+" || op == "-") {
            advance();
            left = std::make_shared<BinaryExpr>(left, op, parse_multiplicative(), left->pos);
        } else {
            break;
        }
    }
    
    return left;
}

ExprPtr Parser::parse_multiplicative() {
    ExprPtr left = parse_unary();
    
    while (peek().type == TokenType::Op) {
        std::string op = std::get<std::string>(peek().value);
        if (op == "*" || op == "/" || op == "%") {
            advance();
            left = std::make_shared<BinaryExpr>(left, op, parse_unary(), left->pos);
        } else {
            break;
        }
    }
    
    return left;
}

ExprPtr Parser::parse_unary() {
    if (peek().type == TokenType::Op) {
        std::string op = std::get<std::string>(peek().value);
        if (op == "-" || op == "!") {
            advance();
            return std::make_shared<UnaryExpr>(op, parse_unary(), peek_pos());
        }
    }
    return parse_primary();
}

ExprPtr Parser::parse_primary() {
    size_t pos = peek_pos();
    
    if (peek().type == TokenType::Number) {
        double val = std::get<double>(peek().value);
        advance();
        return std::make_shared<NumberExpr>(val, pos);
    }
    
    if (peek().type == TokenType::String) {
        std::string val = std::get<std::string>(peek().value);
        advance();
        return std::make_shared<StringExpr>(val, pos);
    }
    
    if (peek().type == TokenType::Color) {
        auto color_val = std::get<std::tuple<uint8_t,uint8_t,uint8_t,uint8_t>>(peek().value);
        uint8_t r = std::get<0>(color_val);
        uint8_t g = std::get<1>(color_val);
        uint8_t b = std::get<2>(color_val);
        uint8_t a = std::get<3>(color_val);
        advance();
        return std::make_shared<ColorExpr>(r, g, b, a, pos);
    }
    
    if (peek().type == TokenType::Ident) {
        std::string name = std::get<std::string>(peek().value);
        advance();
        
        if (peek().type == TokenType::LParen) {
            advance();
            auto call = std::make_shared<CallExpr>(name, pos);
            
            while (peek().type != TokenType::RParen) {
                if (peek().type == TokenType::Ident) {
                    std::string arg_name = std::get<std::string>(peek().value);
                    advance();
                    if (peek().type == TokenType::Colon) {
                        advance();
                        call->kwargs[arg_name] = parse_expr();
                    } else {
                        call->args.push_back(std::make_shared<IdentExpr>(arg_name, pos));
                    }
                } else {
                    call->args.push_back(parse_expr());
                }
                
                if (peek().type == TokenType::Comma) {
                    advance();
                } else {
                    break;
                }
            }
            expect(TokenType::RParen);
            return call;
        }
        
        return std::make_shared<IdentExpr>(name, pos);
    }
    
    if (peek().type == TokenType::LParen) {
        advance();
        std::vector<ExprPtr> elements;
        if (peek().type != TokenType::RParen) {
            elements.push_back(parse_expr());
            while (peek().type == TokenType::Comma) {
                advance();
                if (peek().type != TokenType::RParen) {
                    elements.push_back(parse_expr());
                }
            }
        }
        expect(TokenType::RParen);
        
        if (elements.size() > 1) {
            auto tuple = std::make_shared<TupleExpr>(pos);
            tuple->elements = elements;
            return tuple;
        } else if (elements.size() == 1) {
            return elements[0];
        }
        
        auto tuple = std::make_shared<TupleExpr>(pos);
        return tuple;
    }
    
    if (peek().type == TokenType::Keyword) {
        std::string kw = std::get<std::string>(peek().value);
        if (kw == "true" || kw == "false") {
            advance();
            return std::make_shared<NumberExpr>(kw == "true" ? 1.0 : 0.0, pos);
        }
    }
    
    throw VglError("Unexpected token in expression", peek_pos());
}

} // namespace vgl
