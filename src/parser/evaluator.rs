//! Evaluator for EvScript

use std::collections::{HashMap, HashSet};

use crate::parser::ast::Value;

/// `let` definitions
#[derive(Debug, Clone)]
struct DefinitionEntry {
    params: Vec<String>,
    value: Value, // raw AST value (Const, Set, or Seq)
    line: usize,
}

/// An argument that's already been evaluated
#[derive(Debug, Clone)]
enum ResolvedArg {
    Key(u16),
    Number(i64)
}

#[derive(Debug, Clone)]
struct Evaluator {
    scopes: Vec<HashMap<String, DefinitionEntry>>,
    param_bindings: Vec<HashMap<String, ResolvedArg>>,
    evaluating: HashSet<String>, // cycle detection
}

impl Evaluator {

    // Empty scope stack with one global scope
    fn new() -> Self {
        Evaluator {
            scopes: vec![HashMap::new()],
            param_bindings: vec![HashMap::new()],
            evaluating: HashSet::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) -> Option<HashMap<String, DefinitionEntry>> {
        self.param_bindings.pop();
        self.scopes.pop()
    }

    fn lookup_scope(&self, name: &str) -> Option<&DefinitionEntry> {
        for scope in self.scopes.iter().rev() {
            if scope.contains_key(name) {
                return Some(scope.get_key_value(name).unwrap().1)
            }
        }
        None
    }

    fn lookup_binding(&self, name: &str) -> Option<&ResolvedArg> {
        for binding in self.param_bindings.iter().rev() {
            if binding.contains_key(name) {
                return Some(binding.get_key_value(name).unwrap().1)
            }
        }
        None
    }

    fn define(&mut self, name: String, entry: DefinitionEntry) -> Result<(), String> {
        if self.scopes.is_empty() {
            return Err("Attempted to define with no scope".to_string())
        }
        let current_scope = self.scopes.last_mut().unwrap();
        if current_scope.contains_key(&name) {
            return Err("Current scope already contains this key".to_string())
        }
        self.scopes.last_mut().unwrap().insert(name, entry);
        Ok(())
    }

}
