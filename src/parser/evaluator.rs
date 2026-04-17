//! Turns a parsed `Program` into a flat `Vec<MacroState>` that can be played
//! back via `state::states_to_events`.
//!
//! first register every `Definition`, then execute actions. This gives us forward references for free.

use std::collections::{HashMap, HashSet};

use crate::{
    keymap,
    parser::ast::{Action, Definition, Direction, Expr, HoldTarget, Program, SetAction, Stmt, Value},
    state::MacroState,
};

/// Default tap duration, per LANGUAGE.md.
const TAP_DURATION_MS: u64 = 50;

/// A `let` binding. We store the raw AST value so we can re-evaluate it with
/// fresh parameter bindings on every invocation.
#[derive(Debug, Clone)]
struct DefinitionEntry {
    params: Vec<String>,
    value: Value,
}

/// An argument after evaluation at a call site.
#[derive(Debug, Clone)]
enum ResolvedArg {
    Key(u16),
    Number(i64),
}

#[derive(Debug)]
struct Evaluator {
    /// Lexical scope stack for `let` definitions.
    scopes: Vec<HashMap<String, DefinitionEntry>>,
    /// Parallel stack for parameter -> argument bindings. A separate stack
    /// (rather than sharing `scopes`) keeps the two namespaces cleanly apart.
    param_bindings: Vec<HashMap<String, ResolvedArg>>,
    /// Names currently mid-expansion, for recursion detection.
    evaluating: HashSet<String>,
}

