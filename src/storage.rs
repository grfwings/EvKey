//! DSL-based storage format for macros
//!
//! Human-readable format:
//!   hold W for 12ms
//!   hold W+A for 4ms
//!   wait 100ms
//!   move 10 -5

use crate::keymap;
use crate::recorder::RecordedEvent;
use crate::state::{events_to_states, states_to_events, MacroState};
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

const TAP_DURATION_MS: u64 = 50;

/// Save recorded events as human-readable DSL
pub fn save<P: AsRef<Path>>(path: P, events: &[RecordedEvent]) -> io::Result<()> {
    let mut file = File::create(path)?;

    writeln!(file, "// EvKey Macro")?;
    writeln!(file, "// Layout: QWERTY")?;
    writeln!(file)?;

    // Convert events to states
    let states = events_to_states(events);

    // Write each state in DSL format
    for state in &states {
        for line in format_state(state) {
            writeln!(file, "{}", line)?;
        }
    }

    Ok(())
}

/// Load macro from DSL format
pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Vec<RecordedEvent>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut states = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        match parse_line(line) {
            Ok(state) => states.push(state),
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Line {}: {}", line_num + 1, e),
                ));
            }
        }
    }

    // Convert states back to events
    Ok(states_to_events(&states))
}

/// Format a MacroState as one or more DSL lines
fn format_state(state: &MacroState) -> Vec<String> {
    let mut lines = Vec::new();

    // Format keys
    if !state.keys_pressed.is_empty() {
        let mut keys: Vec<String> = state
            .keys_pressed
            .iter()
            .map(|&code| {
                keymap::keycode_to_name(code)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("KEY_{}", code))
            })
            .collect();
        keys.sort(); // Consistent ordering

        if state.duration_ms > 0 {
            lines.push(format!("hold {} for {}ms", keys.join("+"), state.duration_ms));
        } else {
            lines.push(format!("tap {}", keys.join("+")));
        }
    }

    // Format mouse movement (separate line — instant action)
    if state.mouse_delta != (0, 0) {
        lines.push(format!(
            "move {} {}",
            state.mouse_delta.0, state.mouse_delta.1
        ));
    }

    // Format scroll (separate line — instant action)
    if state.scroll_delta != (0, 0) {
        if state.scroll_delta.0 != 0 {
            let direction = if state.scroll_delta.0 > 0 {
                "up"
            } else {
                "down"
            };
            lines.push(format!("scroll {} {}", direction, state.scroll_delta.0.abs()));
        }
        if state.scroll_delta.1 != 0 {
            let direction = if state.scroll_delta.1 > 0 {
                "right"
            } else {
                "left"
            };
            lines.push(format!(
                "scroll {} {}",
                direction,
                state.scroll_delta.1.abs()
            ));
        }
    }

    // Handle empty state or duration-only with mouse/scroll
    if lines.is_empty() {
        if state.duration_ms > 0 {
            lines.push(format!("wait {}ms", state.duration_ms));
        } else {
            lines.push("// empty state".to_string());
        }
    } else if state.duration_ms > 0 && state.keys_pressed.is_empty() {
        // Has actions (mouse/scroll) with duration but no keys — add separate wait
        lines.push(format!("wait {}ms", state.duration_ms));
    }

    lines
}

