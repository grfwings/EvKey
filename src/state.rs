//! State-based macro representation
//!
//! Converts low-level input events into high-level "states" representing
//! which keys are pressed for how long. This enables human-readable macros.

use crate::recorder::RecordedEvent;
use evdev::{EventType, InputEvent};
use std::collections::HashSet;

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

    /// Check if this state has any actions
    pub fn is_empty(&self) -> bool {
        self.keys_pressed.is_empty()
            && self.mouse_delta == (0, 0)
            && self.scroll_delta == (0, 0)
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
            let duration_ms = elapsed_us / 1000; // Convert microseconds to milliseconds
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
                    0 => accumulated_mouse.0 += value,   // REL_X
                    1 => accumulated_mouse.1 += value,   // REL_Y
                    8 => accumulated_scroll.0 += value,  // REL_WHEEL (vertical)
                    6 => accumulated_scroll.1 += value,  // REL_HWHEEL (horizontal)
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

    // Filter out small mouse movements (< 5px) from all states
    for state in &mut states {
        let distance = state.mouse_delta.0.abs() + state.mouse_delta.1.abs();
        if distance < 5 {
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
        let keys_to_press: Vec<u16> = state
            .keys_pressed
            .difference(&current_keys)
            .copied()
            .collect();
        let keys_to_release: Vec<u16> = current_keys
            .difference(&state.keys_pressed)
            .copied()
            .collect();

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
        for key_code in &keys_to_press {
            events.push(RecordedEvent {
                timestamp_us,
                event: InputEvent::new(EventType::KEY.0, *key_code, 1),
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

    // Release all remaining keys at the end
    for key_code in current_keys {
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
                timestamp_us: 100_000, // 100ms later
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
                timestamp_us: 100_000, // 100ms later
                event: InputEvent::new(EventType::KEY.0, 17, 0), // W release
            },
            RecordedEvent {
                timestamp_us: 6_100_000, // 6 seconds later
                event: InputEvent::new(EventType::KEY.0, 30, 1), // A press
            },
            RecordedEvent {
                timestamp_us: 6_200_000, // 100ms later
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
}
