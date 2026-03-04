//! State-based macro representation
//!
//! Converts low-level input events into high-level "states" representing
//! which keys are pressed for how long. This enables human-readable macros.

use crate::recorder::RecordedEvent;
use evdev::{EventType, InputEvent};
use std::collections::HashSet;

/// Manhattan distance threshold below which mouse movements are filtered out
const MOUSE_FILTER_THRESHOLD: i32 = 5;

/// A macro state: which keys are held and for how long
#[derive(Debug, Clone, PartialEq)]
pub struct MacroState {
    /// Duration this state lasts (in milliseconds)
    pub duration_ms: u64,
    /// Keys that are pressed during this state (Linux keycodes)
    pub keys_pressed: HashSet<u16>,
    /// Mouse movement during this state (relative x, y)
    pub mouse_delta: (i32, i32),
    /// Mouse scroll during this state (vertical, horizontal)
    pub scroll_delta: (i32, i32),
}

impl MacroState {
    pub fn new(duration_ms: u64) -> Self {
        Self {
            duration_ms,
            keys_pressed: HashSet::new(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        }
    }
}

/// Convert recorded events into state-based representation
pub fn events_to_states(events: &[RecordedEvent]) -> Vec<MacroState> {
    if events.is_empty() {
        return Vec::new();
    }

    let mut states = Vec::new();
    let mut current_keys: HashSet<u16> = HashSet::new();
    let mut last_timestamp_us = 0u64;
    let mut accumulated_mouse = (0i32, 0i32);
    let mut accumulated_scroll = (0i32, 0i32);

    for event in events {
        let elapsed_us = event.timestamp_us.saturating_sub(last_timestamp_us);

        // If time has passed, save the current state (even if empty - that's a wait)
        if elapsed_us > 0 {
            let duration_ms = elapsed_us.div_ceil(1000); // Round up to avoid losing sub-ms events
            if duration_ms > 0 {
                let mut state = MacroState::new(duration_ms);
                state.keys_pressed = current_keys.clone();
                state.mouse_delta = accumulated_mouse;
                state.scroll_delta = accumulated_scroll;
                states.push(state);

                // Reset mouse and scroll accumulators after saving
                accumulated_mouse = (0, 0);
                accumulated_scroll = (0, 0);
            }
        }

        // Process the event
        match EventType(event.event.event_type().0) {
            EventType::KEY => {
                let key_code = event.event.code();
                let value = event.event.value();

                match value {
                    1 => {
                        // Key press
                        current_keys.insert(key_code);
                    }
                    0 => {
                        // Key release
                        current_keys.remove(&key_code);
                    }
                    _ => {
                        // Ignore key repeat (value 2)
                    }
                }
            }
            EventType::RELATIVE => {
                // Mouse movement and scroll
                let axis_code = event.event.code();
                let value = event.event.value();

                match axis_code {
                    0 => accumulated_mouse.0 += value,  // REL_X
                    1 => accumulated_mouse.1 += value,  // REL_Y
                    8 => accumulated_scroll.0 += value, // REL_WHEEL (vertical)
                    6 => accumulated_scroll.1 += value, // REL_HWHEEL (horizontal)
                    _ => {}
                }
            }
            _ => {
                // Ignore sync and other event types for state tracking
            }
        }

        last_timestamp_us = event.timestamp_us;
    }

    // Add final state if keys are still pressed or actions remain
    if !current_keys.is_empty() || accumulated_mouse != (0, 0) || accumulated_scroll != (0, 0) {
        let mut state = MacroState::new(0); // Final state with no duration
        state.keys_pressed = current_keys;
        state.mouse_delta = accumulated_mouse;
        state.scroll_delta = accumulated_scroll;
        states.push(state);
    }

    // Filter out small mouse movements (< threshold) from all states
    for state in &mut states {
        let distance = state.mouse_delta.0.abs() + state.mouse_delta.1.abs();
        if distance < MOUSE_FILTER_THRESHOLD {
            state.mouse_delta = (0, 0);
        }
    }

    // Merge consecutive identical states
    merge_consecutive_states(states)
}

/// Merge consecutive states that have the same keys pressed
fn merge_consecutive_states(states: Vec<MacroState>) -> Vec<MacroState> {
    if states.is_empty() {
        return states;
    }

    let mut merged = Vec::new();
    let mut current = states[0].clone();

    for state in states.into_iter().skip(1) {
        // Only merge if keys match and no mouse/scroll movement in either
        // (small movements already filtered to (0, 0) before merging)
        if current.keys_pressed == state.keys_pressed
            && current.mouse_delta == (0, 0)
            && state.mouse_delta == (0, 0)
            && current.scroll_delta == (0, 0)
            && state.scroll_delta == (0, 0)
        {
            current.duration_ms += state.duration_ms;
        } else {
            merged.push(current);
            current = state;
        }
    }

    merged.push(current);
    merged
}

/// Convert state-based representation back to events
pub fn states_to_events(states: &[MacroState]) -> Vec<RecordedEvent> {
    let mut events = Vec::new();
    let mut timestamp_us = 0u64;
    let mut current_keys: HashSet<u16> = HashSet::new();

    for state in states {
        // Determine which keys need to be pressed and released
        let mut keys_to_press: Vec<u16> = state
            .keys_pressed
            .difference(&current_keys)
            .copied()
            .collect();
        let mut keys_to_release: Vec<u16> = current_keys
            .difference(&state.keys_pressed)
            .copied()
            .collect();

        // Sort for deterministic ordering
        keys_to_press.sort();
        keys_to_release.sort();

        // Release keys that are no longer pressed
        for key_code in keys_to_release {
            events.push(RecordedEvent {
                timestamp_us,
                event: InputEvent::new(EventType::KEY.0, key_code, 0),
            });
            events.push(RecordedEvent {
                timestamp_us,
                event: InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
            });
        }

        // Press new keys
        for key_code in keys_to_press {
            events.push(RecordedEvent {
                timestamp_us,
                event: InputEvent::new(EventType::KEY.0, key_code, 1),
            });
            events.push(RecordedEvent {
                timestamp_us,
                event: InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
            });
        }

        // Add mouse movement if any
        if state.mouse_delta != (0, 0) {
            if state.mouse_delta.0 != 0 {
                events.push(RecordedEvent {
                    timestamp_us,
                    event: InputEvent::new(EventType::RELATIVE.0, 0, state.mouse_delta.0),
                });
            }
            if state.mouse_delta.1 != 0 {
                events.push(RecordedEvent {
                    timestamp_us,
                    event: InputEvent::new(EventType::RELATIVE.0, 1, state.mouse_delta.1),
                });
            }
            events.push(RecordedEvent {
                timestamp_us,
                event: InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
            });
        }

        // Add scroll events if any
        if state.scroll_delta != (0, 0) {
            if state.scroll_delta.0 != 0 {
                events.push(RecordedEvent {
                    timestamp_us,
                    event: InputEvent::new(EventType::RELATIVE.0, 8, state.scroll_delta.0), // REL_WHEEL
                });
            }
            if state.scroll_delta.1 != 0 {
                events.push(RecordedEvent {
                    timestamp_us,
                    event: InputEvent::new(EventType::RELATIVE.0, 6, state.scroll_delta.1), // REL_HWHEEL
                });
            }
            events.push(RecordedEvent {
                timestamp_us,
                event: InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
            });
        }

        // Update current state
        current_keys = state.keys_pressed.clone();

        // Advance time
        timestamp_us += state.duration_ms * 1000; // Convert ms to microseconds
    }

    // Release all remaining keys at the end (sorted for determinism)
    let mut remaining: Vec<u16> = current_keys.into_iter().collect();
    remaining.sort();
    for key_code in remaining {
        events.push(RecordedEvent {
            timestamp_us,
            event: InputEvent::new(EventType::KEY.0, key_code, 0),
        });
        events.push(RecordedEvent {
            timestamp_us,
            event: InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
        });
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_events() {
        let states = events_to_states(&[]);
        assert!(states.is_empty());
    }

    #[test]
    fn test_single_key_press() {
        let events = vec![
            RecordedEvent {
                timestamp_us: 0,
                event: InputEvent::new(EventType::KEY.0, 17, 1), // W press
            },
            RecordedEvent {
                timestamp_us: 100_000,                           // 100ms later
                event: InputEvent::new(EventType::KEY.0, 17, 0), // W release
            },
        ];

        let states = events_to_states(&events);
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].duration_ms, 100);
        assert!(states[0].keys_pressed.contains(&17));
    }

