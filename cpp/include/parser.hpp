#ifndef VGL_PARSER_HPP
#define VGL_PARSER_HPP

#include "vgl.hpp"
#include <vector>
#include <string>

namespace vgl {

class Parser {
public:
    Parser(const std::vector<Token>& tokens);
    
    std::vector<StmtPtr> parse_program();
    
private:
    std::vector<Token> tokens_;
    size_t pos_ = 0;
    int loop_depth_ = 0;
    
    const Token& peek() const;
    size_t peek_pos() const;
    Token advance();
    void expect(TokenType type, const std::string& expected_msg = "");
    void expect_keyword(const std::string& kw);
    void expect_op(const std::string& op);
    
    StmtPtr parse_stmt();
    StmtPtr parse_stmt_impl();
    ExprPtr parse_expr();
    ExprPtr parse_primary();
    ExprPtr parse_unary();
    ExprPtr parse_multiplicative();
    ExprPtr parse_additive();
    ExprPtr parse_comparison();
    ExprPtr parse_logical();
    ExprPtr parse_assignment();
    
    std::vector<StmtPtr> parse_block();
    std::map<std::string, ExprPtr> parse_kwargs_block();
};

} // namespace vgl

#endif // VGL_PARSER_HPP
