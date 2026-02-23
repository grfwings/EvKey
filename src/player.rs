//! Playing back recorded events

use crate::recorder::RecordedEvent;
use evdev::{uinput::VirtualDevice, AttributeSet, KeyCode, RelativeAxisCode};
use std::io;
use std::thread;
use std::time::Duration;

/// Maximum keycode value to register with the virtual device
const KEY_MAX: u16 = 0x2ff;

pub struct Player {
    device: VirtualDevice,
}

impl Player {
    /// Create a new player with a virtual device
    pub fn new(device_name: &str) -> io::Result<Self> {
        // Setup all keyboard keys
        let mut keys = AttributeSet::<KeyCode>::new();
        for key_code in 0..=KEY_MAX {
            keys.insert(KeyCode(key_code));
        }

        // Setup mouse relative axes
        let mut relative_axes = AttributeSet::<RelativeAxisCode>::new();
        relative_axes.insert(RelativeAxisCode::REL_X);
        relative_axes.insert(RelativeAxisCode::REL_Y);
        relative_axes.insert(RelativeAxisCode::REL_WHEEL);
        relative_axes.insert(RelativeAxisCode::REL_HWHEEL);

        let device = VirtualDevice::builder()?
            .name(device_name)
            .with_keys(&keys)?
            .with_relative_axes(&relative_axes)?
            .build()?;

        Ok(Self { device })
    }

    /// Play back recorded events with original timing
    ///
    /// # Current Implementation Notes:
    /// - Events are played back sequentially with sleep delays between them
    /// - Simultaneous events (same timestamp) are emitted separately with microsecond-level
    ///   delays between them, rather than being batched into a single emit call
    /// - This is functionally equivalent for most use cases, but true simultaneous events
    ///   could be batched together for more accurate playback
    /// - Held keys with different durations work correctly because press/release are
    ///   separate events with their own timestamps
    pub fn play(&mut self, events: &[RecordedEvent]) -> io::Result<()> {
        if events.is_empty() {
            println!("No events to play");
            return Ok(());
        }

        println!("Playing {} events...", events.len());

        let mut last_timestamp = 0u64;

        for recorded in events {
            // Calculate delay from last event
            let delay_us = recorded.timestamp_us.saturating_sub(last_timestamp);
            if delay_us > 0 {
                thread::sleep(Duration::from_micros(delay_us));
            }

            // TODO: For better accuracy, could batch events with identical timestamps
            // and emit them together in a single call
            self.device.emit(&[recorded.event])?;

            last_timestamp = recorded.timestamp_us;
        }

        println!("Playback complete");
        Ok(())
    }
}