    #[test]
    fn test_merge_consecutive_states() {
        let states = vec![
            MacroState {
                duration_ms: 10,
                keys_pressed: [17].iter().copied().collect(),
                mouse_delta: (0, 0),
                scroll_delta: (0, 0),
            },
            MacroState {
                duration_ms: 20,
                keys_pressed: [17].iter().copied().collect(),
                mouse_delta: (0, 0),
                scroll_delta: (0, 0),
            },
        ];

        let merged = merge_consecutive_states(states);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].duration_ms, 30);
    }

    #[test]
    fn test_wait_gap_between_keys() {
        // Simulate: Press W, hold for 100ms, release, wait 6000ms, press A
        let events = vec![
            RecordedEvent {
                timestamp_us: 0,
                event: InputEvent::new(EventType::KEY.0, 17, 1), // W press
            },
            RecordedEvent {
                timestamp_us: 100_000,                           // 100ms later
                event: InputEvent::new(EventType::KEY.0, 17, 0), // W release
            },
            RecordedEvent {
                timestamp_us: 6_100_000,                         // 6 seconds later
                event: InputEvent::new(EventType::KEY.0, 30, 1), // A press
            },
            RecordedEvent {
                timestamp_us: 6_200_000,                         // 100ms later
                event: InputEvent::new(EventType::KEY.0, 30, 0), // A release
            },
        ];

        let states = events_to_states(&events);

        // Should have 3 states:
        // 1. W held for 100ms
        // 2. Wait (empty) for 6000ms
        // 3. A held for 100ms
        assert_eq!(states.len(), 3);

        // First state: W held
        assert_eq!(states[0].duration_ms, 100);
        assert!(states[0].keys_pressed.contains(&17));

        // Second state: Wait (no keys)
        assert_eq!(states[1].duration_ms, 6000);
        assert!(states[1].keys_pressed.is_empty());

        // Third state: A held
        assert_eq!(states[2].duration_ms, 100);
        assert!(states[2].keys_pressed.contains(&30));
    }

    #[test]
    fn test_states_to_events_single_key() {
        let states = vec![MacroState {
            duration_ms: 100,
            keys_pressed: [17].iter().copied().collect(), // W
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        }];

        let events = states_to_events(&states);

        // Should have: key press + syn, key release + syn
        assert!(events.len() >= 4);

        // First event: key press
        assert_eq!(events[0].event.event_type(), EventType::KEY);
        assert_eq!(events[0].event.code(), 17);
        assert_eq!(events[0].event.value(), 1);

        // SYN after press
        assert_eq!(events[1].event.event_type(), EventType::SYNCHRONIZATION);

        // Last two: release + syn at timestamp 100_000
        let last_key = &events[events.len() - 2];
        assert_eq!(last_key.event.event_type(), EventType::KEY);
        assert_eq!(last_key.event.code(), 17);
        assert_eq!(last_key.event.value(), 0);
        assert_eq!(last_key.timestamp_us, 100_000);
    }

    #[test]
    fn test_states_to_events_multiple_keys_ordered() {
        let states = vec![MacroState {
            duration_ms: 50,
            keys_pressed: [30, 17].iter().copied().collect(), // A=30, W=17
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        }];

        let events = states_to_events(&states);

        // Collect key press events
        let presses: Vec<u16> = events
            .iter()
            .filter(|e| e.event.event_type() == EventType::KEY && e.event.value() == 1)
            .map(|e| e.event.code())
            .collect();

        // Should be sorted: 17 (W) before 30 (A)
        assert_eq!(presses, vec![17, 30]);
    }

    #[test]
    fn test_states_to_events_mouse_and_scroll() {
        let states = vec![MacroState {
            duration_ms: 0,
            keys_pressed: HashSet::new(),
            mouse_delta: (10, -5),
            scroll_delta: (3, 0),
        }];

        let events = states_to_events(&states);

        // Should have REL_X, REL_Y, SYN, REL_WHEEL, SYN
        let rel_events: Vec<_> = events
            .iter()
            .filter(|e| e.event.event_type() == EventType::RELATIVE)
            .collect();
        assert_eq!(rel_events.len(), 3); // REL_X, REL_Y, REL_WHEEL
    }

    #[test]
    fn test_states_to_events_empty() {
        let events = states_to_events(&[]);
        assert!(events.is_empty());
    }

    #[test]
    fn test_states_to_events_releases_all_keys() {
        let states = vec![MacroState {
            duration_ms: 100,
            keys_pressed: [17, 30].iter().copied().collect(),
            mouse_delta: (0, 0),
            scroll_delta: (0, 0),
        }];

        let events = states_to_events(&states);

        // Collect release events
        let releases: Vec<u16> = events
            .iter()
            .filter(|e| e.event.event_type() == EventType::KEY && e.event.value() == 0)
            .map(|e| e.event.code())
            .collect();

        // Both keys should be released at the end, in sorted order
        assert_eq!(releases, vec![17, 30]);
    }

    #[test]
    fn test_mouse_filter_boundary_below() {
        // 4px total movement should be filtered to (0, 0)
        let events = vec![
            RecordedEvent {
                timestamp_us: 0,
                event: InputEvent::new(EventType::RELATIVE.0, 0, 2), // REL_X = 2
            },
            RecordedEvent {
                timestamp_us: 1000,                                  // 1ms later
                event: InputEvent::new(EventType::RELATIVE.0, 1, 2), // REL_Y = 2
            },
        ];

        let states = events_to_states(&events);
        // The mouse delta should be filtered out
        for state in &states {
            assert_eq!(state.mouse_delta, (0, 0));
        }
    }

    #[test]
    fn test_mouse_filter_boundary_at() {
        // 5px total movement within a single state should be kept
        // Both events at timestamp 0 so they accumulate before being saved
        let events = vec![
            RecordedEvent {
                timestamp_us: 0,
                event: InputEvent::new(EventType::RELATIVE.0, 0, 3), // REL_X = 3
            },
            RecordedEvent {
                timestamp_us: 0,
                event: InputEvent::new(EventType::RELATIVE.0, 1, 2), // REL_Y = 2
            },
            RecordedEvent {
                timestamp_us: 1000, // 1ms later triggers state save
                event: InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
            },
        ];

        let states = events_to_states(&events);
        let has_mouse = states.iter().any(|s| s.mouse_delta != (0, 0));
        assert!(has_mouse, "5px movement should be kept");
    }

    #[test]
    fn test_key_repeat_ignored() {
        // Key repeat events (value=2) should not affect output
        let events = vec![
            RecordedEvent {
                timestamp_us: 0,
                event: InputEvent::new(EventType::KEY.0, 17, 1), // W press
            },
            RecordedEvent {
                timestamp_us: 50_000,
                event: InputEvent::new(EventType::KEY.0, 17, 2), // W repeat (should be ignored)
            },
            RecordedEvent {
                timestamp_us: 100_000,
                event: InputEvent::new(EventType::KEY.0, 17, 0), // W release
            },
        ];

        let states = events_to_states(&events);

        // Key repeat should not cause extra states or duplicate keys
        // Should still be just W held for 100ms (with possible sub-states merged)
        let total_duration: u64 = states.iter().map(|s| s.duration_ms).sum();
        assert_eq!(total_duration, 100);

        // W should appear in pressed keys
        assert!(states.iter().any(|s| s.keys_pressed.contains(&17)));

        // No state should have W appearing more than once (it's a HashSet, so this is inherent)
        for state in &states {
            if state.keys_pressed.contains(&17) {
                assert_eq!(state.keys_pressed.iter().filter(|&&k| k == 17).count(), 1);
            }
        }
    }
}
