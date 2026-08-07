//! Tamagotchi P1 Deck host using the normal BMC widget surface.

use retrodeck_native::{canvas, game_audio, process, state_file, tamagotchi, wayland};
use std::cell::RefCell;
use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tamalib::{Buzzer, Clock, LogLevel, Logger, Tamagotchi};

const EMULATION_WINDOW: Duration = Duration::from_millis(4);
const STATE_SAVE_INTERVAL: Duration = Duration::from_secs(30);
const STATE_MAXIMUM_BYTES: usize = 16 * 1024;
const STATE_PATH: &str = "/mnt/data/nes-deck/state/tamagotchi.state";
const TONE_SAMPLE_RATE: u32 = 48_000;
const MAXIMUM_TONE_FRAMES: usize = TONE_SAMPLE_RATE as usize / 10;
const TONE_AMPLITUDE: i16 = 6_553;
const NANOS_PER_SECOND: u128 = 1_000_000_000;
const TICKS_PER_SECOND: u64 = 32_768;

struct HostClock(Instant);

impl Clock for HostClock {
    fn now(&self) -> u64 {
        self.0.elapsed().as_micros().try_into().unwrap_or(u64::MAX)
    }
}

struct HostLogger;

impl Logger for HostLogger {
    fn log(&self, _: LogLevel, _: &str) {}

    fn log_enabled(&self, _: LogLevel) -> bool {
        false
    }
}

#[derive(Clone, Copy)]
struct ToneState {
    frequency: usize,
    playing: bool,
}

struct HostBuzzer {
    state: Rc<RefCell<ToneState>>,
}

impl Buzzer for HostBuzzer {
    fn set_frequency(&mut self, frequency: usize) {
        self.state.borrow_mut().frequency = frequency;
    }

    fn play(&mut self, playing: bool) {
        self.state.borrow_mut().playing = playing;
    }
}

struct ToneMixer {
    state: Rc<RefCell<ToneState>>,
    previous: Instant,
    fraction: u128,
    phase: u64,
    samples: Vec<i16>,
}

impl ToneMixer {
    fn new(state: Rc<RefCell<ToneState>>) -> Self {
        Self {
            state,
            previous: Instant::now(),
            fraction: 0,
            phase: 0,
            samples: Vec::with_capacity(MAXIMUM_TONE_FRAMES * 2),
        }
    }

    fn advance(&mut self, frames: usize, frequency: usize) {
        let rate = u64::from(TONE_SAMPLE_RATE);
        self.phase = (self.phase
            + (frames % TONE_SAMPLE_RATE as usize) as u64 * frequency as u64 % rate)
            % rate;
    }

    fn write_until(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.previous);
        self.previous = now;
        let scaled = elapsed.as_nanos() * u128::from(TONE_SAMPLE_RATE) + self.fraction;
        let frames = (scaled / NANOS_PER_SECOND) as usize;
        self.fraction = scaled % NANOS_PER_SECOND;
        let state = *self.state.borrow();
        let frames_to_write = frames.min(MAXIMUM_TONE_FRAMES);
        self.advance(frames.saturating_sub(frames_to_write), state.frequency);
        self.samples.clear();
        for _ in 0..frames_to_write {
            self.advance(1, state.frequency);
            let sample = if state.playing {
                if self.phase < u64::from(TONE_SAMPLE_RATE) / 2 {
                    TONE_AMPLITUDE
                } else {
                    -TONE_AMPLITUDE
                }
            } else {
                0
            };
            self.samples.extend_from_slice(&[sample, sample]);
        }
        game_audio::write_stereo(&self.samples);
    }
}

struct TouchInput {
    down: bool,
    return_started: Option<Instant>,
    button: Option<tamagotchi::DeckButton>,
}

impl TouchInput {
    fn update(&mut self, down: bool, x: i32, y: i32, tama: &mut Tamagotchi) {
        let next_button = down.then(|| tamagotchi::button_at(x, y)).flatten();
        if self.button != next_button {
            if let Some(button) = self.button.take() {
                tama.io.set_button(button.tamalib(), false);
            }
            if let Some(button) = next_button {
                tama.io.set_button(button.tamalib(), true);
            }
            self.button = next_button;
        }
        self.down = down;
        if down {
            self.return_started.get_or_insert_with(Instant::now);
        } else {
            self.return_started = None;
        }
    }

