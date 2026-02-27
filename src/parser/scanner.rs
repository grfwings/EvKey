//! Lexical scanner for EvScript

use std::collections::HashMap;

use super::token::{Token, TokenType};

pub struct Scanner {
    source: Vec<char>,
    tokens: Vec<Token>,
    start: usize,
    current: usize,
    line: usize,
    keywords: HashMap<&'static str, TokenType>,
    errors: Vec<String>,
}

impl Scanner {
    pub fn new(source: &str) -> Self {
        let keywords = HashMap::from([
            ("let", TokenType::Let),
            ("hold", TokenType::Hold),
            ("tap", TokenType::Tap),
            ("wait", TokenType::Wait),
            ("move", TokenType::Move),
            ("scroll", TokenType::Scroll),
            ("run", TokenType::Run),
            ("for", TokenType::For),
            ("up", TokenType::Up),
            ("down", TokenType::Down),
            ("left", TokenType::Left),
            ("right", TokenType::Right),
        ]);

        Self {
            source: source.chars().collect(),
            tokens: Vec::new(),
            start: 0,
            current: 0,
            line: 1,
            keywords,
            errors: Vec::new(),
        }
    }

    pub fn scan_tokens(&mut self) -> Result<Vec<Token>, Vec<String>> {
        while !self.is_at_end() {
            self.start = self.current;
            self.scan_token();
        }

        self.tokens
            .push(Token::new(TokenType::Eof, String::new(), None, self.line));

        if self.errors.is_empty() {
            Ok(self.tokens.clone())
        } else {
            Err(self.errors.clone())
        }
    }

    fn scan_token(&mut self) {
        let c = self.advance();

        match c {
            '(' => self.add_token(TokenType::LeftParen),
            ')' => self.add_token(TokenType::RightParen),
            '{' => self.add_token(TokenType::LeftBrace),
            '}' => self.add_token(TokenType::RightBrace),
            '[' => self.add_token(TokenType::LeftBracket),
            ']' => self.add_token(TokenType::RightBracket),
            ',' => self.add_token(TokenType::Comma),
            ';' => self.add_token(TokenType::Semicolon),
            '=' => self.add_token(TokenType::Equals),

            // Comments
            '/' => {
                if self.match_char('/') {
                    // Comment goes until end of line
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                } else {
                    self.error("Unexpected character '/'");
                }
            }

            // Whitespace
            ' ' | '\r' | '\t' => {}
            '\n' => self.line += 1,

            // Numbers (including negative)
            '-' => {
                if self.peek().is_ascii_digit() {
                    self.number();
                } else {
                    self.error("Unexpected character '-' (expected digit after)");
                }
            }

            _ => {
                if c.is_ascii_digit() {
                    self.number();
                } else if c.is_ascii_uppercase() || c == '_' {
                    self.upper_identifier();
                } else if c.is_ascii_lowercase() {
                    self.identifier();
                } else {
                    self.error(&format!("Unexpected character '{}'", c));
                }
            }
        }
    }

    fn number(&mut self) {
        while self.peek().is_ascii_digit() {
            self.advance();
        }

        let lexeme = self.current_lexeme();
        match lexeme.parse::<i64>() {
            Ok(value) => self.add_token_literal(TokenType::Number, Some(value)),
            Err(_) => self.error(&format!("Invalid number: {}", lexeme)),
        }
    }

    fn identifier(&mut self) {
        while self.peek().is_ascii_lowercase() || self.peek().is_ascii_digit() || self.peek() == '_'
        {
            self.advance();
        }

        let lexeme = self.current_lexeme();

        // Check if it's a keyword
        let token_type = self
            .keywords
            .get(lexeme.as_str())
            .copied()
            .unwrap_or(TokenType::Identifier);

        self.add_token(token_type);
    }

    fn upper_identifier(&mut self) {
        while self.peek().is_ascii_uppercase() || self.peek().is_ascii_digit() || self.peek() == '_'
        {
            self.advance();
        }

        self.add_token(TokenType::UpperIdent);
    }

