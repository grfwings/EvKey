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

    fn parse_action(&mut self) -> Result<Action, String> {
        match self.peek_type() {

            TokenType::Hold => {
                let line: usize = self.peek().line;

                self.advance();

                // Could be list or single key
                match self.peek_type() {


                    TokenType::UpperIdent | TokenType::Identifier => {
                        let target: HoldTarget = HoldTarget::Single(self.parse_expr()?);
                        self.expect(TokenType::For)?;
                        let duration: Expr = self.parse_expr()?;
                        Ok(Action::Hold { target, duration, line })
                    }

                    // Hold list case
                    TokenType::LeftBrace => {
                        self.advance();
                        let target: HoldTarget  = HoldTarget::InlineSet(self.parse_hold_list()?);
                        self.expect(TokenType::RightBrace)?;
                        self.expect(TokenType::For)?;
                        let duration: Expr = self.parse_expr()?;
                        Ok(Action::Hold { target, duration, line })
                    }

                    _ => self.error(&format!("Expected '{{' or an identifier, got {}", self.peek().lexeme))
                }
            }

            // Anonymous hold list
            TokenType::LeftBrace => {
                let line: usize = self.peek().line;
                self.advance();

                let target: HoldTarget = HoldTarget::InlineSet(self.parse_hold_list()?);
                self.expect(TokenType::RightBrace)?;
                self.expect(TokenType::For)?;
                let duration: Expr = self.parse_expr()?;
                Ok(Action::Hold { target, duration, line })
            }

            TokenType::Tap => {
                let line: usize = self.advance().line;

                match self.peek_type() {

                    TokenType::Identifier | TokenType::UpperIdent => {
                        let key: Expr = self.parse_expr()?;
                        Ok(Action::Tap { key, line })
                    }

                    _ => self.error(&format!("Expected Identifier or Key, got {}", self.peek().lexeme))
                }

            }

            TokenType::Wait => {
                let line: usize = self.advance().line;

                let duration: Expr = self.parse_expr()?;

                Ok(Action::Wait { duration, line })
            }

            TokenType::Move => {
                let line: usize = self.advance().line;

                // Safe to unwrap here because we already checking TokenType::Number

                let x = self.expect(TokenType::Number)?.literal.unwrap();

                let y = self.expect(TokenType::Number)?.literal.unwrap();

                Ok(Action::Move { x, y, line })
            }

            TokenType::Scroll => {
                let line: usize = self.advance().line;

                let direction = self.parse_direction()?;

                let amount = self.expect(TokenType::Number)?.literal.unwrap();

                Ok(Action::Scroll { direction, amount, line })
            }

            TokenType::Run => {
                let line: usize = self.advance().line;

                if self.peek_type() == TokenType::Number {
                    return self.error(&format!("Expected identifier or constant, got number: {}", self.peek().lexeme));
                }

                let name = self.advance().lexeme;
                let mut args: Vec<Expr> = Vec::new();

                // Optional arguments
                if self.peek_type() == TokenType::LeftParen {
                    self.advance();
                    args = self.parse_args()?;
                    self.expect(TokenType::RightParen)?;
                }

                Ok(Action::Run { name, args, line })

            }

            TokenType::Identifier | TokenType::UpperIdent => {

                let line: usize = self.peek().line; 
                let name = self.advance().lexeme;
                let mut args: Vec<Expr> = Vec::new();
                // Optional arguments
                if self.peek_type() == TokenType::LeftParen {
                    self.advance();
                    args = self.parse_args()?;
                    self.expect(TokenType::RightParen)?;
                }

                self.expect(TokenType::For)?;

                let duration = self.parse_expr()?;

                Ok(Action::UseWithDuration { name, args, duration, line })
            }

            _ => self.error(&format!("Expected an action, got {}", self.peek().lexeme))
        }
    }

    /// (statement ";")*
    fn parse_statement(&mut self) -> Result<Stmt, String> {
        match self.peek_type() {

            TokenType::Let => {
                let def = self.parse_definition()?;
                Ok(Stmt::Definition(def))
            }

            _ => {
                let action = self.parse_action()?;
                Ok(Stmt::Action(action))
            }
        }
    }

    /// "let" (identifier | const_name) params? "=" value
    fn parse_definition(&mut self) -> Result<Definition, String> {
        let line: usize = self.advance().line; // Consume "Let"

        match self.peek_type() {

            TokenType::Identifier | TokenType::UpperIdent => {
                let name = self.advance().lexeme;
                let mut params: Vec<String> = Vec::new();
                if self.peek_type() == TokenType::LeftParen {
                    self.advance();
                    params = self.parse_params()?;
                    self.expect(TokenType::RightParen)?;
                }
                self.expect(TokenType::Equals)?;
                let value = self.parse_value()?;
                Ok(Definition { name, params, value, line })
            }

            _ => self.error(&format!("Expected identifier or const, got {}", self.peek().lexeme))
        }
    }

    /// "[" statement_list "]"
    fn parse_sequence(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect(TokenType::LeftBracket)?;

        let mut seq: Vec<Stmt> = Vec::new();

        loop {
            if self.peek_type() == TokenType::RightBracket {
                return Ok(seq);
            }
            seq.push(self.parse_statement()?);
        }
    }

    // Either number (constant), set (hold_list) or sequence (procedure)
    fn parse_value(&mut self) -> Result<Value, String> {

        match self.peek_type() {

            TokenType::Number => {
                let line: usize = self.peek().line;
                let val: i64 = self.advance().literal.unwrap();
                Ok(Value::Constant(val, line))
            }

            TokenType::LeftBrace => {
                let line: usize = self.advance().line;
                let set: Vec<Expr> = self.parse_hold_list()?;
                self.expect(TokenType::RightBrace)?;
                Ok(Value::Set(set, line))
            }

            TokenType::Hold => {
                let line: usize = self.advance().line;
                self.expect(TokenType::LeftBrace)?;
                let set: Vec<Expr> = self.parse_key_list()?;
                self.expect(TokenType::RightBrace)?;
                Ok(Value::Set(set, line))
            }

            // Sequence case
            TokenType::LeftBracket => {
                let line: usize = self.peek().line;

                let seq = self.parse_sequence()?;

                self.expect(TokenType::RightBracket)?;

                Ok(Value::Sequence(seq, line))
            }
            _ => self.error(&format!("Expected a value, got {}", self.peek().lexeme))
        }
    }

    fn parse_program(&mut self) -> Result<Program, String> {
        let mut statements: Vec<Stmt> = Vec::new();

        loop {
            match self.peek_type() {
                TokenType::Eof => return Ok(Program { statements }),
                _ => {
                    statements.push(self.parse_statement()?);
                    self.expect(TokenType::Semicolon)?;
                }
            }
        }
    }
}
