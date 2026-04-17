//! EvScript v2 storage format for macros
//!
//! Saves recorded events as EvScript v2 text and loads EvScript files
//! through the parser + evaluator pipeline.

use crate::keymap;
use crate::parser::evaluator;
use crate::parser::parser::parse_str;
use crate::recorder::RecordedEvent;
use crate::state::{events_to_states, states_to_events, MacroState};
use std::fs;
use std::io;
use std::path::Path;

/// Save recorded events as EvScript v2 text.
pub fn save<P: AsRef<Path>>(path: P, events: &[RecordedEvent]) -> io::Result<()> {
    let states = events_to_states(events);
    let source = format_program(&states);
    fs::write(path, source)
}

/// Load an EvScript v2 file and return playable events.
pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Vec<RecordedEvent>> {
    let source = fs::read_to_string(&path)?;
    let program = parse_str(&source)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let states = evaluator::evaluate(&program)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(states_to_events(&states))
}

/// Format a key name, falling back to `KEY_<N>` for unknown keycodes.
fn format_key(code: u16) -> String {
    keymap::keycode_to_name(code)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("KEY_{}", code))
}

/// Format a scroll direction and amount as `scroll DIR AMT`.
fn format_scroll(vertical: i32, horizontal: i32) -> Vec<String> {
    let mut parts = Vec::new();
    if vertical > 0 {
        parts.push(format!("scroll up {}", vertical));
    } else if vertical < 0 {
        parts.push(format!("scroll down {}", -vertical));
    }
    if horizontal > 0 {
        parts.push(format!("scroll right {}", horizontal));
    } else if horizontal < 0 {
        parts.push(format!("scroll left {}", -horizontal));
    }
    parts
}

