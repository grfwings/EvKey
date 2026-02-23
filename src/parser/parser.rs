//! Parser for EvScript
//!
//! Transform takens produced by the scanner into an AST (ast.rs) following the
//! grammar in LANGUAGE.md using recursive descent.

use crate::parser::token::{Token, TokenType};
use crate::parser::ast::*;

struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {

    /// Create a parser from a Vec<Token>
    fn new(tokens: Vec<Token>) -> Parser {
        Parser { tokens, current: 0 }
    }

    /// Peek at the current token without consuming
    fn peek(&self) -> &Token {
        self.tokens.get(self.current).unwrap()
    }

    /// peek at the current token's type
    fn peek_type(&self) -> TokenType {
        self.tokens.get(self.current).unwrap().token_type
    }

    /// Consume and return current token
    fn advance(&mut self) -> Token {
        let ret = self.tokens.get(self.current).unwrap();
        self.current += 1;
        ret.clone()
    }

    /// Check if token matches a given type 
    fn check_token(&self, t: TokenType ) -> bool {
        self.peek_type() == t
    }

    /// Consume token if it matches a given type
    fn match_token(&mut self, t: TokenType ) -> bool {
        if self.check_token(t) {
            self.advance();
            return true;
        } 
        false
    }

    /// Match or error current token
    fn expect(&mut self, t: TokenType) -> Result<Token, String> {
        match self.check_token(t) {
            true => {
                let current_token =  self.advance();
                Ok(current_token)
            }
            false => self.error(&format!("Expected {:?}, found {}", t ,self.peek().lexeme))
        }
    }

    /// Print an error message
    fn error<T>(&self, msg: &str) -> Result<T, String> {
        let line = self.peek().line;
        let err_msg = format!("line {line}: {msg}");
        Err(err_msg)
    }

    fn is_at_end(&self) -> bool {
        self.peek_type() == TokenType::Eof
    }

    /// Expression either a Number, Identifier, or UpperIdent
    fn parse_expr(&mut self) -> Result<Expr, String> {
        match self.peek_type() {
            TokenType::Number => {
                let tok = self.advance();
                Ok(Expr::Number(tok.literal.unwrap(), tok.line))
            }
            TokenType::Identifier => {
                let tok = self.advance();
                Ok(Expr::Identifier(tok.lexeme, tok.line))
            }
            TokenType::UpperIdent => {
                let tok = self.advance();
                Ok(Expr::UpperIdent(tok.lexeme, tok.line))
            }
            _ => self.error(&format!("Expected expression, found {}", self.peek().lexeme))
        }
    }

    fn parse_direction(&mut self) -> Result<Direction, String> {
        match self.peek_type() {
            TokenType::Up => {
                self.advance();
                Ok(Direction::Up)
            }
            TokenType::Down => {
                self.advance();
                Ok(Direction::Down)
            }
            TokenType::Left => {
                self.advance();
                Ok(Direction::Left)
            }
            TokenType::Right => {
                self.advance();
                Ok(Direction::Right)
            }
            _ => self.error(&format!("Expected direction, found {}", self.peek().lexeme))
        }
    }

    fn parse_key_list(&mut self) -> Result<Vec<Expr>, String> {
        let mut key_list: Vec<Expr> = Vec::new();

        loop {
            // check for RightBrace before parsing in case of {} or ,}
            if self.check_token(TokenType::RightBrace) { return Ok(key_list) }

            key_list.push(self.parse_expr()?);
            match self.peek_type() {
                // After consuming an Expr, the only valid tokens are comma or RightBrace
                TokenType::Comma => {
                    self.advance();
                }
                TokenType::RightBrace => {
                    return Ok(key_list)
                }
                _ => return self.error(&format!("Expected ',' or '}}', got {}", self.peek().lexeme))
            }
        }
    }

    fn parse_hold_list(&mut self) -> Result<Vec<Expr>, String> {
        let mut hold_list: Vec<Expr> = Vec::new();

        loop {

            // Empty or comma-terminating set
            if self.check_token(TokenType::RightBrace) { return Ok(hold_list) }

            // Consume the "hold" token 
            self.expect(TokenType::Hold)?;
            hold_list.push(self.parse_expr()?);

            match self.peek_type() {
                TokenType::Comma => {
                    self.advance();
                }
                TokenType::RightBrace => {
                    return Ok(hold_list)
                }
                _ => return self.error(&format!("Expected ',' or '}}', got {}", self.peek().lexeme))
            }
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut param_list: Vec<Expr> = Vec::new();

        loop {
            if self.check_token(TokenType::RightParen) { return Ok(param_list) }

            param_list.push(self.parse_expr()?);
            match self.peek_type() {
                TokenType::Comma => {
                    self.advance();
                }
                TokenType::RightParen => {
                    return Ok(param_list)
                }
                _ => return self.error(&format!("Expected an expression, got {}", self.peek().lexeme))
            }
        }
    }

    fn parse_params(&mut self) -> Result<Vec<String>, String> {
        let mut param_list: Vec<String> = Vec::new();

        loop {
            if self.check_token(TokenType::RightParen) { return Ok(param_list) }

            let tok: Token = self.expect(TokenType::Identifier)?;
            param_list.push(tok.lexeme);

            match self.peek_type() {
                TokenType::Comma => {
                    self.advance();
                }
                TokenType::RightParen => {
                    return Ok(param_list)
                }
                _ => return self.error(&format!("Expected an identifier, got {}", self.peek().lexeme))
            }
        }
    }
}
