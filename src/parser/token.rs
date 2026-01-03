//! Token types and structures for EvScript lexer

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    // Single-character tokens
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Semicolon,
    Equals,

    // Keywords
    Let,
    Hold,
    Tap,
    Wait,
    Move,
    Scroll,
    Run,
    For,

    // Scroll directions
    Up,
    Down,
    Left,
    Right,

    // Literals and identifiers
    Number,      // 123, -50
    Identifier,  // lowercase: my_proc, key, duration
    UpperIdent,  // uppercase: W, SPACE, BTN_LEFT, MY_CONST

    // Special
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub literal: Option<i64>,
    pub line: usize,
}

impl Token {
    pub fn new(token_type: TokenType, lexeme: String, literal: Option<i64>, line: usize) -> Self {
        Self {
            token_type,
            lexeme,
            literal,
            line,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.literal {
            Some(lit) => write!(f, "{:?} {} {}", self.token_type, self.lexeme, lit),
            None => write!(f, "{:?} {}", self.token_type, self.lexeme),
        }
    }
}
