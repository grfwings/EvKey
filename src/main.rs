use std::env;
use std::error::Error;
use std::path::Path;
use std::thread;
use std::time::Duration;

mod recorder;
mod player;
mod storage;
mod state;
mod keymap;

use recorder::Recorder;
use player::Player;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "record" => {
            if args.len() < 3 {
                eprintln!("Usage: macromate record <output_file>");
                return Ok(());
            }
            record_macro(&args[2])?;
        }
        "play" => {
            if args.len() < 3 {
                eprintln!("Usage: macromate play [--loop] <input_file>");
                return Ok(());
            }

            // Find the input file (first arg that isn't --loop)
            let input_file = args[2..]
                .iter()
                .find(|arg| arg.as_str() != "--loop")
                .map(|s| s.as_str());

            let loop_flag = args.iter().any(|a| a == "--loop");

            match input_file {
                Some(file) => play_macro(file, loop_flag)?,
                None => {
                    eprintln!("Error: No input file specified");
                    eprintln!("Usage: macromate play [--loop] <input_file>");
                    return Ok(());
                }
            }
        }
        "list-devices" => {
            list_devices()?;
        }
        _ => {
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!("MacroMate - AutoHotkey-style macro recorder for Linux\n");
    println!("Usage:");
    println!("  macromate record <output_file>       Record a macro to file");
    println!("  macromate play [--loop] <input_file> Play back a recorded macro");
    println!("  macromate list-devices               List available input devices");
    println!("\nNote: You may need to run with sudo to access input devices");
}

fn list_devices() -> Result<(), Box<dyn Error>> {
    println!("Available input devices:\n");

    for entry in std::fs::read_dir("/dev/input")? {
        let entry = entry?;
        let path = entry.path();

        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            if name_str.starts_with("event") {
                match evdev::Device::open(&path) {
                    Ok(device) => {
                        println!("  {} - {}",
                            path.display(),
                            device.name().unwrap_or("unknown")
                        );
                    }
                    Err(_) => {
                        // Skip devices we can't open
                    }
                }
            }
        }
    }

    Ok(())
}

fn record_macro(output_file: &str) -> Result<(), Box<dyn Error>> {
    println!("MacroMate Recorder");
    println!("==================\n");

    println!("Auto-detecting keyboards and mice...\n");

    let mut recorder = Recorder::new();
    let mut device_count = 0;

    // Enumerate all devices and add keyboards/mice
    for entry in std::fs::read_dir("/dev/input")? {
        let entry = entry?;
        let path = entry.path();

        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            if name_str.starts_with("event") {
                match evdev::Device::open(&path) {
                    Ok(device) => {
                        // Check if device has keys (keyboard) or relative axes (mouse)
                        let has_keys = device.supported_keys().map_or(false, |keys| keys.iter().len() > 0);
                        let has_relative = device.supported_relative_axes().map_or(false, |axes| axes.iter().len() > 0);

                        if has_keys || has_relative {
                            let device_type = match (has_keys, has_relative) {
                                (true, true) => "keyboard+mouse",
                                (true, false) => "keyboard",
                                (false, true) => "mouse",
                                _ => continue,
                            };

                            println!("  {} - {} ({})",
                                path.display(),
                                device.name().unwrap_or("unknown"),
                                device_type
                            );

                            drop(device); // Close device before reopening in recorder
                            match recorder.add_device(&path) {
                                Ok(_) => device_count += 1,
                                Err(e) => eprintln!("    Warning: Could not add device: {}", e),
                            }
                        }
                    }
                    Err(_) => {
                        // Skip devices we can't open (permission issues, etc.)
                    }
                }
            }
        }
    }

    if device_count == 0 {
        eprintln!("\nError: No keyboard or mouse devices found!");
        eprintln!("Make sure you're running with sudo or have appropriate permissions.");
        return Ok(());
    }

    println!("\nFound {} input device(s)", device_count);

    println!("\n=== HOTKEY CONTROLS ===");
    println!("Press F1 to START recording");
    println!("Press F1 again to STOP recording");
    println!("========================\n");
    println!("Waiting for F1 to start...");

    // Poll for events until recording starts and stops
    loop {
        match recorder.poll() {
            Ok(state_changed) => {
                if state_changed {
                    if recorder.is_recording() {
                        println!("\n>>> Recording started! Perform your macro actions...");
                    } else {
                        println!(">>> Recording stopped!");
                        break;
                    }
                }
            },
            Err(e) => eprintln!("Error polling: {}", e),
        }
        thread::sleep(Duration::from_millis(1));
    }

    let events = recorder.stop();

    println!("\nSaving {} events to {}...", events.len(), output_file);
    storage::save(output_file, &events)?;
    println!("Macro saved successfully!");

    Ok(())
}

fn play_macro(input_file: &str, loop_forever: bool) -> Result<(), Box<dyn Error>> {
    println!("MacroMate Player");
    println!("================\n");

    if !Path::new(input_file).exists() {
        eprintln!("Error: File '{}' not found", input_file);
        return Ok(());
    }

    println!("Loading macro from {}...", input_file);
    let events = storage::load(input_file)?;

    println!("Loaded {} events", events.len());
    println!("\nStarting playback in 3 seconds...");

    thread::sleep(Duration::from_secs(3));

    let mut player = Player::new("macromate-playback")?;

    loop {
        player.play(&events)?;

        if loop_forever {
            println!("\nFinished macro, starting again...");
        } else {
            break;
        }
    }

    Ok(())
}