/// Parse a DSL line into a MacroState
fn parse_line(line: &str) -> Result<MacroState, String> {
    let line = line.trim();

    // Parse "hold KEY for NNms" or "hold KEY+KEY2 for NNms"
    if let Some(rest) = line.strip_prefix("hold ") {
        let parts: Vec<&str> = rest.split(" for ").collect();
        if parts.len() != 2 {
            return Err(format!("Invalid 'hold' syntax: {}", line));
        }

        let keys_str = parts[0];
        let duration_str = parts[1];

        // Parse duration
        let duration_ms = parse_duration(duration_str)?;

        // Parse keys
        let keys = parse_keys(keys_str)?;

        return Ok(MacroState {
            duration_ms,
            keys_pressed: keys,
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        });
    }

    // Parse "wait NNms"
    if let Some(rest) = line.strip_prefix("wait ") {
        let duration_ms = parse_duration(rest)?;
        return Ok(MacroState {
            duration_ms,
            keys_pressed: HashSet::new(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        });
    }

    // Parse "move X Y"
    if let Some(rest) = line.strip_prefix("move ") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() != 2 {
            return Err(format!("Invalid 'move' syntax: {}", line));
        }

        let x: i32 = parts[0]
            .parse()
            .map_err(|_| format!("Invalid X coordinate: {}", parts[0]))?;
        let y: i32 = parts[1]
            .parse()
            .map_err(|_| format!("Invalid Y coordinate: {}", parts[1]))?;

        return Ok(MacroState {
            duration_ms: 0,
            keys_pressed: HashSet::new(),
            mouse_delta: (x, y),
            scroll_delta: (0, 0),
        });
    }

    // Parse "scroll DIRECTION AMOUNT" (e.g., "scroll up 3" or "scroll down 5")
    if let Some(rest) = line.strip_prefix("scroll ") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() != 2 {
            return Err(format!("Invalid 'scroll' syntax: {}", line));
        }

        let direction = parts[0];
        let amount: i32 = parts[1]
            .parse()
            .map_err(|_| format!("Invalid scroll amount: {}", parts[1]))?;

        let scroll_delta = match direction {
            "up" => (amount, 0),
            "down" => (-amount, 0),
            "left" => (0, -amount),
            "right" => (0, amount),
            _ => {
                return Err(format!(
                    "Invalid scroll direction '{}', use up/down/left/right",
                    direction
                ))
            }
        };

        return Ok(MacroState {
            duration_ms: 0,
            keys_pressed: HashSet::new(),
            mouse_delta: (0, 0),
            scroll_delta,
        });
    }

    // Parse "tap KEY" or "tap KEY+KEY2"
    if let Some(rest) = line.strip_prefix("tap ") {
        let keys = parse_keys(rest)?;
        return Ok(MacroState {
            duration_ms: TAP_DURATION_MS,
            keys_pressed: keys,
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        });
    }

    Err(format!("Unknown command: {}", line))
}

/// Parse duration string like "100ms" or "2s"
fn parse_duration(s: &str) -> Result<u64, String> {
    if let Some(ms_str) = s.strip_suffix("ms") {
        ms_str
            .parse::<u64>()
            .map_err(|_| format!("Invalid duration: {}", s))
    } else if let Some(s_str) = s.strip_suffix('s') {
        s_str
            .parse::<u64>()
            .map(|s| s * 1000)
            .map_err(|_| format!("Invalid duration: {}", s))
    } else {
        Err(format!("Duration must end with 'ms' or 's': {}", s))
    }
}

