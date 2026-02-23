//! Recording input events from keyboard and mouse

use evdev::{Device, InputEvent, EventSummary, KeyCode};
use std::io;
use std::path::Path;
use std::time::Instant;

/// Recorded event with relative timestamp
#[derive(Debug, Clone)]
pub struct RecordedEvent {
    /// Time since recording started (in microseconds)
    pub timestamp_us: u64,
    /// The actual input event
    pub event: InputEvent,
}

pub struct Recorder {
    devices: Vec<Device>,
    start_time: Option<Instant>,
    events: Vec<RecordedEvent>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            start_time: None,
            events: Vec::new(),
        }
    }

    /// Add a device to record from
    pub fn add_device<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let device = Device::open(path)?;
        device.set_nonblocking(true)?;
        println!("Added device: {}", device.name().unwrap_or("unknown"));
        self.devices.push(device);
        Ok(())
    }

    /// Poll all devices and record events
    /// Returns true if recording state changed (started or stopped)
    pub fn poll(&mut self) -> io::Result<bool> {
        let mut state_changed = false;

        for device in &mut self.devices {
            match device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        if let EventSummary::Key(_, key, value) = event.destructure() {
                            if key == KeyCode::KEY_F1 {
                                if value == 1 {
                                    // F1 pressed - toggle recording state
                                    println!("F1 key pressed!");
                                    if self.start_time.is_none() {
                                        // Start recording
                                        self.start_time = Some(Instant::now());
                                        self.events.clear();
                                        state_changed = true;
                                    } else {
                                        // Stop recording
                                        self.start_time = None;
                                        state_changed = true;
                                    }
                                }
                                continue; // Don't record F1 press or release
                            }
                        }
                        // Only record events if we're currently recording
                        if let Some(start_time) = self.start_time {
                            let elapsed = start_time.elapsed();
                            let timestamp_us = elapsed.as_micros() as u64;

                            self.events.push(RecordedEvent {
                                timestamp_us,
                                event,
                            });
                        }
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Don't error if there are no events polled
                    continue;
                }
                Err(e) => {
                    eprintln!("Device read error: {}", e);
                }

            }
        }

        Ok(state_changed)
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.start_time.is_some()
    }

    /// Stop recording and return recorded events
    pub fn stop(&mut self) -> Vec<RecordedEvent> {
        self.start_time = None;
        println!("Recording stopped. Recorded {} events", self.events.len());
        std::mem::take(&mut self.events)
    }

}
