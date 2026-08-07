mod cpu;
mod io;
mod rom;
mod logger;
mod snapshot;
use crate::cpu::{Cpu, InputPin};
use crate::io::iobus::{Event, IOBus};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;


pub use io::{Button, Buzzer, IO, Screen, SCREEN_WIDTH, SCREEN_HEIGHT, ICONS_COUNT};
pub use cpu::Clock;
pub use logger::{LogLevel, Logger};
pub use snapshot::{SnapshotError, SNAPSHOT_VERSION};

/// Clock timestamp units per second. [`Clock::now`] returns monotonic microseconds.
pub const TIMESTAMP_FREQUENCY: u64 = 1_000_000;


#[derive(PartialEq, Eq)]
pub enum ExecMode {
    Pause,
    Run,
    Step,
    Next,
    ToCall,
    ToReturn
}


#[derive(Default)]
pub struct TamagotchiBuilder {
    rom: Option<Vec<u8>>,
    screen: Option<Rc<RefCell<dyn Screen>>>,
    buzzer: Option<Box<dyn Buzzer>>,
    system_clock: Option<Box<dyn Clock>>,
    logger: Option<Box<dyn Logger>>,
}

impl TamagotchiBuilder {
    pub fn rom(mut self, rom: Vec<u8>) -> Self {
        self.rom = Some(rom);
        self
    }

    pub fn screen(mut self, screen: Rc<RefCell<dyn Screen>>) -> Self {
        self.screen = Some(screen);
        self
    }

    pub fn buzzer(mut self, buzzer: Box<dyn Buzzer>) -> Self {
        self.buzzer = Some(buzzer);
        self
    }

    pub fn system_clock(mut self, system_clock: Box<dyn Clock>) -> Self {
        self.system_clock = Some(system_clock);
        self
    }

    pub fn logger(mut self, logger: Box<dyn Logger>) -> Self {
        self.logger = Some(logger);
        self
    }

    pub fn build(self) -> Tamagotchi {
        let rom = self.rom.unwrap();
        let screen = self.screen.unwrap();
        let buzzer = self.buzzer.unwrap();
        let system_clock = self.system_clock.unwrap();
        let logger = self.logger.unwrap();
        Tamagotchi::new(rom, screen, buzzer, system_clock, logger)
    }
}





pub struct Tamagotchi {
    cpu: Cpu,
    pub io: IO,
    event_queue: Rc<RefCell<Vec<Event>>>,
    pub framerate: usize,
    exec_mode: ExecMode,
    depth: usize,
    ts_freq: u64,
    screen_ts: u64,
    logger: Rc<RefCell<Box<dyn Logger>>>,
}

impl Tamagotchi {

    pub fn new(rom_bytes: Vec<u8>, screen: Rc<RefCell<dyn Screen>>, buzzer: Box<dyn Buzzer>, system_clock: Box<dyn Clock>, logger: Box<dyn Logger>) -> Self {
        let bus = Rc::new(IOBus::new());
        let event_queue = Rc::new(RefCell::new(Vec::new()));
        let event_queue_clone = event_queue.clone();
        bus.subscribe(Rc::new(move |event| {
            event_queue_clone.borrow_mut().push(event.clone());
        }));
        let breakpoints: VecDeque<usize> = VecDeque::from([]);
        let rom = rom::load_rom(rom_bytes);
        let logger = Rc::new(RefCell::new(logger));
        let cpu = Cpu::new(rom, TIMESTAMP_FREQUENCY, bus.clone(), system_clock, logger.clone(), breakpoints);
        let io = IO::new(screen, buzzer, bus.clone());
        let screen_ts = cpu.clock.system_clock.now();
        Self {
            cpu,
            io,
            event_queue,
            framerate: 30,
            exec_mode: ExecMode::Run,
            depth: 0,
            ts_freq: TIMESTAMP_FREQUENCY,
            screen_ts,
            logger: logger.clone(),
        }
    }

    pub fn builder() -> TamagotchiBuilder {
        TamagotchiBuilder::default()
    }

    pub fn run_step(&mut self) {
        self.step();
        if self.framerate == 0 {
            return;
        }
        let now = self.cpu.clock.system_clock.now();
        let elapsed = now.saturating_sub(self.screen_ts);

        if elapsed >= self.ts_freq / self.framerate as u64 {
            self.screen_ts = now;
            self.io.screen.borrow_mut().update();
        }
    }

    /// Returns the wrapping 32,768 Hz emulated clock tick count.
    pub fn emulated_ticks(&self) -> u64 {
        self.cpu.clock.tick_counter
    }