/// Parse key names like "W" or "W+A+SHIFT"
///
/// Supports named keys (via keymap) and numeric fallback "KEY_NNN".
fn parse_keys(s: &str) -> Result<HashSet<u16>, String> {
    let key_names: Vec<&str> = s.split('+').collect();
    let mut keycodes = HashSet::new();

    for name in key_names {
        let name = name.trim();
        if let Some(code) = keymap::name_to_keycode(name) {
            keycodes.insert(code);
        } else if let Some(num_str) = name.strip_prefix("KEY_") {
            let code: u16 = num_str
                .parse()
                .map_err(|_| format!("Invalid numeric keycode: {}", name))?;
            keycodes.insert(code);
        } else {
            return Err(format!("Unknown key: {}", name));
        }
    }

    Ok(keycodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hold() {
        let state = parse_line("hold W for 100ms").unwrap();
        assert_eq!(state.duration_ms, 100);
        assert!(state.keys_pressed.contains(&17)); // W = 17
    }

    #[test]
    fn test_parse_hold_multiple() {
        let state = parse_line("hold W+A for 50ms").unwrap();
        assert_eq!(state.duration_ms, 50);
        assert!(state.keys_pressed.contains(&17)); // W
        assert!(state.keys_pressed.contains(&30)); // A
    }

    #[test]
    fn test_parse_wait() {
        let state = parse_line("wait 200ms").unwrap();
        assert_eq!(state.duration_ms, 200);
        assert!(state.keys_pressed.is_empty());
    }

    #[test]
    fn test_parse_move() {
        let state = parse_line("move 10 -5").unwrap();
        assert_eq!(state.mouse_delta, (10, -5));
    }

    #[test]
    fn test_parse_scroll() {
        let state = parse_line("scroll up 3").unwrap();
        assert_eq!(state.scroll_delta, (3, 0));

        let state = parse_line("scroll down 5").unwrap();
        assert_eq!(state.scroll_delta, (-5, 0));

        let state = parse_line("scroll left 2").unwrap();
        assert_eq!(state.scroll_delta, (0, -2));

        let state = parse_line("scroll right 4").unwrap();
        assert_eq!(state.scroll_delta, (0, 4));
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("100ms").unwrap(), 100);
        assert_eq!(parse_duration("2s").unwrap(), 2000);
        assert!(parse_duration("100").is_err());
    }

    #[test]
    fn test_format_scroll_with_duration() {
        // State with scroll and duration should output scroll + wait
        let state = MacroState {
            duration_ms: 500,
            keys_pressed: HashSet::new(),
            mouse_delta: (0, 0),
            scroll_delta: (-1, 0), // scroll down
        };

        let lines = format_state(&state);
        assert!(lines.iter().any(|l| l.contains("scroll down 1")));
        assert!(lines.iter().any(|l| l.contains("wait 500ms")));
    }

    #[test]
    fn test_tap_duration() {
        let state = parse_line("tap W").unwrap();
        assert_eq!(state.duration_ms, TAP_DURATION_MS);
        assert!(state.keys_pressed.contains(&17));
    }

    #[test]
    fn test_unknown_keycode_fallback() {
        // A keycode not in our map should format as KEY_NNN
        let state = MacroState {
            duration_ms: 100,
            keys_pressed: [999].iter().copied().collect(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };
        let lines = format_state(&state);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("KEY_999"));
    }

    #[test]
    fn test_parse_numeric_keycode() {
        let state = parse_line("hold KEY_999 for 100ms").unwrap();
        assert!(state.keys_pressed.contains(&999));
    }

    #[test]
    fn test_roundtrip_keys_only() {
        let original = MacroState {
            duration_ms: 200,
            keys_pressed: [17, 30].iter().copied().collect(), // W + A
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };

        let lines = format_state(&original);
        assert_eq!(lines.len(), 1);
        let parsed = parse_line(&lines[0]).unwrap();
        assert_eq!(parsed.duration_ms, original.duration_ms);
        assert_eq!(parsed.keys_pressed, original.keys_pressed);
    }

    #[test]
    fn test_roundtrip_wait() {
        let original = MacroState {
            duration_ms: 500,
            keys_pressed: HashSet::new(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };

        let lines = format_state(&original);
        assert_eq!(lines.len(), 1);
        let parsed = parse_line(&lines[0]).unwrap();
        assert_eq!(parsed.duration_ms, original.duration_ms);
    }

    #[test]
    fn test_roundtrip_mouse() {
        let original = MacroState {
            duration_ms: 0,
            keys_pressed: HashSet::new(),
            mouse_delta: (10, -5),
            scroll_delta: (0, 0),
        };

        let lines = format_state(&original);
        assert_eq!(lines.len(), 1);
        let parsed = parse_line(&lines[0]).unwrap();
        assert_eq!(parsed.mouse_delta, original.mouse_delta);
    }

    #[test]
    fn test_roundtrip_scroll() {
        let original = MacroState {
            duration_ms: 0,
            keys_pressed: HashSet::new(),
            mouse_delta: (0, 0),
            scroll_delta: (3, 0),
        };

        let lines = format_state(&original);
        assert_eq!(lines.len(), 1);
        let parsed = parse_line(&lines[0]).unwrap();
        assert_eq!(parsed.scroll_delta, original.scroll_delta);
    }

    #[test]
    fn test_roundtrip_keys_with_mouse() {
        // Keys + mouse should produce separate lines, each round-trippable
        let original = MacroState {
            duration_ms: 200,
            keys_pressed: [17].iter().copied().collect(),
            mouse_delta: (10, -5),
            scroll_delta: (0, 0),
        };

        let lines = format_state(&original);
        assert_eq!(lines.len(), 2); // hold line + move line

        let key_state = parse_line(&lines[0]).unwrap();
        assert_eq!(key_state.duration_ms, 200);
        assert!(key_state.keys_pressed.contains(&17));

        let move_state = parse_line(&lines[1]).unwrap();
        assert_eq!(move_state.mouse_delta, (10, -5));
    }

    #[test]
    fn test_format_state_keys_with_duration() {
        let state = MacroState {
            duration_ms: 100,
            keys_pressed: [17].iter().copied().collect(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        };
        let lines = format_state(&state);
        assert_eq!(lines, vec!["hold W for 100ms"]);
    }

    #[test]
    fn test_invalid_parse_line() {
        assert!(parse_line("hold").is_err());
        assert!(parse_line("hold W").is_err());
        assert!(parse_line("hold W for").is_err());
        assert!(parse_line("move 10").is_err());
        assert!(parse_line("scroll diagonal 5").is_err());
        assert!(parse_line("gibberish").is_err());
        assert!(parse_line("hold UNKNOWNKEY for 100ms").is_err());
    }
}
