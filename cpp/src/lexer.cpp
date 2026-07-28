#include "lexer.hpp"
#include <stdexcept>
#include <tuple>

namespace vgl {

static const std::vector<std::string> KEYWORDS = {
    "canvas", "bg", "let", "for", "in", "if", "else",
    "fn", "return", "pixel", "stroke", "render",
    "while", "break", "and", "or", "not", "seed", "true", "false",
    "continue", "struct", "import", "material", "layer", "field",
    "null", "const", "var", "as", "match", "case", "default",
    "enum", "class", "from", "module", "step"
};

Lexer::Lexer(const std::string& source) 
    : source_(source), chars_(source.begin(), source.end()), pos_(0) {}

char Lexer::peek() const {
    if (pos_ >= chars_.size()) return '\0';
    return chars_[pos_];
}

char Lexer::advance() {
    if (pos_ >= chars_.size()) return '\0';
    return chars_[pos_++];
}

void Lexer::skip_whitespace() {
    while (true) {
        char c = peek();
        if (c == '\0' || !std::isspace(static_cast<unsigned char>(c))) break;
        advance();
    }
}

double Lexer::read_number() {
    size_t start = pos_;
    while (true) {
        char c = peek();
        if (!std::isdigit(static_cast<unsigned char>(c))) break;
        advance();
    }
    
    // Check for decimal point (but not ..)
    if (peek() == '.' && pos_ + 1 < chars_.size() && chars_[pos_ + 1] != '.') {
        advance(); // consume '.'
        while (true) {
            char c = peek();
            if (!std::isdigit(static_cast<unsigned char>(c))) break;
            advance();
        }
    }
    
    std::string num_str(chars_.begin() + start, chars_.begin() + pos_);
    try {
        return std::stod(num_str);
    } catch (...) {
        throw VglError("Illegal number: " + num_str, start);
    }
}

std::string Lexer::read_ident() {
    size_t start = pos_;
    while (true) {
        char c = peek();
        if (!std::isalnum(static_cast<unsigned char>(c)) && c != '_') break;
        advance();
    }
    return std::string(chars_.begin() + start, chars_.begin() + pos_);
}

std::string Lexer::read_string() {
    size_t start_pos = pos_;
    advance(); // skip opening quote
    std::string result;
    
    while (true) {
        char c = peek();
        if (c == '\0') {
            throw VglError("Unterminated string", start_pos);
        }
        if (c == '"') {
            advance();
            return result;
        }
        if (c == '\\') {
            advance();
            char next = peek();
            switch (next) {
                case 'n': result += '\n'; break;
                case 't': result += '\t'; break;
                case 'r': result += '\r'; break;
                case '\\': result += '\\'; break;
                case '"': result += '"'; break;
                case '0': result += '\0'; break;
                default:
                    result += '\\';
                    result += next;
                    break;
            }
            advance();
            continue;
        }
        result += c;
        advance();
    }
}

std::tuple<uint8_t, uint8_t, uint8_t, uint8_t> Lexer::read_color() {
    size_t start_pos = pos_;
    advance(); // skip '#'
    
    size_t hex_start = pos_;
    while (true) {
        char c = peek();
        if (!std::isxdigit(static_cast<unsigned char>(c))) break;
        advance();
    }
    
    std::string hex(chars_.begin() + hex_start, chars_.begin() + pos_);
    
    auto hex_to_u8 = [](const std::string& h) -> uint8_t {
        return static_cast<uint8_t>(std::stoul(h, nullptr, 16));
    };
    
    if (hex.size() == 6) {
        return {hex_to_u8(hex.substr(0,2)), hex_to_u8(hex.substr(2,2)), 
                hex_to_u8(hex.substr(4,2)), 255};
    } else if (hex.size() == 8) {
        return {hex_to_u8(hex.substr(0,2)), hex_to_u8(hex.substr(2,2)), 
                hex_to_u8(hex.substr(4,2)), hex_to_u8(hex.substr(6,2))};
    } else if (hex.size() == 3) {
        std::string r(2, hex[0]), g(2, hex[1]), b(2, hex[2]);
        return {hex_to_u8(r), hex_to_u8(g), hex_to_u8(b), 255};
    } else if (hex.size() == 4) {
        std::string r(2, hex[0]), g(2, hex[1]), b(2, hex[2]), a(2, hex[3]);
        return {hex_to_u8(r), hex_to_u8(g), hex_to_u8(b), hex_to_u8(a)};
    } else {
        throw VglError("Invalid color: #" + hex, start_pos);
    }
}

bool Lexer::is_keyword(const std::string& ident) const {
    for (const auto& kw : KEYWORDS) {
        if (kw == ident) return true;
    }
    return false;
}

Token Lexer::next_token() {
    while (true) {
        skip_whitespace();
        
        char c = peek();
        if (c == '\0') {
            Token t(TokenType::Eof, pos_);
            return t;
        }
        
        // Skip comments
        if (c == '/' && pos_ + 1 < chars_.size()) {
            char next = chars_[pos_ + 1];
            if (next == '/') {
                // Line comment
                pos_ += 2;
                while (peek() != '\n' && peek() != '\0') advance();
                if (peek() == '\n') advance();
                continue;
            }
            if (next == '*') {
                // Block comment
                size_t block_start = pos_;
                pos_ += 2;
                while (true) {
                    if (peek() == '\0') {
                        throw VglError("Unterminated block comment", block_start);
                    }
                    if (peek() == '*' && pos_ + 1 < chars_.size() && chars_[pos_ + 1] == '/') {
                        pos_ += 2;
                        break;
                    }
                    advance();
                }
                continue;
            }
        }
        
        break;
    }
    
    size_t tok_pos = pos_;
    char c = peek();
    
    if (c == '\0') {
        Token t(TokenType::Eof, pos_);
        return t;
    }
    
    // Range operator or decimal starting with .
    if (c == '.') {
        if (pos_ + 1 < chars_.size() && chars_[pos_ + 1] == '.') {
            advance(); advance();
            Token t(TokenType::DotDot, tok_pos);
            t.value = std::string("..");
            return t;
        }
        if (pos_ + 1 < chars_.size() && std::isdigit(static_cast<unsigned char>(chars_[pos_ + 1]))) {
            Token t(TokenType::Number, tok_pos);
            t.value = read_number();
            return t;
        }
    }
    
    // Number
    if (std::isdigit(static_cast<unsigned char>(c))) {
        Token t(TokenType::Number, tok_pos);
        t.value = read_number();
        return t;
    }
    
    // String
    if (c == '"') {
        Token t(TokenType::String, tok_pos);
        t.value = read_string();
        return t;
    }
    
    // Color
    if (c == '#') {
        Token t(TokenType::Color, tok_pos);
        t.value = read_color();
        return t;
    }
    
    // Identifier or keyword
    if (std::isalpha(static_cast<unsigned char>(c)) || c == '_') {
        std::string ident = read_ident();
        Token t(is_keyword(ident) ? TokenType::Keyword : TokenType::Ident, tok_pos);
        t.value = ident;
        return t;
    }
    
    // Operators and delimiters
    Token t(TokenType::Op, tok_pos);
    std::string op(1, c);
    advance();
    
    // Check for two-character operators
    if (pos_ < chars_.size()) {
        char next = peek();
        std::string two = op + next;
        if (two == "<<" || two == ">>" || two == "++" || two == "--" || two == "=>") {
            advance();
            op = two;
        } else if (next == '=' && (c == '<' || c == '>' || c == '=' || c == '!' || 
                   c == '+' || c == '-' || c == '*' || c == '/' || c == '%')) {
            advance();
            op = two;
        }
    }
    
    // Set token type based on operator/delimiter
    if (op == "(") t.type = TokenType::LParen;
    else if (op == ")") t.type = TokenType::RParen;
    else if (op == "{") t.type = TokenType::LBrace;
    else if (op == "}") t.type = TokenType::RBrace;
    else if (op == "[") t.type = TokenType::LBracket;
    else if (op == "]") t.type = TokenType::RBracket;
    else if (op == ",") t.type = TokenType::Comma;
    else if (op == ":") t.type = TokenType::Colon;
    else if (op == ".") t.type = TokenType::Dot;
    
    t.value = op;
    return t;
}

std::vector<Token> Lexer::tokenize() {
    std::vector<Token> tokens;
    while (true) {
        Token t = next_token();
        tokens.push_back(t);
        if (t.type == TokenType::Eof) break;
    }
    return tokens;
}

} // namespace vgl