    pub fn set_mode(&mut self, mode: ExecMode) {
        self.exec_mode = mode;
        self.depth = self.cpu.call_depth;
        self.cpu.sync_ref_timestamp();
    }

    pub fn set_speed(&mut self, speed: u8) {
        self.cpu.set_speed(speed);
        self.logger.borrow().log(LogLevel::Info, &format!("Speed set to {}", speed));
    }

    pub fn step(&mut self) {
        if self.exec_mode == ExecMode::Pause {
            return;
        }
        self.process_events();

        if self.cpu.step() {
            self.exec_mode = ExecMode::Pause;
            self.depth = self.cpu.call_depth;
            return;
        }

        match self.exec_mode {
            ExecMode::Pause | ExecMode::Run => {}
            ExecMode::Step => {
                self.exec_mode = ExecMode::Pause;
            }
            ExecMode::Next => {
                if self.cpu.call_depth <= self.depth {
                    self.exec_mode = ExecMode::Pause;
                    self.depth = self.cpu.call_depth;
                }
            }
            ExecMode::ToCall => {
                if self.cpu.call_depth > self.depth {
                    self.exec_mode = ExecMode::Pause;
                    self.depth = self.cpu.call_depth;
                }
            }
            ExecMode::ToReturn => {
                if self.cpu.call_depth < self.depth {
                    self.exec_mode = ExecMode::Pause;
                    self.depth = self.cpu.call_depth;
                }
            }
        }
    }