impl Evaluator {
    fn new() -> Self {
        Evaluator {
            scopes: vec![HashMap::new()],
            param_bindings: vec![HashMap::new()],
            evaluating: HashSet::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.param_bindings.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.param_bindings.pop();
        self.scopes.pop();
    }

    fn lookup_scope(&self, name: &str) -> Option<&DefinitionEntry> {
        lookup_in_stack(&self.scopes, name)
    }

    fn lookup_binding(&self, name: &str) -> Option<&ResolvedArg> {
        lookup_in_stack(&self.param_bindings, name)
    }

    fn define(&mut self, name: String, line: usize, entry: DefinitionEntry) -> Result<(), String> {
        let current = self
            .scopes
            .last_mut()
            .ok_or_else(|| "internal error: no active scope".to_string())?;
        if current.contains_key(&name) {
            return Err(format!(
                "line {}: `{}` is already defined in this scope",
                line, name
            ));
        }
        current.insert(name, entry);
        Ok(())
    }

    /// Resolve an expression to a keycode.
    ///
    /// Per LANGUAGE.md, local definitions may shadow key names, so we check
    /// the scope stack *before* the keymap. If a shadowing `let` exists it's
    /// necessarily not a key (constants are numbers, sets/procs are not keys),
    /// so that's an error.
    fn resolve_key(&self, expr: &Expr) -> Result<u16, String> {
        match expr {
            Expr::UpperIdent(name, line) => {
                if let Some(entry) = self.lookup_scope(name) {
                    return match &entry.value {
                        Value::Constant(_, _) => Err(format!(
                            "line {}: expected a key, but `{}` is a constant",
                            line, name
                        )),
                        Value::Set(_, _) | Value::Sequence(_, _) => Err(format!(
                            "line {}: expected a key, but `{}` is a definition",
                            line, name
                        )),
                    };
                }
                lookup_key_name(name)
                    .ok_or_else(|| format!("line {}: unknown key `{}`", line, name))
            }
            Expr::Identifier(name, line) => match self.lookup_binding(name) {
                Some(ResolvedArg::Key(k)) => Ok(*k),
                Some(ResolvedArg::Number(_)) => Err(format!(
                    "line {}: expected a key, but parameter `{}` is a number",
                    line, name
                )),
                None => Err(format!(
                    "line {}: `{}` is not a parameter in scope",
                    line, name
                )),
            },
            Expr::Number(n, line) => {
                Err(format!("line {}: expected a key, got number {}", line, n))
            }
        }
    }

    /// Resolve an expression to a number (for durations / numeric args).
    fn resolve_duration(&self, expr: &Expr) -> Result<i64, String> {
        match expr {
            Expr::Number(n, _) => Ok(*n),
            Expr::Identifier(name, line) => match self.lookup_binding(name) {
                Some(ResolvedArg::Number(n)) => Ok(*n),
                Some(ResolvedArg::Key(_)) => Err(format!(
                    "line {}: expected a number, but parameter `{}` is a key",
                    line, name
                )),
                None => Err(format!(
                    "line {}: `{}` is not a parameter in scope",
                    line, name
                )),
            },
            Expr::UpperIdent(name, line) => {
                if let Some(entry) = self.lookup_scope(name) {
                    match &entry.value {
                        Value::Constant(n, _) => Ok(*n),
                        _ => Err(format!(
                            "line {}: `{}` is not a numeric constant",
                            line, name
                        )),
                    }
                } else {
                    Err(format!("line {}: unknown constant `{}`", line, name))
                }
            }
        }
    }

    /// Resolve an argument at a call site (could be either a key or a number,
    /// depending on what the definition uses it for).
    fn resolve_arg(&self, expr: &Expr) -> Result<ResolvedArg, String> {
        match expr {
            Expr::Number(n, _) => Ok(ResolvedArg::Number(*n)),
            Expr::UpperIdent(name, line) => {
                // Scope wins over the keymap (shadowing is allowed).
                if let Some(entry) = self.lookup_scope(name) {
                    return match &entry.value {
                        Value::Constant(n, _) => Ok(ResolvedArg::Number(*n)),
                        Value::Set(_, _) | Value::Sequence(_, _) => Err(format!(
                            "line {}: `{}` is a definition and cannot be passed as an argument",
                            line, name
                        )),
                    };
                }
                lookup_key_name(name)
                    .map(ResolvedArg::Key)
                    .ok_or_else(|| format!("line {}: unknown key or constant `{}`", line, name))
            }
            Expr::Identifier(name, line) => {
                if let Some(arg) = self.lookup_binding(name) {
                    return Ok(arg.clone());
                }
                Err(format!(
                    "line {}: `{}` is not a parameter in scope",
                    line, name
                ))
            }
        }
    }

    /// Pass 1: register every top-level `Definition` in `stmts` into the
    /// current scope. Must run before exec_stmts so forward refs work.
    fn register_defs(&mut self, stmts: &[Stmt]) -> Result<(), String> {
        for s in stmts {
            if let Stmt::Definition(Definition {
                name,
                params,
                value,
                line,
            }) = s
            {
                self.define(
                    name.clone(),
                    *line,
                    DefinitionEntry {
                        params: params.clone(),
                        value: value.clone(),
                    },
                )?;
            }
        }
        Ok(())
    }

    fn exec_stmt(&mut self, stmt: &Stmt, out: &mut Vec<MacroState>) -> Result<(), String> {
        match stmt {
            // Definitions were already registered by register_defs.
            Stmt::Definition(_) => Ok(()),
            Stmt::Action(action) => self.exec_action(action, out),
        }
    }

    fn exec_action(&mut self, action: &Action, out: &mut Vec<MacroState>) -> Result<(), String> {
        match action {
            Action::Hold {
                target, duration, ..
            } => {
                let dur = self.resolve_duration_nonneg(duration)?;
                let mut state = self.resolve_hold_target(target)?;
                state.duration_ms = dur;
                out.push(state);
                Ok(())
            }
            Action::Tap { key, .. } => {
                let k = self.resolve_key(key)?;
                let mut s = MacroState::new(TAP_DURATION_MS);
                s.keys_pressed.insert(k);
                out.push(s);
                Ok(())
            }
            Action::Wait { duration, .. } => {
                let dur = self.resolve_duration_nonneg(duration)?;
                out.push(MacroState::new(dur));
                Ok(())
            }
            Action::Move { x, y, line } => {
                let x = i32::try_from(*x)
                    .map_err(|_| format!("line {}: move x {} is out of i32 range", line, x))?;
                let y = i32::try_from(*y)
                    .map_err(|_| format!("line {}: move y {} is out of i32 range", line, y))?;
                let mut s = MacroState::new(0);
                s.mouse_delta = (x, y);
                out.push(s);
                Ok(())
            }
            Action::Scroll {
                direction,
                amount,
                line,
            } => {
                if *amount < 0 {
                    return Err(format!(
                        "line {}: scroll amount must be non-negative (use direction instead)",
                        line
                    ));
                }
                let amt = *amount as i32;
                let mut s = MacroState::new(0);
                // scroll_delta = (vertical, horizontal) per state.rs.
                // REL_WHEEL:  positive = up,    negative = down
                // REL_HWHEEL: positive = right, negative = left
                s.scroll_delta = match direction {
                    Direction::Up => (amt, 0),
                    Direction::Down => (-amt, 0),
                    Direction::Right => (0, amt),
                    Direction::Left => (0, -amt),
                };
                out.push(s);
                Ok(())
            }
            Action::Run { name, args, line } => self.invoke(name, args, None, *line, out),
            Action::UseWithDuration {
                name,
                args,
                duration,
                line,
            } => {
                let dur = self.resolve_duration_nonneg(duration)?;
                self.invoke(name, args, Some(dur), *line, out)
            }
        }
    }

    /// Resolve a hold target into a `MacroState` (without duration set).
    fn resolve_hold_target(&self, target: &HoldTarget) -> Result<MacroState, String> {
        match target {
            HoldTarget::Single(e) => {
                let mut s = MacroState::new(0);
                s.keys_pressed.insert(self.resolve_key(e)?);
                Ok(s)
            }
            HoldTarget::InlineSet(actions) => self.resolve_set_actions(actions),
        }
    }

    /// Resolve a list of `SetAction`s into a single `MacroState` (without duration).
    fn resolve_set_actions(&self, actions: &[SetAction]) -> Result<MacroState, String> {
        let mut state = MacroState::new(0);
        for action in actions {
            match action {
                SetAction::Hold(expr) => {
                    state.keys_pressed.insert(self.resolve_key(expr)?);
                }
                SetAction::Move(x, y, line) => {
                    let x = i32::try_from(*x)
                        .map_err(|_| format!("line {}: move x {} is out of i32 range", line, x))?;
                    let y = i32::try_from(*y)
                        .map_err(|_| format!("line {}: move y {} is out of i32 range", line, y))?;
                    state.mouse_delta.0 += x;
                    state.mouse_delta.1 += y;
                }
                SetAction::Scroll(direction, amount, line) => {
                    if *amount < 0 {
                        return Err(format!(
                            "line {}: scroll amount must be non-negative",
                            line
                        ));
                    }
                    let amt = *amount as i32;
                    match direction {
                        Direction::Up => state.scroll_delta.0 += amt,
                        Direction::Down => state.scroll_delta.0 -= amt,
                        Direction::Right => state.scroll_delta.1 += amt,
                        Direction::Left => state.scroll_delta.1 -= amt,
                    }
                }
            }
        }
        Ok(state)
    }

    /// Wraps `resolve_duration` and rejects negatives (durations are >= 0).
    fn resolve_duration_nonneg(&self, expr: &Expr) -> Result<u64, String> {
        let n = self.resolve_duration(expr)?;
        if n < 0 {
            let line = match expr {
                Expr::Number(_, l) | Expr::Identifier(_, l) | Expr::UpperIdent(_, l) => *l,
            };
            return Err(format!("line {}: duration must be non-negative, got {}", line, n));
        }
        Ok(n as u64)
    }

    /// Invoke a named definition. `duration = Some(d)` means we're at a
    /// `NAME(..) for d` site (sets only). `None` means `run NAME(..)`
    /// (procedures only).
    fn invoke(
        &mut self,
        name: &str,
        args: &[Expr],
        duration: Option<u64>,
        line: usize,
        out: &mut Vec<MacroState>,
    ) -> Result<(), String> {
        if self.evaluating.contains(name) {
            return Err(format!("line {}: recursion detected via `{}`", line, name));
        }

        // Clone before pushing a new scope: lookup borrows `self` immutably and
        // `push_scope` takes a mutable borrow, so we cannot hold the reference across.
        let entry = self
            .lookup_scope(name)
            .ok_or_else(|| format!("line {}: unknown name `{}`", line, name))?
            .clone();

        if entry.params.len() != args.len() {
            return Err(format!(
                "line {}: `{}` expects {} argument(s), got {}",
                line,
                name,
                entry.params.len(),
                args.len()
            ));
        }

        // Evaluate args in the *caller's* environment before we push a frame.
        let resolved_args: Vec<ResolvedArg> = args
            .iter()
            .map(|e| self.resolve_arg(e))
            .collect::<Result<_, _>>()?;

        self.evaluating.insert(name.to_string());
        self.push_scope();

        {
            let frame = self.param_bindings.last_mut().unwrap();
            for (p, a) in entry.params.iter().zip(resolved_args.into_iter()) {
                frame.insert(p.clone(), a);
            }
        }

        // Do the work inside a closure so `?` early-returns don't skip cleanup.
        let result = (|| -> Result<(), String> {
            match &entry.value {
                Value::Constant(_, _) => Err(format!(
                    "line {}: `{}` is a numeric constant and cannot be used as an action",
                    line, name
                )),
                Value::Set(actions, _) => {
                    let dur = duration.ok_or_else(|| {
                        format!(
                            "line {}: set `{}` must be followed by `for <duration>`",
                            line, name
                        )
                    })?;
                    let mut state = self.resolve_set_actions(actions)?;
                    state.duration_ms = dur;
                    out.push(state);
                    Ok(())
                }
                Value::Sequence(stmts, _) => {
                    if duration.is_some() {
                        return Err(format!(
                            "line {}: procedure `{}` cannot be used with `for`; use `run`",
                            line, name
                        ));
                    }
                    // Register local defs first so procedures inside the body
                    // can forward-reference each other.
                    self.register_defs(stmts)?;
                    for s in stmts {
                        self.exec_stmt(s, out)?;
                    }
                    Ok(())
                }
            }
        })();

        self.pop_scope();
        self.evaluating.remove(name);
        result
    }
}

/// Try to parse a `KEY_<N>` style identifier into a keycode.
fn parse_key_numeric(name: &str) -> Option<u16> {
    name.strip_prefix("KEY_")
        .and_then(|n| n.parse::<u16>().ok())
}

/// Resolve a key name: keymap first, then `KEY_<N>` numeric fallback.
fn lookup_key_name(name: &str) -> Option<u16> {
    keymap::name_to_keycode(name).or_else(|| parse_key_numeric(name))
}

/// Linear scan of a scope stack, searching innermost (last) frame first.
fn lookup_in_stack<'a, V>(stack: &'a [HashMap<String, V>], name: &str) -> Option<&'a V> {
    stack.iter().rev().find_map(|m| m.get(name))
}

