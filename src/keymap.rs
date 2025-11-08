//! Keyboard layout mappings for converting between keycodes and human-readable names
//!
//! Currently supports QWERTY layout. Future: XKB integration for multi-layout support.

use std::collections::HashMap;

/// Get human-readable name for a Linux keycode (QWERTY layout)
pub fn keycode_to_name(keycode: u16) -> Option<String> {
    let map = get_qwerty_map();
    map.get(&keycode).map(|s| s.to_string())
}

/// Get Linux keycode from human-readable name (QWERTY layout)
pub fn name_to_keycode(name: &str) -> Option<u16> {
    let map = get_qwerty_reverse_map();
    map.get(name.to_uppercase().as_str()).copied()
}

/// QWERTY layout keycode to name mapping
fn get_qwerty_map() -> HashMap<u16, &'static str> {
    HashMap::from([
        // Letters (QWERTY physical layout)
        (16, "Q"),
        (17, "W"),
        (18, "E"),
        (19, "R"),
        (20, "T"),
        (21, "Y"),
        (22, "U"),
        (23, "I"),
        (24, "O"),
        (25, "P"),
        (30, "A"),
        (31, "S"),
        (32, "D"),
        (33, "F"),
        (34, "G"),
        (35, "H"),
        (36, "J"),
        (37, "K"),
        (38, "L"),
        (44, "Z"),
        (45, "X"),
        (46, "C"),
        (47, "V"),
        (48, "B"),
        (49, "N"),
        (50, "M"),

        // Numbers row
        (2, "1"),
        (3, "2"),
        (4, "3"),
        (5, "4"),
        (6, "5"),
        (7, "6"),
        (8, "7"),
        (9, "8"),
        (10, "9"),
        (11, "0"),
        (12, "MINUS"),
        (13, "EQUAL"),

        // Function keys
        (59, "F1"),
        (60, "F2"),
        (61, "F3"),
        (62, "F4"),
        (63, "F5"),
        (64, "F6"),
        (65, "F7"),
        (66, "F8"),
        (67, "F9"),
        (68, "F10"),
        (87, "F11"),
        (88, "F12"),

        // Special keys
        (1, "ESC"),
        (14, "BACKSPACE"),
        (15, "TAB"),
        (28, "ENTER"),
        (29, "CTRL"),
        (42, "SHIFT"),
        (54, "RIGHTSHIFT"),
        (56, "ALT"),
        (57, "SPACE"),
        (58, "CAPSLOCK"),
        (97, "RIGHTCTRL"),
        (100, "RIGHTALT"),

        // Navigation
        (102, "HOME"),
        (103, "UP"),
        (104, "PAGEUP"),
        (105, "LEFT"),
        (106, "RIGHT"),
        (107, "END"),
        (108, "DOWN"),
        (109, "PAGEDOWN"),
        (110, "INSERT"),
        (111, "DELETE"),

        // Punctuation
        (26, "LEFTBRACE"),
        (27, "RIGHTBRACE"),
        (39, "SEMICOLON"),
        (40, "APOSTROPHE"),
        (41, "GRAVE"),
        (43, "BACKSLASH"),
        (51, "COMMA"),
        (52, "DOT"),
        (53, "SLASH"),

        // Keypad
        (55, "KPASTERISK"),
        (71, "KP7"),
        (72, "KP8"),
        (73, "KP9"),
        (74, "KPMINUS"),
        (75, "KP4"),
        (76, "KP5"),
        (77, "KP6"),
        (78, "KPPLUS"),
        (79, "KP1"),
        (80, "KP2"),
        (81, "KP3"),
        (82, "KP0"),
        (83, "KPDOT"),
        (96, "KPENTER"),
        (98, "KPSLASH"),

        // Mouse buttons (for completeness)
        (272, "BTN_LEFT"),
        (273, "BTN_RIGHT"),
        (274, "BTN_MIDDLE"),
    ])
}

/// Reverse mapping: name to keycode
fn get_qwerty_reverse_map() -> HashMap<&'static str, u16> {
    get_qwerty_map().into_iter().map(|(k, v)| (v, k)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keycode_to_name() {
        assert_eq!(keycode_to_name(17), Some("W".to_string()));
        assert_eq!(keycode_to_name(30), Some("A".to_string()));
        assert_eq!(keycode_to_name(57), Some("SPACE".to_string()));
    }

    #[test]
    fn test_name_to_keycode() {
        assert_eq!(name_to_keycode("W"), Some(17));
        assert_eq!(name_to_keycode("w"), Some(17)); // Case insensitive
        assert_eq!(name_to_keycode("SPACE"), Some(57));
        assert_eq!(name_to_keycode("INVALID"), None);
    }

    #[test]
    fn test_roundtrip() {
        let keycode = 17;
        let name = keycode_to_name(keycode).unwrap();
        assert_eq!(name_to_keycode(&name), Some(keycode));
    }
}