    pub fn process_events(&mut self) {
        let mut queue = self.event_queue.borrow_mut();
        for event in queue.drain(..) {
            match event {
                Event::ButtonPressed { pin, value } => {
                    if let Some(pin_enum) = InputPin::from_u8(pin) {
                        self.cpu.set_pin(pin_enum, value);
                    }
                }
                Event::BuzzerFreqSet(freq) => {
                    self.io.set_buzzer_freq(freq);
                }
                Event::BuzzerPlay(enabled) => {
                    self.io.play_buzzer(enabled);
                }
                Event::ScreenPinSet { seg, com, value } => {
                    self.io.set_screen_pin(seg, com, value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Default)]
    struct RecordingScreen {
        updates: usize,
        pixel_writes: usize,
    }

    impl Screen for RecordingScreen {
        fn update(&mut self) {
            self.updates += 1;
        }

        fn set_pixel(&mut self, _x: usize, _y: usize, _value: bool) {
            self.pixel_writes += 1;
        }

        fn set_icon(&mut self, _icon: usize, _value: bool) {}
    }

    struct DummyBuzzer;
    impl Buzzer for DummyBuzzer {
        fn set_frequency(&mut self, _freq: usize) {}
        fn play(&mut self, _value: bool) {}
    }

    struct ManualClock(Rc<Cell<u64>>);
    impl Clock for ManualClock {
        fn now(&self) -> u64 {
            self.0.get()
        }
    }

    struct AdvancingClock(Rc<Cell<u64>>);
    impl Clock for AdvancingClock {
        fn now(&self) -> u64 {
            let now = self.0.get();
            self.0.set(now.saturating_add(1));
            now
        }
    }

    struct DummyLogger;
    impl Logger for DummyLogger {
        fn log(&self, _level: LogLevel, _message: &str) {}
        fn log_enabled(&self, _level: LogLevel) -> bool {
            true
        }
    }

    fn test_rom() -> Vec<u8> {
        vec![0; 12288]
    }

    fn make_tamagotchi(
        rom: Vec<u8>,
        timestamp: u64,
    ) -> (Tamagotchi, Rc<Cell<u64>>, Rc<RefCell<RecordingScreen>>) {
        let clock = Rc::new(Cell::new(timestamp));
        let recorded_screen = Rc::new(RefCell::new(RecordingScreen::default()));
        let screen: Rc<RefCell<dyn Screen>> = recorded_screen.clone();
        let tama = Tamagotchi::new(
            rom,
            screen,
            Box::new(DummyBuzzer),
            Box::new(ManualClock(clock.clone())),
            Box::new(DummyLogger),
        );
        (tama, clock, recorded_screen)
    }

    fn rewrite_checksum(snapshot: &mut [u8]) {
        let checksum_offset = snapshot.len() - std::mem::size_of::<u64>();
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in &snapshot[..checksum_offset] {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        snapshot[checksum_offset..].copy_from_slice(&hash.to_le_bytes());
    }

    #[test]
    fn constructs_a_tamagotchi() {
        let (tama, _, _) = make_tamagotchi(test_rom(), 0);
        assert_eq!(tama.cpu.pc, 0x100);
    }

    #[test]
    fn snapshot_round_trip_restores_cpu_and_input_state() {
        let (mut tama, _, _) = make_tamagotchi(test_rom(), 500);
        tama.set_speed(0);
        for _ in 0..32 {
            tama.run_step();
        }
        tama.io.set_button(Button::LEFT, true);
        let snapshot = tama.save_state().unwrap();
        let expected_pc = tama.cpu.pc;
        let expected_memory = tama.cpu.memory;
        let expected_ticks = tama.cpu.clock.tick_counter;
        let expected_inputs = tama.cpu.inputs.ports;

        tama.cpu.pc = 1;
        tama.cpu.memory[42] ^= 0xff;
        tama.cpu.clock.tick_counter = 0;
        tama.cpu.inputs.ports = [0, 0];

        tama.load_state(&snapshot).unwrap();
        assert_eq!(tama.cpu.pc, expected_pc);
        assert_eq!(tama.cpu.memory, expected_memory);
        assert_eq!(tama.cpu.clock.tick_counter, expected_ticks);
        assert_eq!(tama.cpu.inputs.ports, expected_inputs);
    }

    #[test]
    fn snapshot_rejects_corruption_without_mutating_state() {
        let (mut tama, _, _) = make_tamagotchi(test_rom(), 500);
        let snapshot = tama.save_state().unwrap();
        let before_pc = tama.cpu.pc;
        let before_memory = tama.cpu.memory;

        let mut corrupt = snapshot.clone();
        corrupt[32] ^= 0xff;
        assert_eq!(tama.load_state(&corrupt), Err(SnapshotError::ChecksumMismatch));
        assert_eq!(tama.cpu.pc, before_pc);
        assert_eq!(tama.cpu.memory, before_memory);

        let mut bad_version = snapshot.clone();
        bad_version[8] = SNAPSHOT_VERSION.wrapping_add(1) as u8;
        rewrite_checksum(&mut bad_version);
        assert_eq!(
            tama.load_state(&bad_version),
            Err(SnapshotError::UnsupportedVersion(SNAPSHOT_VERSION + 1))
        );
        assert_eq!(tama.cpu.pc, before_pc);
        assert_eq!(tama.cpu.memory, before_memory);

        let mut truncated = snapshot;
        truncated.pop();
        assert!(matches!(
            tama.load_state(&truncated),
            Err(SnapshotError::ChecksumMismatch | SnapshotError::Truncated)
        ));
        assert_eq!(tama.cpu.pc, before_pc);
        assert_eq!(tama.cpu.memory, before_memory);
    }

    #[test]
    fn snapshot_rejects_a_different_rom() {
        let (mut source, _, _) = make_tamagotchi(test_rom(), 500);
        let snapshot = source.save_state().unwrap();
        let mut different_rom = test_rom();
        different_rom[0] = 1;
        let (mut target, _, _) = make_tamagotchi(different_rom, 500);
        let before_pc = target.cpu.pc;
        assert_eq!(target.load_state(&snapshot), Err(SnapshotError::RomMismatch));
        assert_eq!(target.cpu.pc, before_pc);
    }

    #[test]
    fn restore_rebases_time_and_refreshes_output() {
        let (mut source, _, _) = make_tamagotchi(test_rom(), 100);
        source.cpu.memory[cpu::MEM_DISPLAY1_ADDR] = 1;
        source.cpu.clock.ref_ts = 90;
        source.screen_ts = 95;
        let snapshot = source.save_state().unwrap();

        let (mut target, _, screen) = make_tamagotchi(test_rom(), 1_000_000);
        target.load_state(&snapshot).unwrap();
        assert_eq!(target.cpu.clock.ref_ts, 999_990);
        assert_eq!(target.screen_ts, 999_995);
        assert!(screen.borrow().pixel_writes > 0);
        assert_eq!(screen.borrow().updates, 1);
    }

    #[test]
    fn restored_state_steps_deterministically() {
        let (mut source, _, _) = make_tamagotchi(test_rom(), 100);
        source.set_speed(0);
        for _ in 0..16 {
            source.run_step();
        }
        let snapshot = source.save_state().unwrap();

        let (mut first, _, _) = make_tamagotchi(test_rom(), 200);
        let (mut second, _, _) = make_tamagotchi(test_rom(), 200);
        first.load_state(&snapshot).unwrap();
        second.load_state(&snapshot).unwrap();
        for _ in 0..64 {
            first.run_step();
            second.run_step();
        }
        assert_eq!(first.save_state().unwrap(), second.save_state().unwrap());
    }

    #[test]
    fn u64_clock_survives_past_the_32_bit_microsecond_limit() {
        let beyond_u32 = u32::MAX as u64 + 1;
        let (mut tama, clock, screen) = make_tamagotchi(test_rom(), beyond_u32);
        tama.set_speed(0);
        assert_eq!(tama.cpu.clock.ref_ts, beyond_u32);
        clock.set(beyond_u32 + 1_000_000);
        tama.run_step();
        assert_eq!(tama.cpu.clock.ref_ts, beyond_u32 + 1_000_000);
        assert_eq!(screen.borrow().updates, 1);
    }

    #[test]
    fn snapshot_rejects_checked_invalid_state_without_mutation() {
        const CPU_FREQUENCY_OFFSET: usize = 4250;
        const PROGRAMMABLE_TIMER_ENABLED_OFFSET: usize = 4221;

        let (mut tama, _, _) = make_tamagotchi(test_rom(), 500);
        let snapshot = tama.save_state().unwrap();
        let before_pc = tama.cpu.pc;

        let mut invalid_frequency = snapshot.clone();
        invalid_frequency[CPU_FREQUENCY_OFFSET..CPU_FREQUENCY_OFFSET + 8]
            .copy_from_slice(&123u64.to_le_bytes());
        rewrite_checksum(&mut invalid_frequency);
        assert_eq!(
            tama.load_state(&invalid_frequency),
            Err(SnapshotError::InvalidState("CPU frequency"))
        );
        assert_eq!(tama.cpu.pc, before_pc);

        let mut invalid_boolean = snapshot;
        invalid_boolean[PROGRAMMABLE_TIMER_ENABLED_OFFSET] = 2;
        rewrite_checksum(&mut invalid_boolean);
        assert_eq!(
            tama.load_state(&invalid_boolean),
            Err(SnapshotError::InvalidState("boolean"))
        );
        assert_eq!(tama.cpu.pc, before_pc);
    }

    #[test]
    fn snapshot_rejects_bad_magic_and_trailing_payload() {
        let (mut tama, _, _) = make_tamagotchi(test_rom(), 500);
        let snapshot = tama.save_state().unwrap();

        let mut bad_magic = snapshot.clone();
        bad_magic[0] ^= 0xff;
        rewrite_checksum(&mut bad_magic);
        assert_eq!(tama.load_state(&bad_magic), Err(SnapshotError::InvalidMagic));

        let mut trailing = snapshot;
        let checksum_offset = trailing.len() - std::mem::size_of::<u64>();
        trailing.insert(checksum_offset, 0);
        rewrite_checksum(&mut trailing);
        assert_eq!(tama.load_state(&trailing), Err(SnapshotError::TrailingData));
    }

    #[test]
    fn save_rejects_an_invalid_public_framerate() {
        let (mut tama, _, _) = make_tamagotchi(test_rom(), 500);
        tama.framerate = 0;
        assert_eq!(
            tama.save_state(),
            Err(SnapshotError::InvalidState("framerate"))
        );
        tama.set_speed(0);
        tama.run_step();
    }

    #[test]
    fn normal_speed_restore_uses_the_rebased_clock() {
        let source_clock = Rc::new(Cell::new(1_000));
        let source_screen = Rc::new(RefCell::new(RecordingScreen::default()));
        let source_screen_io: Rc<RefCell<dyn Screen>> = source_screen;
        let mut source = Tamagotchi::new(
            test_rom(),
            source_screen_io,
            Box::new(DummyBuzzer),
            Box::new(AdvancingClock(source_clock)),
            Box::new(DummyLogger),
        );
        source.run_step();
        source.run_step();
        let snapshot = source.save_state().unwrap();

        let target_clock = Rc::new(Cell::new(10_000));
        let target_screen = Rc::new(RefCell::new(RecordingScreen::default()));
        let target_screen_io: Rc<RefCell<dyn Screen>> = target_screen;
        let mut target = Tamagotchi::new(
            test_rom(),
            target_screen_io,
            Box::new(DummyBuzzer),
            Box::new(AdvancingClock(target_clock)),
            Box::new(DummyLogger),
        );
        target.load_state(&snapshot).unwrap();
        let before_ticks = target.cpu.clock.tick_counter;
        target.run_step();
        assert!(target.cpu.clock.tick_counter > before_ticks);
    }
}
