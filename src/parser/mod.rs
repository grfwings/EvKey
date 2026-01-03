//! EvScript parser module
//!
//! Implements lexing, parsing, and evaluation for EvScript v2.

pub mod scanner;
pub mod token;

pub use scanner::Scanner;
pub use token::{Token, TokenType};
