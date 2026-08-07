pub const TICK_FREQUENCY: u64 = 32768; // Hz

// Oscillator frequencies
pub const OSC1_FREQUENCY: u64 = TICK_FREQUENCY; // Hz
pub const OSC3_FREQUENCY: u64 = 1000000; // Hz

pub const TIMER_PERIODS: [u64; 8] = [
    TICK_FREQUENCY / 2,
    TICK_FREQUENCY / 4,
    TICK_FREQUENCY / 8,
    TICK_FREQUENCY / 16,
    TICK_FREQUENCY / 32,
    TICK_FREQUENCY / 64,
    TICK_FREQUENCY / 128,
    TICK_FREQUENCY / 256,
];

/// Monotonic timestamp source in microseconds.
pub trait Clock {
    fn now(&self) -> u64;
}

#[repr(usize)]
pub enum TimerType {
    Timer2Hz = 0,
    Timer4Hz = 1,
    Timer8Hz = 2,
    Timer16Hz = 3,
    Timer32Hz = 4,
    Timer64Hz = 5,
    Timer128Hz = 6,
    Timer256Hz = 7,
}

/// Clock timer
pub struct Timer {
    // Timestamp in emulated ticks
    pub ts: u64,
    pub period: u64,
}

impl Timer {
    pub fn new(period: u64) -> Self {
        Self { ts: 0, period }
    }
}

/// Programmable timer
pub struct ProgTimer {
    pub enabled: bool,
    // Timestamp in emulated ticks
    pub ts: u64,
    pub reload: u8,
    pub data: u8,
}

impl ProgTimer {
    pub const PERIOD: u64 = TICK_FREQUENCY / 256;

    pub fn new() -> Self {
        Self {
            enabled: false,
            ts: 0,
            reload: 0,
            data: 0,
        }
    }
}

pub struct CpuClock {
    pub timers: [Timer; 8],
    pub prog_timer: ProgTimer,

    pub tick_counter: u64,
    /// Host timestamp units per second.
    pub ts_freq: u64,
    pub speed_ratio: u8,
    /// Host monotonic timestamp in microseconds.
    pub ref_ts: u64,

    pub cpu_halted: bool,
    pub cpu_freq: u64, // hz
    pub scaled_cycle_accumulator: u64,

    pub system_clock: Box<dyn Clock>,

    pub previous_cycles: u8,
}

impl CpuClock {
    pub fn new(cpu_freq: u64, ts_freq: u64, system_clock: Box<dyn Clock>) -> Self {
        let now = system_clock.now();
        Self {
            timers: TIMER_PERIODS.map(Timer::new),
            prog_timer: ProgTimer::new(),
            tick_counter: 0,
            ts_freq,
            speed_ratio: 1,
            ref_ts: now,
            cpu_halted: false,
            cpu_freq,
            scaled_cycle_accumulator: 0,
            system_clock,
            previous_cycles: 0,
        }
    }

    pub fn wait_timer(&mut self, timer_type: TimerType) -> bool {
        let timer = &mut self.timers[timer_type as usize];

        if self.tick_counter.wrapping_sub(timer.ts) >= timer.period {
            loop {
                timer.ts = timer.ts.wrapping_add(timer.period);
                if self.tick_counter.wrapping_sub(timer.ts) < timer.period {
                    break;
                }
            }

            return true;
        }

        false
    }
}
