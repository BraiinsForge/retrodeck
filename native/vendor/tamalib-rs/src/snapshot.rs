use crate::cpu::{
    CPU_OSC3_CTRL_ADDR, MEM_SIZE, OSC1_FREQUENCY, OSC3_FREQUENCY, TICK_FREQUENCY, TIMER_PERIODS,
};
use crate::TIMESTAMP_FREQUENCY;
use crate::{ExecMode, Tamagotchi};
use std::fmt;

const SNAPSHOT_MAGIC: [u8; 8] = *b"TAMASAV\0";

/// Version of the stable binary save-state format.
pub const SNAPSHOT_VERSION: u16 = 1;

/// A malformed or incompatible Tamagotchi save state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    InvalidMagic,
    UnsupportedVersion(u16),
    Truncated,
    TrailingData,
    ChecksumMismatch,
    RomMismatch,
    InvalidState(&'static str),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "invalid Tamagotchi snapshot magic"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported Tamagotchi snapshot version {version}")
            }
            Self::Truncated => write!(f, "truncated Tamagotchi snapshot"),
            Self::TrailingData => write!(f, "trailing data in Tamagotchi snapshot"),
            Self::ChecksumMismatch => write!(f, "corrupt Tamagotchi snapshot checksum"),
            Self::RomMismatch => write!(f, "Tamagotchi snapshot ROM does not match"),
            Self::InvalidState(field) => write!(f, "invalid Tamagotchi snapshot field {field}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

#[derive(Clone, Copy, Default)]
struct TimerState {
    ts: u64,
}

#[derive(Clone, Copy, Default)]
struct InterruptState {
    triggered: bool,
    factor: u8,
    mask: u8,
}

struct SnapshotState {
    saved_now: u64,
    pc: usize,
    next_pc: usize,
    sp: u8,
    np: u8,
    a: u8,
    b: u8,
    x: u16,
    y: u16,
    flags: u8,
    memory: [u8; MEM_SIZE],
    timers: [TimerState; 8],
    prog_timer_enabled: bool,
    prog_timer_ts: u64,
    prog_timer_reload: u8,
    prog_timer_data: u8,
    tick_counter: u64,
    speed_ratio: u8,
    ref_ts: u64,
    cpu_halted: bool,
    cpu_freq: u64,
    scaled_cycle_accumulator: u64,
    previous_cycles: u8,
    call_depth: usize,
    interrupts: [InterruptState; 6],
    input_ports: [u8; 2],
    framerate: usize,
    exec_mode: ExecMode,
    depth: usize,
    screen_ts: u64,
}

impl Tamagotchi {
    /// Saves the complete emulation state in a versioned, checksummed binary format.
    ///
    /// Pending input and output events are first applied, so callbacks and external
    /// screen or buzzer objects are never serialized.
    pub fn save_state(&mut self) -> Result<Vec<u8>, SnapshotError> {
        validate_state(self)?;
        self.process_events();

        let saved_now = self.cpu.clock.system_clock.now();
        let mut writer = SnapshotWriter::default();
        writer.bytes(&SNAPSHOT_MAGIC);
        writer.u16(SNAPSHOT_VERSION);
        writer.u16(0);
        writer.u64(self.cpu.program.len() as u64);
        writer.u64(program_fingerprint(&self.cpu.program));
        writer.u64(saved_now);

        writer.u64(self.cpu.pc as u64);
        writer.u64(self.cpu.next_pc as u64);
        writer.u8(self.cpu.sp);
        writer.u8(self.cpu.np);
        writer.u8(self.cpu.a);
        writer.u8(self.cpu.b);
        writer.u16(self.cpu.x);
        writer.u16(self.cpu.y);
        writer.u8(self.cpu.flags.0);
        writer.bytes(&self.cpu.memory);

        for timer in &self.cpu.clock.timers {
            writer.u64(timer.ts);
        }
        writer.bool(self.cpu.clock.prog_timer.enabled);
        writer.u64(self.cpu.clock.prog_timer.ts);
        writer.u8(self.cpu.clock.prog_timer.reload);
        writer.u8(self.cpu.clock.prog_timer.data);
        writer.u64(self.cpu.clock.tick_counter);
        writer.u8(self.cpu.clock.speed_ratio);
        writer.u64(self.cpu.clock.ref_ts);
        writer.bool(self.cpu.clock.cpu_halted);
        writer.u64(self.cpu.clock.cpu_freq);
        writer.u64(self.cpu.clock.scaled_cycle_accumulator);
        writer.u8(self.cpu.clock.previous_cycles);

        writer.u64(self.cpu.call_depth as u64);
        for interrupt in &self.cpu.interrupts {
            writer.bool(interrupt.triggered);
            writer.u8(interrupt.factor);
            writer.u8(interrupt.mask);
        }
        writer.bytes(&self.cpu.inputs.ports);

        writer.u64(self.framerate as u64);
        writer.u8(exec_mode_code(&self.exec_mode));
        writer.u64(self.depth as u64);
        writer.u64(self.screen_ts);

        let checksum = fnv1a64(&writer.bytes);
        writer.u64(checksum);
        Ok(writer.bytes)
    }

    /// Restores a state created by [`Self::save_state`].
    ///
    /// The currently loaded ROM must match the snapshot. Host monotonic timestamps
    /// are rebased to the current clock, then LCD and buzzer outputs are refreshed.
    pub fn load_state(&mut self, bytes: &[u8]) -> Result<(), SnapshotError> {
        let state = parse_snapshot(bytes, &self.cpu.program)?;
        let now = self.cpu.clock.system_clock.now();

        self.cpu.pc = state.pc;
        self.cpu.next_pc = state.next_pc;
        self.cpu.sp = state.sp;
        self.cpu.np = state.np;
        self.cpu.a = state.a;
        self.cpu.b = state.b;
        self.cpu.x = state.x;
        self.cpu.y = state.y;
        self.cpu.flags.0 = state.flags;
        self.cpu.memory = state.memory;

        for (timer, saved) in self.cpu.clock.timers.iter_mut().zip(state.timers) {
            timer.ts = saved.ts;
        }
        self.cpu.clock.prog_timer.enabled = state.prog_timer_enabled;
        self.cpu.clock.prog_timer.ts = state.prog_timer_ts;
        self.cpu.clock.prog_timer.reload = state.prog_timer_reload;
        self.cpu.clock.prog_timer.data = state.prog_timer_data;
        self.cpu.clock.tick_counter = state.tick_counter;
        self.cpu.clock.speed_ratio = state.speed_ratio;
        self.cpu.clock.ref_ts = rebase_timestamp(state.saved_now, state.ref_ts, now);
        self.cpu.clock.cpu_halted = state.cpu_halted;
        self.cpu.clock.cpu_freq = state.cpu_freq;
        self.cpu.clock.scaled_cycle_accumulator = state.scaled_cycle_accumulator;
        self.cpu.clock.previous_cycles = state.previous_cycles;

        self.cpu.call_depth = state.call_depth;
        for (interrupt, saved) in self.cpu.interrupts.iter_mut().zip(state.interrupts) {
            interrupt.triggered = saved.triggered;
            interrupt.factor = saved.factor;
            interrupt.mask = saved.mask;
        }
        self.cpu.inputs.ports = state.input_ports;

        self.framerate = state.framerate;
        self.exec_mode = state.exec_mode;
        self.depth = state.depth;
        self.screen_ts = rebase_timestamp(state.saved_now, state.screen_ts, now);

        self.event_queue.borrow_mut().clear();
        self.cpu.refresh_io();
        self.process_events();
        self.io.screen.borrow_mut().update();
        Ok(())
    }
}

fn validate_state(tama: &Tamagotchi) -> Result<(), SnapshotError> {
    if tama.cpu.pc >= tama.cpu.program.len() || tama.cpu.next_pc >= tama.cpu.program.len() {
        return Err(SnapshotError::InvalidState("program counter"));
    }
    if tama.framerate == 0 {
        return Err(SnapshotError::InvalidState("framerate"));
    }
    if tama.cpu.clock.ts_freq != TIMESTAMP_FREQUENCY || tama.ts_freq != TIMESTAMP_FREQUENCY {
        return Err(SnapshotError::InvalidState("timestamp frequency"));
    }
    if tama.cpu.flags.0 & !0x0f != 0 {
        return Err(SnapshotError::InvalidState("flags"));
    }
    if !supported_cpu_frequency(tama.cpu.clock.cpu_freq, &tama.cpu.memory) {
        return Err(SnapshotError::InvalidState("CPU frequency"));
    }
    if tama.cpu.clock.scaled_cycle_accumulator >= tama.cpu.clock.cpu_freq {
        return Err(SnapshotError::InvalidState("cycle accumulator"));
    }
    for (timer, period) in tama.cpu.clock.timers.iter().zip(TIMER_PERIODS) {
        if timer.period != period || tama.cpu.clock.tick_counter.wrapping_sub(timer.ts) >= period {
            return Err(SnapshotError::InvalidState("timer timestamp"));
        }
    }
    if tama.cpu.clock.prog_timer.enabled
        && tama
            .cpu
            .clock
            .tick_counter
            .wrapping_sub(tama.cpu.clock.prog_timer.ts)
            >= TICK_FREQUENCY / 256
    {
        return Err(SnapshotError::InvalidState("programmable timer timestamp"));
    }
    Ok(())
}

fn supported_cpu_frequency(cpu_freq: u64, memory: &[u8; MEM_SIZE]) -> bool {
    let expected = if memory[CPU_OSC3_CTRL_ADDR] & 0x8 != 0 {
        OSC3_FREQUENCY
    } else {
        OSC1_FREQUENCY
    };
    cpu_freq == expected
}

fn parse_snapshot(bytes: &[u8], program: &[u16]) -> Result<SnapshotState, SnapshotError> {
    const CHECKSUM_SIZE: usize = std::mem::size_of::<u64>();
    if bytes.len() < CHECKSUM_SIZE {
        return Err(SnapshotError::Truncated);
    }

    let (payload, checksum_bytes) = bytes.split_at(bytes.len() - CHECKSUM_SIZE);
    let expected_checksum = u64::from_le_bytes(
        checksum_bytes
            .try_into()
            .map_err(|_| SnapshotError::Truncated)?,
    );
    if fnv1a64(payload) != expected_checksum {
        return Err(SnapshotError::ChecksumMismatch);
    }

    let mut reader = SnapshotReader::new(payload);
    if reader.array::<8>()? != SNAPSHOT_MAGIC {
        return Err(SnapshotError::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != SNAPSHOT_VERSION {
        return Err(SnapshotError::UnsupportedVersion(version));
    }
    if reader.u16()? != 0 {
        return Err(SnapshotError::InvalidState("reserved"));
    }
    if reader.u64()? != program.len() as u64 || reader.u64()? != program_fingerprint(program) {
        return Err(SnapshotError::RomMismatch);
    }
    let saved_now = reader.u64()?;

    let pc = reader.usize("pc")?;
    let next_pc = reader.usize("next_pc")?;
    if pc >= program.len() || next_pc >= program.len() {
        return Err(SnapshotError::InvalidState("program counter"));
    }
    let sp = reader.u8()?;
    let np = reader.u8()?;
    let a = reader.u8()?;
    let b = reader.u8()?;
    let x = reader.u16()?;
    let y = reader.u16()?;
    let flags = reader.u8()?;
    if flags & !0x0f != 0 {
        return Err(SnapshotError::InvalidState("flags"));
    }
    let memory = reader.array::<MEM_SIZE>()?;

    let mut timers = [TimerState::default(); 8];
    for timer in &mut timers {
        timer.ts = reader.u64()?;
    }
    let prog_timer_enabled = reader.bool()?;
    let prog_timer_ts = reader.u64()?;
    let prog_timer_reload = reader.u8()?;
    let prog_timer_data = reader.u8()?;
    let tick_counter = reader.u64()?;
    for (timer, period) in timers.iter().zip(TIMER_PERIODS) {
        if tick_counter.wrapping_sub(timer.ts) >= period {
            return Err(SnapshotError::InvalidState("timer timestamp"));
        }
    }
    if prog_timer_enabled && tick_counter.wrapping_sub(prog_timer_ts) >= TICK_FREQUENCY / 256 {
        return Err(SnapshotError::InvalidState("programmable timer timestamp"));
    }
    let speed_ratio = reader.u8()?;
    let ref_ts = reader.u64()?;
    let cpu_halted = reader.bool()?;
    let cpu_freq = reader.u64()?;
    if !supported_cpu_frequency(cpu_freq, &memory) {
        return Err(SnapshotError::InvalidState("CPU frequency"));
    }
    let scaled_cycle_accumulator = reader.u64()?;
    if scaled_cycle_accumulator >= cpu_freq {
        return Err(SnapshotError::InvalidState("cycle accumulator"));
    }
    let previous_cycles = reader.u8()?;

    let call_depth = reader.usize("call depth")?;
    let mut interrupts = [InterruptState::default(); 6];
    for interrupt in &mut interrupts {
        interrupt.triggered = reader.bool()?;
        interrupt.factor = reader.u8()?;
        interrupt.mask = reader.u8()?;
    }
    let input_ports = reader.array::<2>()?;

    let framerate = reader.usize("framerate")?;
    if framerate == 0 {
        return Err(SnapshotError::InvalidState("framerate"));
    }
    let exec_mode = exec_mode_from_code(reader.u8()?)?;
    let depth = reader.usize("debug depth")?;
    let screen_ts = reader.u64()?;
    reader.finish()?;

    Ok(SnapshotState {
        saved_now,
        pc,
        next_pc,
        sp,
        np,
        a,
        b,
        x,
        y,
        flags,
        memory,
        timers,
        prog_timer_enabled,
        prog_timer_ts,
        prog_timer_reload,
        prog_timer_data,
        tick_counter,
        speed_ratio,
        ref_ts,
        cpu_halted,
        cpu_freq,
        scaled_cycle_accumulator,
        previous_cycles,
        call_depth,
        interrupts,
        input_ports,
        framerate,
        exec_mode,
        depth,
        screen_ts,
    })
}

fn exec_mode_code(mode: &ExecMode) -> u8 {
    match mode {
        ExecMode::Pause => 0,
        ExecMode::Run => 1,
        ExecMode::Step => 2,
        ExecMode::Next => 3,
        ExecMode::ToCall => 4,
        ExecMode::ToReturn => 5,
    }
}

fn exec_mode_from_code(code: u8) -> Result<ExecMode, SnapshotError> {
    match code {
        0 => Ok(ExecMode::Pause),
        1 => Ok(ExecMode::Run),
        2 => Ok(ExecMode::Step),
        3 => Ok(ExecMode::Next),
        4 => Ok(ExecMode::ToCall),
        5 => Ok(ExecMode::ToReturn),
        _ => Err(SnapshotError::InvalidState("execution mode")),
    }
}

fn rebase_timestamp(saved_now: u64, saved_timestamp: u64, now: u64) -> u64 {
    now.saturating_sub(saved_now.saturating_sub(saved_timestamp))
}

fn program_fingerprint(program: &[u16]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for word in program {
        for byte in word.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[derive(Default)]
struct SnapshotWriter {
    bytes: Vec<u8>,
}

impl SnapshotWriter {
    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn bool(&mut self, value: bool) {
        self.u8(value as u8);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }
}

struct SnapshotReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SnapshotReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn finish(&self) -> Result<(), SnapshotError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(SnapshotError::TrailingData)
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], SnapshotError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(SnapshotError::Truncated)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(SnapshotError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], SnapshotError> {
        self.take(N)?
            .try_into()
            .map_err(|_| SnapshotError::Truncated)
    }

    fn bool(&mut self) -> Result<bool, SnapshotError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(SnapshotError::InvalidState("boolean")),
        }
    }

    fn u8(&mut self) -> Result<u8, SnapshotError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, SnapshotError> {
        Ok(u16::from_le_bytes(self.array::<2>()?))
    }

    fn u64(&mut self) -> Result<u64, SnapshotError> {
        Ok(u64::from_le_bytes(self.array::<8>()?))
    }

    fn usize(&mut self, field: &'static str) -> Result<usize, SnapshotError> {
        usize::try_from(self.u64()?).map_err(|_| SnapshotError::InvalidState(field))
    }
}
