#ifndef VGL_LEXER_HPP
#define VGL_LEXER_HPP

#include "vgl.hpp"
#include <string>
#include <vector>
#include <cctype>

namespace vgl {

class Lexer {
public:
    Lexer(const std::string& source);
    
    std::vector<Token> tokenize();
    
private:
    std::string source_;
    std::vector<char> chars_;
    size_t pos_ = 0;
    
    char peek() const;
    char advance();
    void skip_whitespace();
    double read_number();
    std::string read_ident();
    std::string read_string();
    std::tuple<uint8_t, uint8_t, uint8_t, uint8_t> read_color();
    Token next_token();
    
    bool is_keyword(const std::string& ident) const;
};

} // namespace vgl

#endif // VGL_LEXER_HPP