/// Format a `Vec<MacroState>` as EvScript v2 source text.
pub fn format_program(states: &[MacroState]) -> String {
    let mut lines = Vec::new();
    lines.push("// EvKey Macro".to_string());
    lines.push("// Layout: QWERTY".to_string());
    lines.push(String::new());

    for state in states {
        let has_keys = !state.keys_pressed.is_empty();
        let has_mouse = state.mouse_delta != (0, 0);
        let has_scroll = state.scroll_delta != (0, 0);
        let has_duration = state.duration_ms > 0;

        let channel_count =
            has_keys as usize + has_mouse as usize + has_scroll as usize;

        if channel_count == 0 {
            if has_duration {
                lines.push(format!("wait {};", state.duration_ms));
            }
            continue;
        }

        if channel_count == 1 && !has_keys {
            // Single non-key action (mouse or scroll) — emit as standalone.
            if has_mouse {
                lines.push(format!(
                    "move {} {};",
                    state.mouse_delta.0, state.mouse_delta.1
                ));
            } else if has_scroll {
                for s in format_scroll(state.scroll_delta.0, state.scroll_delta.1) {
                    lines.push(format!("{};", s));
                }
            }
            if has_duration {
                lines.push(format!("wait {};", state.duration_ms));
            }
            continue;
        }

        // Both remaining branches need keys sorted for deterministic output.
        let mut sorted_keys: Vec<u16> = state.keys_pressed.iter().copied().collect();
        sorted_keys.sort();

        if channel_count == 1 {
            if sorted_keys.len() == 1 {
                lines.push(format!(
                    "hold {} for {};",
                    format_key(sorted_keys[0]),
                    state.duration_ms
                ));
            } else {
                let key_list: Vec<String> = sorted_keys.iter().map(|&k| format_key(k)).collect();
                lines.push(format!(
                    "hold {{ {} }} for {};",
                    key_list.join(", "),
                    state.duration_ms
                ));
            }
            continue;
        }

        // Multiple channels — use the general `{ ... } for DUR;` set syntax.
        let mut parts: Vec<String> = Vec::new();
        for k in &sorted_keys {
            parts.push(format!("hold {}", format_key(*k)));
        }
        if has_mouse {
            parts.push(format!(
                "move {} {}",
                state.mouse_delta.0, state.mouse_delta.1
            ));
        }
        if has_scroll {
            for s in format_scroll(state.scroll_delta.0, state.scroll_delta.1) {
                parts.push(s);
            }
        }
        lines.push(format!(
            "{{ {} }} for {};",
            parts.join(", "),
            state.duration_ms
        ));
    }

    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Helper: format a state, parse it back, evaluate, and return the resulting states.
    fn round_trip(state: &MacroState) -> Vec<MacroState> {
        let program_text = format_program(&[state.clone()]);
        let program = parse_str(&program_text).unwrap_or_else(|e| {
            panic!("round-trip parse failed: {e}\nsource:\n{program_text}");
        });
        evaluator::evaluate(&program).unwrap_or_else(|e| {
            panic!("round-trip eval failed: {e}\nsource:\n{program_text}");
        })
    }

    #[test]
    fn test_round_trip_keys_only() {
        let original = MacroState {
            duration_ms: 200,
            keys_pressed: [17, 30].iter().copied().collect(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 200);
        assert_eq!(states[0].keys_pressed, original.keys_pressed);
    }

    #[test]
    fn test_round_trip_single_key() {
        let original = MacroState {
            duration_ms: 100,
            keys_pressed: [17].iter().copied().collect(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 100);
        assert!(states[0].keys_pressed.contains(&17));
    }

    #[test]
    fn test_round_trip_wait() {
        let original = MacroState {
            duration_ms: 500,
            keys_pressed: HashSet::new(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 500);
        assert!(states[0].keys_pressed.is_empty());
    }

    #[test]
    fn test_round_trip_mouse_only() {
        let original = MacroState {
            duration_ms: 0,
            keys_pressed: HashSet::new(),
            mouse_delta: (10, -5),
            scroll_delta: (0, 0),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].mouse_delta, (10, -5));
    }

    #[test]
    fn test_round_trip_scroll_only() {
        let original = MacroState {
            duration_ms: 0,
            keys_pressed: HashSet::new(),
            mouse_delta: (0, 0),
            scroll_delta: (3, 0),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].scroll_delta, (3, 0));
    }

    #[test]
    fn test_round_trip_scroll_horizontal() {
        let original = MacroState {
            duration_ms: 0,
            keys_pressed: HashSet::new(),
            mouse_delta: (0, 0),
            scroll_delta: (0, -2),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].scroll_delta, (0, -2));
    }

    #[test]
    fn test_round_trip_keys_and_mouse() {
        let original = MacroState {
            duration_ms: 100,
            keys_pressed: [17].iter().copied().collect(),
            mouse_delta: (10, -5),
            scroll_delta: (0, 0),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 100);
        assert!(states[0].keys_pressed.contains(&17));
        assert_eq!(states[0].mouse_delta, (10, -5));
    }

    #[test]
    fn test_round_trip_keys_and_scroll() {
        let original = MacroState {
            duration_ms: 200,
            keys_pressed: [17].iter().copied().collect(),
            mouse_delta: (0, 0),
            scroll_delta: (-3, 0),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 200);
        assert!(states[0].keys_pressed.contains(&17));
        assert_eq!(states[0].scroll_delta, (-3, 0));
    }

    #[test]
    fn test_round_trip_keys_mouse_scroll() {
        let original = MacroState {
            duration_ms: 50,
            keys_pressed: [17, 272].iter().copied().collect(),
            mouse_delta: (10, -5),
            scroll_delta: (-1, 0),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 50);
        assert!(states[0].keys_pressed.contains(&17));
        assert!(states[0].keys_pressed.contains(&272));
        assert_eq!(states[0].mouse_delta, (10, -5));
        assert_eq!(states[0].scroll_delta, (-1, 0));
    }

    #[test]
    fn test_round_trip_unknown_keycode() {
        let original = MacroState {
            duration_ms: 100,
            keys_pressed: [412].iter().copied().collect(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert!(states[0].keys_pressed.contains(&412));
    }

    #[test]
    fn test_round_trip_zero_duration_key() {
        let original = MacroState {
            duration_ms: 0,
            keys_pressed: [17].iter().copied().collect(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };
        let states = round_trip(&original);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 0);
        assert!(states[0].keys_pressed.contains(&17));
    }

    #[test]
    fn test_format_header() {
        let text = format_program(&[]);
        assert!(text.starts_with("// EvKey Macro\n// Layout: QWERTY"));
    }

    #[test]
    fn test_format_hold_single() {
        let state = MacroState {
            duration_ms: 100,
            keys_pressed: [17].iter().copied().collect(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };
        let text = format_program(&[state]);
        assert!(text.contains("hold W for 100;"));
    }

    #[test]
    fn test_format_hold_multiple() {
        let state = MacroState {
            duration_ms: 100,
            keys_pressed: [17, 30].iter().copied().collect(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };
        let text = format_program(&[state]);
        assert!(text.contains("hold {") && text.contains("} for 100;"));
    }

    #[test]
    fn test_format_combined_set() {
        let state = MacroState {
            duration_ms: 100,
            keys_pressed: [17].iter().copied().collect(),
            mouse_delta: (10, -5),
            scroll_delta: (0, 0),
        };
        let text = format_program(&[state]);
        assert!(text.contains("{ hold W, move 10 -5 } for 100;"));
    }

    #[test]
    fn test_load_hand_written() {
        // Simulate loading from a hand-written v2 source.
        let source = "\
let HOLD_TIME = 500;\n\
let combo = hold { W, A };\n\
\n\
combo for HOLD_TIME;\n\
wait 100;\n\
tap SPACE;\n";

        let program = parse_str(source).unwrap();
        let states = evaluator::evaluate(&program).unwrap();

        assert_eq!(states.len(), 3);
        assert_eq!(states[0].duration_ms, 500);
        assert!(states[0].keys_pressed.contains(&17)); // W
        assert!(states[0].keys_pressed.contains(&30)); // A
        assert_eq!(states[1].duration_ms, 100);
        assert!(states[1].keys_pressed.is_empty());
        assert_eq!(states[2].duration_ms, 50); // tap default
    }
}
