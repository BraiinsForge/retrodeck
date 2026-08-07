# tamalib-rs

<pre>
          ..'':
         :..''
      _.-|-._
    .'   |   '.
  /'           '.
 / \.---------./ \
/  |           |  \
|  |  TamaLib  |  |
|- |           | -|
|  |           |  |
 \ /`---------'\ /
  '\  o  o  o  /'
    '-.__|__.-'
</pre>

A simple, headless emulator library for the Tamagotchi P1 virtual pet, written in Rust.

This fork is based on [Cackbone/tamalib-rs](https://github.com/Cackbone/tamalib-rs) and the [tamalib project](https://github.com/jcrona/tamalib/), which informed the CPU implementation.


[![Crates.io](https://img.shields.io/crates/v/tamalib)](https://crates.io/crates/tamalib)

---

## Features

- **CPU Emulation**: Emulates the Epson E0C6S46 CPU used in the original Tamagotchi P1.
- **Screen & Buzzer**: Abstract traits for screen and buzzer, allowing integration with any backend.
- **Button Input**: Simulate button presses (TAP, LEFT, MIDDLE, RIGHT).
- **Event-driven IO**: Uses an event bus for IO interactions.
- **Customizable Logging**: Plug in your own logger for debugging or tracing.
- **Versioned save states**: Save and restore emulation state with ROM identity and corruption checks.

---

## Getting Started

### Add to your project

```toml
[dependencies]
tamalib = "0.2.0"
```

### Example Usage

```rust
use tamalib::{Tamagotchi, TamagotchiBuilder, Button, Screen, Buzzer, Clock, Logger, LogLevel};
use std::rc::Rc;
use std::cell::RefCell;
use std::fs;

// Implement the required traits for your platform:
struct DummyScreen;
impl Screen for DummyScreen {
    fn update(&mut self) {}
    fn set_pixel(&mut self, _x: usize, _y: usize, _value: bool) {}
    fn set_icon(&mut self, _icon: usize, _value: bool) {}
}

struct DummyBuzzer;
impl Buzzer for DummyBuzzer {
    fn set_frequency(&mut self, _freq: usize) {}
    fn play(&mut self, _value: bool) {}
}

struct DummyClock;
impl Clock for DummyClock {
    fn now(&self) -> u64 { 0 }
}

struct DummyLogger;
impl Logger for DummyLogger {
    fn log(&self, _level: LogLevel, _message: &str) {}
    fn log_enabled(&self, _level: LogLevel) -> bool { true }
}

fn main() {
    let rom = fs::read("path/to/rom.bin").expect("Failed to read ROM file");
    let screen = Rc::new(RefCell::new(DummyScreen));
    let buzzer = Box::new(DummyBuzzer);
    let clock = Box::new(DummyClock);
    let logger = Box::new(DummyLogger);

    let mut tama = Tamagotchi::new(rom, screen, buzzer, clock, logger);

    // Run a single emulation step
    // The mainloop is not implemented inside the lib because it can create issues with some platforms like WASM
    tama.run_step();
}
```

---

## Save states

`Tamagotchi::save_state()` writes a versioned binary state and
`Tamagotchi::load_state()` restores it only when the loaded ROM matches. Save
states flush pending input/output events, rebase host monotonic timestamps on
restore, and refresh the screen and buzzer callbacks immediately. `run_step()`
never waits: frontends batch a bounded amount of emulated time, then use their
operating system event wait. The trailing
checksum detects accidental corruption; it is not an authenticity mechanism.

`Clock::now()` returns a monotonic microsecond timestamp as `u64`. This avoids
32-bit timestamp exhaustion on long-running targets.

---

## Library Structure

- **CPU**: Emulates the Tamagotchi's CPU, memory, and instruction set.
- **IO**: Abstracts screen, buzzer, and button input.
- **ROM**: Loading and decoding of Tamagotchi ROMs.
- **Logger**: Customizable logging interface.
- **Event Bus**: For decoupled IO event handling.

---

## Traits

You must implement the following traits for your platform:

- `Screen`: For display output.
- `Buzzer`: For sound output.
- `Clock`: For timing.
- `Logger`: For logging/debugging.

---

## License

See [LICENSE](LICENSE).

---

## Links

- [Fork GitHub](https://github.com/BraiinsForge/tamalib-rs)
- [Upstream GitHub](https://github.com/Cackbone/tamalib-rs)
- [Crates.io](https://crates.io/crates/tamalib)