/// Public entry point: evaluate a parsed program into a flat sequence of
/// `MacroState`s.
pub fn evaluate(program: &Program) -> Result<Vec<MacroState>, String> {
    let mut ev = Evaluator::new();
    ev.register_defs(&program.statements)?;
    let mut states = Vec::new();
    for s in &program.statements {
        ev.exec_stmt(s, &mut states)?;
    }
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_source(source: &str) -> Result<Vec<MacroState>, String> {
        let prog = crate::parser::parser::parse_str(source)?;
        evaluate(&prog)
    }

    #[test]
    fn test_hold_single_key() {
        let states = run_source("hold W for 100;").unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 100);
        assert!(states[0].keys_pressed.contains(&17)); // W
    }

    #[test]
    fn test_tap_uses_default_duration() {
        let states = run_source("tap W;").unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, TAP_DURATION_MS);
    }

    #[test]
    fn test_wait() {
        let states = run_source("wait 250;").unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 250);
        assert!(states[0].keys_pressed.is_empty());
    }

    #[test]
    fn test_inline_hold_set() {
        let states = run_source("hold { W, A } for 100;").unwrap();
        assert_eq!(states.len(), 1);
        assert!(states[0].keys_pressed.contains(&17)); // W
        assert!(states[0].keys_pressed.contains(&30)); // A
    }

    #[test]
    fn test_named_set_for_duration() {
        let src = "\
            let diag = hold { W, A };\n\
            diag for 500;\n";
        let states = run_source(src).unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 500);
        assert!(states[0].keys_pressed.contains(&17));
        assert!(states[0].keys_pressed.contains(&30));
    }

    #[test]
    fn test_procedure_expansion() {
        let src = "\
            let p = [ hold W for 100; wait 50; tap A; ];\n\
            run p;\n";
        let states = run_source(src).unwrap();
        assert_eq!(states.len(), 3);
        assert_eq!(states[0].duration_ms, 100);
        assert_eq!(states[1].duration_ms, 50);
        assert_eq!(states[2].duration_ms, TAP_DURATION_MS);
    }

    #[test]
    fn test_constants_as_durations() {
        let src = "\
            let T = 750;\n\
            hold W for T;\n";
        let states = run_source(src).unwrap();
        assert_eq!(states[0].duration_ms, 750);
    }

    #[test]
    fn test_parameterized_procedure_key_and_duration() {
        let src = "\
            let strafe(k, d) = [ hold k for d; ];\n\
            run strafe(D, 400);\n";
        let states = run_source(src).unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 400);
        assert!(states[0].keys_pressed.contains(&32)); // D
    }

    #[test]
    fn test_parameterized_set() {
        let src = "\
            let combo(a, b) = hold { a, b };\n\
            combo(W, A) for 250;\n";
        let states = run_source(src).unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 250);
        assert!(states[0].keys_pressed.contains(&17));
        assert!(states[0].keys_pressed.contains(&30));
    }

    #[test]
    fn test_forward_reference() {
        let src = "\
            run p;\n\
            let p = [ hold W for 10; ];\n";
        let states = run_source(src).unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 10);
    }

    #[test]
    fn test_scroll_directions() {
        let states = run_source("scroll up 3;").unwrap();
        assert_eq!(states[0].scroll_delta, (3, 0));
        let states = run_source("scroll down 2;").unwrap();
        assert_eq!(states[0].scroll_delta, (-2, 0));
        let states = run_source("scroll right 4;").unwrap();
        assert_eq!(states[0].scroll_delta, (0, 4));
        let states = run_source("scroll left 1;").unwrap();
        assert_eq!(states[0].scroll_delta, (0, -1));
    }

    #[test]
    fn test_move_action() {
        let states = run_source("move 10 -5;").unwrap();
        assert_eq!(states[0].mouse_delta, (10, -5));
        assert_eq!(states[0].duration_ms, 0);
    }

    #[test]
    fn test_error_recursion_direct() {
        let src = "\
            let loopy = [ run loopy; ];\n\
            run loopy;\n";
        let err = run_source(src).unwrap_err();
        assert!(err.contains("recursion"), "got: {err}");
    }

    #[test]
    fn test_error_recursion_indirect() {
        let src = "\
            let a = [ run b; ];\n\
            let b = [ run a; ];\n\
            run a;\n";
        let err = run_source(src).unwrap_err();
        assert!(err.contains("recursion"), "got: {err}");
    }

    #[test]
    fn test_error_wrong_arg_count() {
        let src = "\
            let p(k) = [ hold k for 10; ];\n\
            run p;\n";
        let err = run_source(src).unwrap_err();
        assert!(err.contains("expects"), "got: {err}");
    }

    #[test]
    fn test_error_number_passed_as_key() {
        let src = "\
            let p(k) = [ hold k for 10; ];\n\
            run p(500);\n";
        let err = run_source(src).unwrap_err();
        assert!(err.contains("key"), "got: {err}");
    }

    #[test]
    fn test_error_set_without_duration() {
        let src = "\
            let s = hold { W, A };\n\
            run s;\n";
        let err = run_source(src).unwrap_err();
        assert!(
            err.contains("for") || err.contains("duration"),
            "got: {err}"
        );
    }

    #[test]
    fn test_error_unknown_name() {
        let err = run_source("run nope;").unwrap_err();
        assert!(err.contains("unknown"), "got: {err}");
    }

    #[test]
    fn test_shadow_key_name_with_constant() {
        // LANGUAGE.md: local defs may shadow global defs, including key names.
        // Inside the procedure, `W` refers to the constant 100, not keycode 17.
        // So this procedure holds the *A* key for 100ms (W is used as duration).
        let src = "\
            let p = [ let W = 100; hold A for W; ];\n\
            run p;\n";
        let states = run_source(src).unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 100);
        assert!(states[0].keys_pressed.contains(&30)); // A
        assert!(!states[0].keys_pressed.contains(&17)); // not W
    }

    #[test]
    fn test_shadow_key_name_then_used_as_key_errors() {
        // After shadowing W with a constant, trying to *hold* W is a type error.
        let src = "\
            let p = [ let W = 100; hold W for 50; ];\n\
            run p;\n";
        let err = run_source(src).unwrap_err();
        assert!(err.contains("constant") || err.contains("key"), "got: {err}");
    }

    #[test]
    fn test_shadow_does_not_leak_out_of_scope() {
        // After the procedure returns, W should be a key again at the top level.
        let src = "\
            let p = [ let W = 100; wait W; ];\n\
            run p;\n\
            hold W for 25;\n";
        let states = run_source(src).unwrap();
        assert_eq!(states.len(), 2);
        assert_eq!(states[0].duration_ms, 100);
        assert!(states[0].keys_pressed.is_empty()); // wait state
        assert_eq!(states[1].duration_ms, 25);
        assert!(states[1].keys_pressed.contains(&17)); // W keycode restored
    }

    #[test]
    fn test_error_duplicate_definition() {
        let src = "\
            let x = 1;\n\
            let x = 2;\n";
        let err = run_source(src).unwrap_err();
        assert!(err.contains("already defined"), "got: {err}");
    }

    // --- Simultaneous actions (extended sets) ---

    #[test]
    fn test_simultaneous_key_and_mouse() {
        let states = run_source("{ hold W, move 10 -5 } for 100;").unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 100);
        assert!(states[0].keys_pressed.contains(&17)); // W
        assert_eq!(states[0].mouse_delta, (10, -5));
    }

    #[test]
    fn test_simultaneous_key_and_scroll() {
        let states = run_source("{ hold W, scroll down 3 } for 200;").unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 200);
        assert!(states[0].keys_pressed.contains(&17));
        assert_eq!(states[0].scroll_delta, (-3, 0));
    }

    #[test]
    fn test_simultaneous_key_mouse_scroll() {
        let states = run_source(
            "{ hold W, hold BTN_LEFT, move 10 -5, scroll down 1 } for 50;",
        )
        .unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 50);
        assert!(states[0].keys_pressed.contains(&17)); // W
        assert!(states[0].keys_pressed.contains(&272)); // BTN_LEFT
        assert_eq!(states[0].mouse_delta, (10, -5));
        assert_eq!(states[0].scroll_delta, (-1, 0));
    }

    // --- KEY_<N> fallback ---

    #[test]
    fn test_key_numeric_hold() {
        let states = run_source("hold KEY_412 for 50;").unwrap();
        assert_eq!(states.len(), 1);
        assert!(states[0].keys_pressed.contains(&412));
    }

    #[test]
    fn test_key_numeric_tap() {
        let states = run_source("tap KEY_412;").unwrap();
        assert_eq!(states.len(), 1);
        assert!(states[0].keys_pressed.contains(&412));
    }

    #[test]
    fn test_key_numeric_in_set() {
        let states = run_source("{ hold KEY_412, hold W } for 100;").unwrap();
        assert_eq!(states.len(), 1);
        assert!(states[0].keys_pressed.contains(&412));
        assert!(states[0].keys_pressed.contains(&17));
    }

    #[test]
    fn test_key_numeric_as_arg() {
        let src = "\
            let p(k) = [ hold k for 10; ];\n\
            run p(KEY_412);\n";
        let states = run_source(src).unwrap();
        assert!(states[0].keys_pressed.contains(&412));
    }

    #[test]
    fn test_named_set_with_move() {
        let src = "\
            let attack = { hold W, move 10 0 };\n\
            attack for 300;\n";
        let states = run_source(src).unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 300);
        assert!(states[0].keys_pressed.contains(&17));
        assert_eq!(states[0].mouse_delta, (10, 0));
    }

}