    // Helper methods

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current];
        self.current += 1;
        c
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.source[self.current]
        }
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.source[self.current] != expected {
            false
        } else {
            self.current += 1;
            true
        }
    }

    fn current_lexeme(&self) -> String {
        self.source[self.start..self.current].iter().collect()
    }

    fn add_token(&mut self, token_type: TokenType) {
        self.add_token_literal(token_type, None);
    }

    fn add_token_literal(&mut self, token_type: TokenType, literal: Option<i64>) {
        let lexeme = self.current_lexeme();
        self.tokens
            .push(Token::new(token_type, lexeme, literal, self.line));
    }

    fn error(&mut self, message: &str) {
        self.errors
            .push(format!("[line {}] Error: {}", self.line, message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_chars() {
        let mut scanner = Scanner::new("(){}[],;=");
        let tokens = scanner.scan_tokens().unwrap();

        assert_eq!(tokens[0].token_type, TokenType::LeftParen);
        assert_eq!(tokens[1].token_type, TokenType::RightParen);
        assert_eq!(tokens[2].token_type, TokenType::LeftBrace);
        assert_eq!(tokens[3].token_type, TokenType::RightBrace);
        assert_eq!(tokens[4].token_type, TokenType::LeftBracket);
        assert_eq!(tokens[5].token_type, TokenType::RightBracket);
        assert_eq!(tokens[6].token_type, TokenType::Comma);
        assert_eq!(tokens[7].token_type, TokenType::Semicolon);
        assert_eq!(tokens[8].token_type, TokenType::Equals);
        assert_eq!(tokens[9].token_type, TokenType::Eof);
    }

    #[test]
    fn test_keywords() {
        let mut scanner = Scanner::new("let hold tap wait move scroll run for up down left right");
        let tokens = scanner.scan_tokens().unwrap();

        assert_eq!(tokens[0].token_type, TokenType::Let);
        assert_eq!(tokens[1].token_type, TokenType::Hold);
        assert_eq!(tokens[2].token_type, TokenType::Tap);
        assert_eq!(tokens[3].token_type, TokenType::Wait);
        assert_eq!(tokens[4].token_type, TokenType::Move);
        assert_eq!(tokens[5].token_type, TokenType::Scroll);
        assert_eq!(tokens[6].token_type, TokenType::Run);
        assert_eq!(tokens[7].token_type, TokenType::For);
        assert_eq!(tokens[8].token_type, TokenType::Up);
        assert_eq!(tokens[9].token_type, TokenType::Down);
        assert_eq!(tokens[10].token_type, TokenType::Left);
        assert_eq!(tokens[11].token_type, TokenType::Right);
    }

    #[test]
    fn test_identifiers() {
        let mut scanner = Scanner::new("my_proc key duration test123");
        let tokens = scanner.scan_tokens().unwrap();

        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].lexeme, "my_proc");
        assert_eq!(tokens[1].token_type, TokenType::Identifier);
        assert_eq!(tokens[2].token_type, TokenType::Identifier);
        assert_eq!(tokens[3].token_type, TokenType::Identifier);
    }

    #[test]
    fn test_upper_identifiers() {
        let mut scanner = Scanner::new("W SPACE BTN_LEFT MY_CONST");
        let tokens = scanner.scan_tokens().unwrap();

        assert_eq!(tokens[0].token_type, TokenType::UpperIdent);
        assert_eq!(tokens[0].lexeme, "W");
        assert_eq!(tokens[1].token_type, TokenType::UpperIdent);
        assert_eq!(tokens[1].lexeme, "SPACE");
        assert_eq!(tokens[2].token_type, TokenType::UpperIdent);
        assert_eq!(tokens[3].token_type, TokenType::UpperIdent);
    }

    #[test]
    fn test_numbers() {
        let mut scanner = Scanner::new("123 -50 0");
        let tokens = scanner.scan_tokens().unwrap();

        assert_eq!(tokens[0].token_type, TokenType::Number);
        assert_eq!(tokens[0].literal, Some(123));
        assert_eq!(tokens[1].token_type, TokenType::Number);
        assert_eq!(tokens[1].literal, Some(-50));
        assert_eq!(tokens[2].token_type, TokenType::Number);
        assert_eq!(tokens[2].literal, Some(0));
    }

    #[test]
    fn test_comments() {
        let mut scanner = Scanner::new("hold W // this is a comment\nwait 100");
        let tokens = scanner.scan_tokens().unwrap();

        assert_eq!(tokens[0].token_type, TokenType::Hold);
        assert_eq!(tokens[1].token_type, TokenType::UpperIdent);
        assert_eq!(tokens[2].token_type, TokenType::Wait);
        assert_eq!(tokens[3].token_type, TokenType::Number);
    }

    #[test]
    fn test_full_statement() {
        let mut scanner = Scanner::new("let gather = [ hold W for 2000; ];");
        let tokens = scanner.scan_tokens().unwrap();

        assert_eq!(tokens[0].token_type, TokenType::Let);
        assert_eq!(tokens[1].token_type, TokenType::Identifier);
        assert_eq!(tokens[1].lexeme, "gather");
        assert_eq!(tokens[2].token_type, TokenType::Equals);
        assert_eq!(tokens[3].token_type, TokenType::LeftBracket);
        assert_eq!(tokens[4].token_type, TokenType::Hold);
        assert_eq!(tokens[5].token_type, TokenType::UpperIdent);
        assert_eq!(tokens[6].token_type, TokenType::For);
        assert_eq!(tokens[7].token_type, TokenType::Number);
        assert_eq!(tokens[8].token_type, TokenType::Semicolon);
        assert_eq!(tokens[9].token_type, TokenType::RightBracket);
        assert_eq!(tokens[10].token_type, TokenType::Semicolon);
    }

    #[test]
    fn test_line_tracking() {
        let mut scanner = Scanner::new("hold W\nwait 100\nscroll down 1");
        let tokens = scanner.scan_tokens().unwrap();

        assert_eq!(tokens[0].line, 1); // hold
        assert_eq!(tokens[1].line, 1); // W
        assert_eq!(tokens[2].line, 2); // wait
        assert_eq!(tokens[3].line, 2); // 100
        assert_eq!(tokens[4].line, 3); // scroll
    }

    #[test]
    fn test_error_lone_slash() {
        let mut scanner = Scanner::new("/");
        let result = scanner.scan_tokens();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].contains("Unexpected character '/'"));
    }

    #[test]
    fn test_error_lone_minus() {
        let mut scanner = Scanner::new("-");
        let result = scanner.scan_tokens();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].contains("Unexpected character '-'"));
    }

    #[test]
    fn test_error_unexpected_chars() {
        let mut scanner = Scanner::new("@");
        let result = scanner.scan_tokens();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("Unexpected character '@'"));

        let mut scanner = Scanner::new("#");
        let result = scanner.scan_tokens();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("Unexpected character '#'"));
    }

    #[test]
    fn test_error_overflow_number() {
        // A number too large for i64
        let mut scanner = Scanner::new("99999999999999999999999999999");
        let result = scanner.scan_tokens();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("Invalid number"));
    }

    #[test]
    fn test_error_number_no_token_emitted() {
        // On parse failure, no Number token should be emitted
        let mut scanner = Scanner::new("99999999999999999999999999999");
        let result = scanner.scan_tokens();
        assert!(result.is_err());
    }
}