    fn return_requested(&self) -> bool {
        self.down
            && self
                .return_started
                .is_some_and(|started| started.elapsed() >= Duration::from_secs(2))
    }

    fn release(&mut self, tama: &mut Tamagotchi) {
        if let Some(button) = self.button.take() {
            tama.io.set_button(button.tamalib(), false);
        }
        self.down = false;
        self.return_started = None;
    }
}

fn firmware_argument(arguments: &[OsString]) -> Result<Vec<u8>, String> {
    let [path] = arguments else {
        return Err("usage: tamagotchi-deck /mnt/data/nes-deck/games/tamagotchi/tama.b".to_owned());
    };
    let path = std::path::Path::new(path);
    let firmware = std::fs::read(path).map_err(|error| {
        format!(
            "cannot read Tamagotchi P1 firmware {}: {error}",
            path.display()
        )
    })?;
    if !tamagotchi::valid_firmware(&firmware) {
        return Err("Tamagotchi P1 firmware must be an exact 12 KiB P1 dump".to_owned());
    }
    Ok(firmware)
}

fn benchmark(arguments: &[OsString]) -> Result<(), String> {
    let [firmware_path, seconds] = arguments else {
        return Err(
            "usage: tamagotchi-deck --benchmark /mnt/data/nes-deck/games/tamagotchi/tama.b SECONDS"
                .to_owned(),
        );
    };
    let seconds = seconds
        .to_str()
        .ok_or_else(|| "benchmark duration must be UTF-8 seconds".to_owned())?
        .parse::<u64>()
        .map_err(|_| "benchmark duration must be an integer number of seconds".to_owned())?;
    if !(1..=60).contains(&seconds) {
        return Err("benchmark duration must be between 1 and 60 seconds".to_owned());
    }
    let firmware = firmware_argument(std::slice::from_ref(firmware_path))?;
    let (screen, _) = tamagotchi::make_screen();
    let buzzer_state = Rc::new(RefCell::new(ToneState {
        frequency: 1_000,
        playing: false,
    }));
    let mut tama = Tamagotchi::builder()
        .rom(firmware)
        .screen(screen)
        .buzzer(Box::new(HostBuzzer { state: buzzer_state }))
        .system_clock(Box::new(HostClock(Instant::now())))
        .logger(Box::new(HostLogger))
        .build();
    let started = Instant::now();
    let initial_ticks = tama.emulated_ticks();
    while started.elapsed() < Duration::from_secs(seconds) {
        let wait_ms = run_emulation_window(&mut tama);
        if wait_ms > 0 {
            std::thread::sleep(Duration::from_millis(u64::from(wait_ms)));
        }
    }
    report_speed(&tama, started, initial_ticks);
    Ok(())
}

fn restore_state(tama: &mut Tamagotchi, path: &Path) -> Result<(), String> {
    match state_file::read_bounded(path, STATE_MAXIMUM_BYTES)? {
        state_file::StateRead::Missing => Ok(()),
        state_file::StateRead::Value(bytes) => match tama.load_state(&bytes) {
            Ok(()) => {
                eprintln!("tamagotchi-deck: restored {}", path.display());
                Ok(())
            }
            Err(error) => {
                eprintln!(
                    "tamagotchi-deck: ignoring invalid state {}: {error}",
                    path.display()
                );
                Ok(())
            }
        },
    }
}

fn save_state(tama: &mut Tamagotchi, path: &Path) -> Result<(), String> {
    let bytes = tama
        .save_state()
        .map_err(|error| format!("cannot save Tamagotchi state: {error}"))?;
    state_file::write_bounded(path, &bytes, STATE_MAXIMUM_BYTES)
}

fn report_speed(tama: &Tamagotchi, started: Instant, initial_ticks: u64) {
    let elapsed = started.elapsed().as_secs_f64();
    if elapsed == 0.0 {
        return;
    }
    let emulated_ticks = tama.emulated_ticks().wrapping_sub(initial_ticks);
    let emulated_seconds = emulated_ticks as f64 / TICKS_PER_SECOND as f64;
    eprintln!(
        "tamagotchi-deck: emulated {emulated_seconds:.3}s in {elapsed:.3}s ({:.1}% realtime)",
        emulated_seconds / elapsed * 100.0
    );
}

