//! Parser for EvScript
//!
//! Transform takens produced by the scanner into an AST (ast.rs) following the
//! grammar in LANGUAGE.md using recursive descent.

use crate::parser::ast::*;
use crate::parser::token::{Token, TokenType};

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
    fn check_token(&self, t: TokenType) -> bool {
        self.peek_type() == t
    }

    /// Consume token if it matches a given type
    fn match_token(&mut self, t: TokenType) -> bool {
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
                let current_token = self.advance();
                Ok(current_token)
            }
            false => self.error(&format!("Expected {:?}, found {}", t, self.peek().lexeme)),
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
            _ => self.error(&format!(
                "Expected expression, found {}",
                self.peek().lexeme
            )),
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
            _ => self.error(&format!("Expected direction, found {}", self.peek().lexeme)),
        }
    }

    fn parse_key_list(&mut self) -> Result<Vec<Expr>, String> {
        let mut key_list: Vec<Expr> = Vec::new();

        loop {
            // check for RightBrace before parsing in case of {} or ,}
            if self.check_token(TokenType::RightBrace) {
                return Ok(key_list);
            }

            key_list.push(self.parse_expr()?);
            match self.peek_type() {
                // After consuming an Expr, the only valid tokens are comma or RightBrace
                TokenType::Comma => {
                    self.advance();
                }
                TokenType::RightBrace => return Ok(key_list),
                _ => {
                    return self.error(&format!("Expected ',' or '}}', got {}", self.peek().lexeme));
                }
            }
        }
    }

    fn parse_hold_list(&mut self) -> Result<Vec<Expr>, String> {
        let mut hold_list: Vec<Expr> = Vec::new();

        loop {
            // Empty or comma-terminating set
            if self.check_token(TokenType::RightBrace) {
                return Ok(hold_list);
            }

            // Consume the "hold" token
            self.expect(TokenType::Hold)?;
            hold_list.push(self.parse_expr()?);

            match self.peek_type() {
                TokenType::Comma => {
                    self.advance();
                }
                TokenType::RightBrace => return Ok(hold_list),
                _ => {
                    return self.error(&format!("Expected ',' or '}}', got {}", self.peek().lexeme));
                }
            }
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut param_list: Vec<Expr> = Vec::new();

        loop {
            if self.check_token(TokenType::RightParen) {
                return Ok(param_list);
            }

            param_list.push(self.parse_expr()?);
            match self.peek_type() {
                TokenType::Comma => {
                    self.advance();
                }
                TokenType::RightParen => return Ok(param_list),
                _ => {
                    return self.error(&format!(
                        "Expected an expression, got {}",
                        self.peek().lexeme
                    ));
                }
            }
        }
    }

    fn parse_params(&mut self) -> Result<Vec<String>, String> {
        let mut param_list: Vec<String> = Vec::new();

        loop {
            if self.check_token(TokenType::RightParen) {
                return Ok(param_list);
            }

            let tok: Token = self.expect(TokenType::Identifier)?;
            param_list.push(tok.lexeme);

            match self.peek_type() {
                TokenType::Comma => {
                    self.advance();
                }
                TokenType::RightParen => return Ok(param_list),
                _ => {
                    return self.error(&format!(
                        "Expected an identifier, got {}",
                        self.peek().lexeme
                    ));
                }
            }
        }
    }

    fn parse_action(&mut self) -> Result<Action, String> {
        match self.peek_type() {

            TokenType::Hold => {
                let line: usize = self.advance().line;

                // Could be list or single key
                match self.peek_type() {

                    TokenType::UpperIdent | TokenType::Identifier => {
                        let target: HoldTarget = HoldTarget::Single(self.parse_expr()?);
                        self.expect(TokenType::For)?;
                        let duration: Expr = self.parse_expr()?;
                        Ok(Action::Hold {
                            target,
                            duration,
                            line,
                        })
                    }

                    // hold { A, B, C }
                    TokenType::LeftBrace => {
                        self.advance();
                        let target: HoldTarget = HoldTarget::InlineSet(self.parse_key_list()?);
                        self.expect(TokenType::RightBrace)?;
                        self.expect(TokenType::For)?;
                        let duration: Expr = self.parse_expr()?;
                        Ok(Action::Hold {
                            target,
                            duration,
                            line,
                        })
                    }

                    _ => self.error(&format!(
                        "Expected '{{' or an identifier, got {}",
                        self.peek().lexeme
                    )),
                }
            }

            // Anonymous hold list
            TokenType::LeftBrace => {
                let line: usize = self.advance().line;

                let target: HoldTarget = HoldTarget::InlineSet(self.parse_hold_list()?);
                self.expect(TokenType::RightBrace)?;
                self.expect(TokenType::For)?;
                let duration: Expr = self.parse_expr()?;
                Ok(Action::Hold {
                    target,
                    duration,
                    line,
                })
            }

            TokenType::Tap => {
                let line: usize = self.advance().line;

                match self.peek_type() {
                    TokenType::Identifier | TokenType::UpperIdent => {
                        let key: Expr = self.parse_expr()?;
                        Ok(Action::Tap { key, line })
                    }

                    _ => self.error(&format!(
                        "Expected Identifier or Key, got {}",
                        self.peek().lexeme
                    )),
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

                Ok(Action::Scroll {
                    direction,
                    amount,
                    line,
                })
            }

            TokenType::Run => {
                let line: usize = self.advance().line;

                if self.peek_type() == TokenType::Number {
                    return self.error(&format!(
                        "Expected identifier or constant, got number: {}",
                        self.peek().lexeme
                    ));
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

                Ok(Action::UseWithDuration {
                    name,
                    args,
                    duration,
                    line,
                })
            }

            _ => self.error(&format!("Expected an action, got {}", self.peek().lexeme)),
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
                Ok(Definition {
                    name,
                    params,
                    value,
                    line,
                })
            }

            _ => self.error(&format!(
                "Expected identifier or const, got {}",
                self.peek().lexeme
            )),
        }
    }

    /// "[" statement_list "]"
    fn parse_sequence(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect(TokenType::LeftBracket)?;

        let mut seq: Vec<Stmt> = Vec::new();

        loop {
            if self.peek_type() == TokenType::RightBracket {
                self.advance();
                return Ok(seq);
            }
            seq.push(self.parse_statement()?);
            self.expect(TokenType::Semicolon)?;
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

                Ok(Value::Sequence(seq, line))
            }
            _ => self.error(&format!("Expected a value, got {}", self.peek().lexeme)),
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

/// Helper: parse source string into a Program
fn parse_str(source: &str) -> Result<Program, String> {
    use crate::parser::scanner::Scanner;
    let mut sc = Scanner::new(source);
    let tokens = sc.scan_tokens().unwrap();
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::scanner::Scanner;

    /// Helper: parse source string, expecting an error
    fn parse_err(source: &str) -> String {
        let mut sc = Scanner::new(source);
        let tokens = sc.scan_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_program().unwrap_err()
    }

    #[test]
    fn test_const_definition() {
        let program = parse_str("let X = 50;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Definition(Definition {
                name: "X".to_string(),
                params: vec![],
                value: Value::Constant(50, 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_procedure_definition() {
        let program = parse_str("let gather = [ hold W for 2000; ];").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Definition(Definition {
                name: "gather".to_string(),
                params: vec![],
                value: Value::Sequence(vec![
                    Stmt::Action(Action::Hold {
                        target: HoldTarget::Single(Expr::UpperIdent("W".to_string(), 1)),
                        duration: Expr::Number(2000, 1),
                        line: 1,
                    }),
                ], 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_set_hold_syntax() {
        let program = parse_str("let diagonal = hold { W, D };").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Definition(Definition {
                name: "diagonal".to_string(),
                params: vec![],
                value: Value::Set(vec![
                    Expr::UpperIdent("W".to_string(), 1),
                    Expr::UpperIdent("D".to_string(), 1),
                ], 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_set_brace_syntax() {
        let program = parse_str("let diagonal = { hold W, hold D };").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Definition(Definition {
                name: "diagonal".to_string(),
                params: vec![],
                value: Value::Set(vec![
                    Expr::UpperIdent("W".to_string(), 1),
                    Expr::UpperIdent("D".to_string(), 1),
                ], 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_parameterized_definition() {
        let program = parse_str("let strafe(key, dur) = [ hold key for dur; ];").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Definition(Definition {
                name: "strafe".to_string(),
                params: vec!["key".to_string(), "dur".to_string()],
                value: Value::Sequence(vec![
                    Stmt::Action(Action::Hold {
                        target: HoldTarget::Single(Expr::Identifier("key".to_string(), 1)),
                        duration: Expr::Identifier("dur".to_string(), 1),
                        line: 1,
                    }),
                ], 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_tap_action() {
        let program = parse_str("tap W;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Tap {
                key: Expr::UpperIdent("W".to_string(), 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_wait_action() {
        let program = parse_str("wait 100;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Wait {
                duration: Expr::Number(100, 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_move_action() {
        let program = parse_str("move 10 -5;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Move {
                x: 10, y: -5, line: 1,
            })],
        });
    }

    #[test]
    fn test_scroll_action() {
        let program = parse_str("scroll down 3;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Scroll {
                direction: Direction::Down,
                amount: 3,
                line: 1,
            })],
        });
    }

    #[test]
    fn test_run_action() {
        let program = parse_str("run gather;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Run {
                name: "gather".to_string(),
                args: vec![],
                line: 1,
            })],
        });
    }

    #[test]
    fn test_run_with_args() {
        let program = parse_str("run strafe(D, 5000);").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Run {
                name: "strafe".to_string(),
                args: vec![
                    Expr::UpperIdent("D".to_string(), 1),
                    Expr::Number(5000, 1),
                ],
                line: 1,
            })],
        });
    }

    #[test]
    fn test_use_with_duration() {
        let program = parse_str("diagonal for 1000;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::UseWithDuration {
                name: "diagonal".to_string(),
                args: vec![],
                duration: Expr::Number(1000, 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_use_with_duration_and_args() {
        let program = parse_str("strafe(D, 5000) for 100;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::UseWithDuration {
                name: "strafe".to_string(),
                args: vec![
                    Expr::UpperIdent("D".to_string(), 1),
                    Expr::Number(5000, 1),
                ],
                duration: Expr::Number(100, 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_hold_single_key() {
        let program = parse_str("hold W for 1000;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Hold {
                target: HoldTarget::Single(Expr::UpperIdent("W".to_string(), 1)),
                duration: Expr::Number(1000, 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_hold_inline_set() {
        let program = parse_str("hold { W, D } for 1000;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Hold {
                target: HoldTarget::InlineSet(vec![
                    Expr::UpperIdent("W".to_string(), 1),
                    Expr::UpperIdent("D".to_string(), 1),
                ]),
                duration: Expr::Number(1000, 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_brace_hold_action() {
        let program = parse_str("{ hold W, hold D } for 1000;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Hold {
                target: HoldTarget::InlineSet(vec![
                    Expr::UpperIdent("W".to_string(), 1),
                    Expr::UpperIdent("D".to_string(), 1),
                ]),
                duration: Expr::Number(1000, 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_trailing_comma_in_set() {
        let program = parse_str("hold { W, D, } for 100;").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Hold {
                target: HoldTarget::InlineSet(vec![
                    Expr::UpperIdent("W".to_string(), 1),
                    Expr::UpperIdent("D".to_string(), 1),
                ]),
                duration: Expr::Number(100, 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_trailing_comma_in_args() {
        let program = parse_str("run strafe(D, 5000,);").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Action(Action::Run {
                name: "strafe".to_string(),
                args: vec![
                    Expr::UpperIdent("D".to_string(), 1),
                    Expr::Number(5000, 1),
                ],
                line: 1,
            })],
        });
    }

    #[test]
    fn test_trailing_comma_in_params() {
        let program = parse_str("let strafe(key, dur,) = [ hold key for dur; ];").unwrap();
        assert_eq!(program, Program {
            statements: vec![Stmt::Definition(Definition {
                name: "strafe".to_string(),
                params: vec!["key".to_string(), "dur".to_string()],
                value: Value::Sequence(vec![
                    Stmt::Action(Action::Hold {
                        target: HoldTarget::Single(Expr::Identifier("key".to_string(), 1)),
                        duration: Expr::Identifier("dur".to_string(), 1),
                        line: 1,
                    }),
                ], 1),
                line: 1,
            })],
        });
    }

    #[test]
    fn test_multi_statement_program() {
        let program = parse_str("let X = 50;\nhold W for X;\nwait 100;").unwrap();
        assert_eq!(program.statements.len(), 3);
        assert!(matches!(program.statements[0], Stmt::Definition(_)));
        assert!(matches!(program.statements[1], Stmt::Action(Action::Hold { .. })));
        assert!(matches!(program.statements[2], Stmt::Action(Action::Wait { .. })));
    }

    #[test]
    fn test_error_missing_semicolon() {
        let err = parse_err("hold W for 100");
        assert!(err.contains("Expected"));
    }

    #[test]
    fn test_error_missing_for_in_hold() {
        let err = parse_err("hold W 100;");
        assert!(err.contains("Expected"));
    }

    #[test]
    fn test_error_invalid_action() {
        let err = parse_err("123;");
        assert!(err.contains("Expected"));
    }
}
