//! AST types for EvScript v2.
//!
//! Each type maps to a grammar production from LANGUAGE.md.
//! The evaluator resolves `Expr` nodes to concrete values (keys vs constants)
//! in a second pass, since the parser can't distinguish them syntactically.

/// A complete EvScript program.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

/// A statement, either a definition or an action.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Definition(Definition),
    Action(Action),
}

/// `let name(params) = value;`
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub name: String,
    pub params: Vec<String>,
    pub value: Value,
    pub line: usize,
}

/// Righthand side of a definition.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Numeric constant: `let X = 50;`
    Constant(i64, usize),
    /// Set of simultaneous actions: `hold { W, S }` or `{ hold W, move 10 -5 }`
    Set(Vec<SetAction>, usize),
    /// Procedure body: `[ stmt; stmt; ]`
    Sequence(Vec<Stmt>, usize),
}

/// An executable action.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// `hold W for 1000` or `hold { W, S } for 1000`
    Hold {
        target: HoldTarget,
        duration: Expr,
        line: usize,
    },
    /// `tap W`
    Tap { key: Expr, line: usize },
    /// `wait 1000`
    Wait { duration: Expr, line: usize },
    /// `move 10 -5`
    Move { x: i64, y: i64, line: usize },
    /// `scroll down 3`
    Scroll {
        direction: Direction,
        amount: i64,
        line: usize,
    },
    /// `run gather` or `run strafe(D, 5000)`
    Run {
        name: String,
        args: Vec<Expr>,
        line: usize,
    },
    /// `diagonal for 1000` or `my_set(W, S) for 500`
    UseWithDuration {
        name: String,
        args: Vec<Expr>,
        duration: Expr,
        line: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum HoldTarget {
    /// Single key or parameter: `hold W`, `hold my_param`
    Single(Expr),
    /// Inline set: `hold { W, S }` or `{ hold W, move 10 -5 }`
    InlineSet(Vec<SetAction>),
}

/// An expression resolved to a concrete value by the evaluator.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Literal number: `1000`, `-50`
    Number(i64, usize),
    /// Lowercase identifier (parameter or name reference): `duration`, `my_proc`
    Identifier(String, usize),
    /// Uppercase identifier (key or constant disambiguated by evaluator): `W`, `TAP_TIME`
    UpperIdent(String, usize),
}

/// Scroll direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// An action inside a set (simultaneous actions).
#[derive(Debug, Clone, PartialEq)]
pub enum SetAction {
    /// `hold W` or `hold param`
    Hold(Expr),
    /// `move 10 -5`
    Move(i64, i64, usize),
    /// `scroll down 3`
    Scroll(Direction, i64, usize),
}