fn open_audio() {
    let volume = game_audio::volume_percent().unwrap_or_else(|error| {
        eprintln!("tamagotchi-deck: {error}");
        0
    });
    if let Err(error) = game_audio::open(TONE_SAMPLE_RATE, volume) {
        eprintln!("tamagotchi-deck: sound disabled: {error}");
    }
}

fn present(screen: &tamagotchi::ScreenState) -> Result<(), String> {
    tamagotchi::render(screen);
    canvas::with_pixels(wayland::present_rgba)
}

fn run_emulation_window(tama: &mut Tamagotchi) -> u32 {
    let target = tama
        .current_timestamp()
        .saturating_add(EMULATION_WINDOW.as_micros() as u64);
    while tama.emulated_timestamp() < target {
        let before = tama.emulated_timestamp();
        tama.run_step();
        if tama.emulated_timestamp() == before {
            return EMULATION_WINDOW.as_millis() as u32;
        }
    }
    let remaining = tama
        .emulated_timestamp()
        .saturating_sub(tama.current_timestamp());
    ((remaining.saturating_add(999)) / 1_000).min(u64::from(u32::MAX)) as u32
}

fn run(arguments: &[OsString]) -> Result<(), String> {
    let firmware = firmware_argument(arguments)?;
    process::install_signal_handlers()?;
    let (screen, state) = tamagotchi::make_screen();
    let buzzer_state = Rc::new(RefCell::new(ToneState {
        frequency: 1_000,
        playing: false,
    }));
    let mut tama = Tamagotchi::builder()
        .rom(firmware)
        .screen(screen)
        .buzzer(Box::new(HostBuzzer {
            state: buzzer_state.clone(),
        }))
        .system_clock(Box::new(HostClock(Instant::now())))
        .logger(Box::new(HostLogger))
        .build();
    let state_path = Path::new(STATE_PATH);
    restore_state(&mut tama, state_path)?;
    tamagotchi::release_all_buttons(&mut tama);
    let started = Instant::now();
    let initial_ticks = tama.emulated_ticks();
    wayland::open_game_widget()?;
    open_audio();
    let mut mixer = ToneMixer::new(buzzer_state);

    let mut input = TouchInput {
        down: false,
        return_started: None,
        button: None,
    };
    let mut redraw = true;
    let mut next_state_save = Instant::now() + STATE_SAVE_INTERVAL;
    let mut result = (|| -> Result<(), String> {
        loop {
            let wait_ms = run_emulation_window(&mut tama);
            mixer.write_until(Instant::now());
            wayland::dispatch(wait_ms)?;
            while let Some(touch) = wayland::next_touch() {
                input.update(touch.down, touch.x, touch.y, &mut tama);
                redraw = true;
            }
            redraw |= state.borrow_mut().take_dirty();
            if redraw {
                present(&state.borrow())?;
                redraw = false;
            }
            let now = Instant::now();
            if now >= next_state_save {
                if let Err(error) = save_state(&mut tama, state_path) {
                    eprintln!("tamagotchi-deck: {error}");
                }
                next_state_save = now + STATE_SAVE_INTERVAL;
            }
            if input.return_requested()
                || process::shutdown_requested()
                || wayland::shutdown_requested()
            {
                break Ok(());
            }
        }
    })();
    input.release(&mut tama);
    if let Err(error) = save_state(&mut tama, state_path) {
        eprintln!("tamagotchi-deck: {error}");
        if result.is_ok() {
            result = Err(error);
        }
    }
    report_speed(&tama, started, initial_ticks);
    game_audio::close();
    wayland::close();
    result
}

fn main() -> ExitCode {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let result = match arguments.first().and_then(|argument| argument.to_str()) {
        Some("--benchmark") => benchmark(&arguments[1..]),
        _ => run(&arguments),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tamagotchi-deck: {error}");
            ExitCode::from(1)
        }
    }
}
