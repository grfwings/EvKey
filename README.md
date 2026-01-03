# EvKey

EvKey is a fast, minimal keyboard & mouse automation tool for Linux. It uses [libevdev](https://www.freedesktop.org/wiki/Software/libevdev/) for event handling, which means it can be used with Wayland or X11. EvKey was inspired primarily by the [AutoHotkey](https://www.autohotkey.com/) and [ydotool](https://www.autohotkey.com/) projects.

## Features

- Record keyboard and mouse input events (keys, buttons, movement, wheel)
- Simple scripting language
- Display server agnostic, all you need is a kernel!

## Requirements

- A linux kernel with `evdev`
- Root access or permissions to read `/dev/input/event*` devices

## Installation

```bash
cargo build --release
sudo cp target/release/evkey /usr/local/bin/
```

## Usage

### Record a macro

```bash
evkey record my_macro.macro
```

### Play back a macro

```bash
evkey play my_macro.macro
```

## File Format

Coming soon!

## Future Enhancements

- [x] Hotkey detection to start/stop recording
- [x] Repeat/loop playback
- [ ] Configurable hotkeys (currently F1 is hardcoded)
- [ ] Better scripting language
- [ ] X keyboard extension support

## License

GPL-3.0-or-later
