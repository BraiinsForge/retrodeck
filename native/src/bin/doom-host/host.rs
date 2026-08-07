//! DOOM host: the Rust platform layer for the statically linked fbDOOM
//! engine.
//!
//! The engine owns the main loop, so this file is inverted relative to the
//! libretro host: instead of driving a core, it exports the functions the
//! DOOM platform backends in `native/doom/` call, then hands control to
//! `doom_main` and never gets it back. Presentation, pacing, input mapping,
//! audio, and the clock all live here; the engine side stays free of Deck
//! specifics.

use std::ffi::{CString, c_char, c_int};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use retrodeck_native::doom_input::{Event, Mapper};

pub const NAME: &str = "doom-deck";

/// DOOM's fixed screen geometry. The host scaler turns this into 640x400
/// inside the Deck's safe area.
const SCREEN_WIDTH: u32 = 320;
const SCREEN_HEIGHT: u32 = 200;

/// DOOM's tic rate, from the engine's i_timer.h. Presenting faster than
/// this only burns battery: the engine cannot produce new game state
/// between tics.
const TICRATE: f64 = 35.0;

/// Sample rate for the mixer. DOOM's sound effects are 11025 Hz, which
/// divides into this exactly.
const SAMPLE_RATE: u32 = 44100;

/// A WAD smaller than this cannot hold a playable IWAD, and one larger is
/// not something the Deck should be asked to memory-map.
const MINIMUM_WAD_BYTES: u64 = 4 * 1024;
const MAXIMUM_WAD_BYTES: u64 = 64 * 1024 * 1024;

unsafe extern "C" {
    /// fbDOOM's `main`, renamed by the build so this binary can own the
    /// real entry point.
    fn doom_main(argc: c_int, argv: *const *const c_char) -> c_int;

    /// Effects the sound module has started, reported by the test summary.
    fn retrodeck_doom_sfx_started() -> c_int;

    /// OPL spans that carried signal, reported by the test summary.
    fn retrodeck_opl_voiced_spans() -> c_int;
    fn retrodeck_opl_register_writes() -> c_int;
    fn retrodeck_opl_callback_schedules() -> c_int;
}

struct State {
    clock: retrodeck_native::game_video::FrameClock,
    mapper: Mapper,
    pending: Vec<Event>,
    /// Start of the current 60-present diagnostics window.
    diagnostics_started: Instant,
    previous_audio_frames: u64,
    shed_frames: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Set by a signal, a failed present, or the test frame limit.
static QUIT: AtomicBool = AtomicBool::new(false);
static PRESENT_SKIP: AtomicBool = AtomicBool::new(false);
static PRESENTS: AtomicU64 = AtomicU64::new(0);
static AUDIO_FRAMES: AtomicU64 = AtomicU64::new(0);

/// Zero when running normally; otherwise the number of presents after which
/// the host reports its frame hash and exits.
static TEST_FRAMES: AtomicU64 = AtomicU64::new(0);

/// Test-mode clock, advanced by presents and sleeps instead of read from
/// the machine, so a recorded demo hashes the same on every run.
static SYNTHETIC_MS: AtomicI64 = AtomicI64::new(0);
static SLEEPS: AtomicU64 = AtomicU64::new(0);

static AUDIO_RATE: AtomicU32 = AtomicU32::new(0);
static DIAGNOSTICS: AtomicBool = AtomicBool::new(false);

static STARTED: Mutex<Option<Instant>> = Mutex::new(None);

fn test_mode() -> bool {
    TEST_FRAMES.load(Ordering::Relaxed) > 0
}

fn frame_milliseconds() -> i64 {
    (1000.0 / TICRATE) as i64
}

// ---------------------------------------------------------------------------
// Exports called by the DOOM platform backends.
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn retrodeck_doom_present(pixels: *const u32, width: c_int, height: c_int) -> c_int {
    if pixels.is_null() || width <= 0 || height <= 0 {
        return 0;
    }
    if QUIT.load(Ordering::Relaxed) {
        return 0;
    }

    let width = width as u32;
    let height = height as u32;
    let presents = PRESENTS.fetch_add(1, Ordering::Relaxed) + 1;

    if test_mode() {
        SYNTHETIC_MS.fetch_add(frame_milliseconds(), Ordering::Relaxed);
    }

    // A frame that is already half a tic late is shed rather than chased,
    // the same rule the libretro host uses to keep audio fed.
    let shed = PRESENT_SKIP.swap(false, Ordering::Relaxed);
    if !shed {
        let pitch = width as usize * 4;
        if let Err(error) =
            retrodeck_native::game_video::present(pixels.cast(), width, height, pitch, false)
        {
            eprintln!("{NAME}: {error}");
            QUIT.store(true, Ordering::Relaxed);
            return 1;
        }
    }

    let limit = TEST_FRAMES.load(Ordering::Relaxed);
    if limit > 0 && presents >= limit {
        println!(
            "{NAME}: test frames={presents} video={presents} audio={} sfx={} \
             music={}/{}/{} sleeps={} ms={} hash={:016x}",
            AUDIO_FRAMES.load(Ordering::Relaxed),
            unsafe { retrodeck_doom_sfx_started() },
            unsafe { retrodeck_opl_voiced_spans() },
            unsafe { retrodeck_opl_register_writes() },
            unsafe { retrodeck_opl_callback_schedules() },
            SLEEPS.load(Ordering::Relaxed),
            SYNTHETIC_MS.load(Ordering::Relaxed),
            retrodeck_native::game_video::frame_hash()
        );
        // doom_main never returns, so the test path leaves from here.
        shutdown();
        std::process::exit(0);
    }

    if !test_mode()
        && let Ok(mut guard) = STATE.lock()
        && let Some(state) = guard.as_mut()
    {
        if shed {
            state.shed_frames += 1;
        }

        // Reported before pacing, so the wall time reflects the work a
        // frame cost rather than how long the pacer then waited.
        if DIAGNOSTICS.load(Ordering::Relaxed) && presents % 60 == 0 {
            let elapsed = state.diagnostics_started.elapsed().as_secs_f64();
            let audio_frames = AUDIO_FRAMES.load(Ordering::Relaxed);
            println!(
                "{NAME}: diagnostics video=60 wall={elapsed:.3} fps={:.1} \
                 shed={} audio={} queued={} dropped={}",
                60.0 / elapsed,
                state.shed_frames,
                audio_frames - state.previous_audio_frames,
                retrodeck_native::game_audio::queued_frames(),
                retrodeck_native::game_audio::dropped_frames()
            );
            state.diagnostics_started = Instant::now();
            state.previous_audio_frames = audio_frames;
            state.shed_frames = 0;
        }

        state.clock.wait_for_next_frame();

        // Shedding a present only helps when presentation is what made the
        // frame late. When the cost is elsewhere - OPL music emulation, for
        // instance - lateness never recovers, so an unconditional rule sheds
        // almost every frame and the picture stops moving while the engine
        // happily runs at rate. Never shed twice in a row: worst case the
        // display halves, instead of collapsing to a few frames a second.
        let late = state.clock.lateness() > state.clock.frame_nanoseconds() / 2;
        PRESENT_SKIP.store(late && !shed, Ordering::Relaxed);
    }

    0
}

#[unsafe(no_mangle)]
pub extern "C" fn retrodeck_doom_poll_events(
    events: *mut Event,
    max_events: c_int,
) -> c_int {
    if events.is_null() || max_events <= 0 {
        return 0;
    }
    let Ok(mut guard) = STATE.lock() else {
        return 0;
    };
    let Some(state) = guard.as_mut() else {
        return 0;
    };

    state.mapper.poll(
        retrodeck_native::joypad::joypad_distinct_state,
        retrodeck_native::joypad::keyboard_key_held,
        &mut state.pending,
    );

    let count = state.pending.len().min(max_events as usize);
    let destination = unsafe { std::slice::from_raw_parts_mut(events, count) };
    destination.copy_from_slice(&state.pending[..count]);
    // Anything past the engine's buffer stays queued for the next tic
    // rather than being dropped, so a release always follows its press.
    state.pending.drain(..count);
    count as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn retrodeck_doom_quit_requested() -> c_int {
    c_int::from(
        QUIT.load(Ordering::Relaxed)
            || retrodeck_native::process::shutdown_requested()
            || retrodeck_native::game_video::exit_requested(),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn retrodeck_doom_ticks() -> c_int {
    let milliseconds = if test_mode() {
        SYNTHETIC_MS.load(Ordering::Relaxed)
    } else {
        STARTED
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .map_or(0, |started| started.elapsed().as_millis() as i64)
    };
    milliseconds.clamp(0, i64::from(c_int::MAX)) as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn retrodeck_doom_sleep(milliseconds: c_int) {
    if milliseconds <= 0 {
        return;
    }
    if test_mode() {
        // Keeping the synthetic clock moving is what stops the engine's
        // "wait for new tics" loop from spinning forever without a panel.
        SLEEPS.fetch_add(1, Ordering::Relaxed);
        SYNTHETIC_MS.fetch_add(i64::from(milliseconds), Ordering::Relaxed);
        return;
    }
    std::thread::sleep(std::time::Duration::from_millis(
        u64::try_from(milliseconds).unwrap_or(0).min(100),
    ));
}

#[unsafe(no_mangle)]
pub extern "C" fn retrodeck_doom_audio_write(frames: *const i16, frame_count: c_int) {
    if frames.is_null() || frame_count <= 0 || AUDIO_RATE.load(Ordering::Relaxed) == 0 {
        return;
    }
    if !test_mode() {
        let samples = unsafe { std::slice::from_raw_parts(frames, frame_count as usize * 2) };
        retrodeck_native::game_audio::write_stereo(samples);
    }
    AUDIO_FRAMES.fetch_add(frame_count as u64, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn retrodeck_doom_audio_rate() -> c_int {
    AUDIO_RATE.load(Ordering::Relaxed) as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn retrodeck_doom_audio_queued() -> c_int {
    if test_mode() {
        // No sound card in the sandbox, so report a permanently empty queue.
        // The mixer then runs a fixed batch every pass, which keeps the
        // reported frame count deterministic and still exercises the whole
        // path from DMX parsing through mixing.
        return 0;
    }
    c_int::try_from(retrodeck_native::game_audio::queued_frames()).unwrap_or(c_int::MAX)
}

#[unsafe(no_mangle)]
pub extern "C" fn retrodeck_doom_exit(status: c_int) -> ! {
    shutdown();
    std::process::exit(status);
}

#[unsafe(no_mangle)]
pub extern "C" fn retrodeck_doom_log(message: *const c_char) {
    if message.is_null() {
        return;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(message) };
    eprintln!("{NAME}: {}", text.to_string_lossy());
}

// ---------------------------------------------------------------------------
// Startup.
// ---------------------------------------------------------------------------

fn read_wad(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("cannot stat WAD {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("WAD is not a regular file: {}", path.display()));
    }
    if !(MINIMUM_WAD_BYTES..=MAXIMUM_WAD_BYTES).contains(&metadata.len()) {
        return Err(format!("WAD has an invalid size: {}", path.display()));
    }
    let mut header = [0_u8; 4];
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("cannot open WAD {}: {error}", path.display()))?;
    std::io::Read::read_exact(&mut file, &mut header)
        .map_err(|error| format!("cannot read WAD header {}: {error}", path.display()))?;
    if &header != b"IWAD" && &header != b"PWAD" {
        return Err(format!(
            "{} is not a DOOM WAD; it has no IWAD or PWAD header",
            path.display()
        ));
    }
    Ok(())
}

fn shutdown() {
    let _ = std::io::Write::flush(&mut std::io::stdout());
    retrodeck_native::joypad::shutdown();
    retrodeck_native::game_audio::close();
    retrodeck_native::game_video::close();
}

pub fn run_host(arguments: &[String]) -> u8 {
    if arguments.len() != 1 {
        eprintln!("Usage: {NAME} IWAD.wad");
        return 2;
    }
    let wad = &arguments[0];

    if let Err(error) = retrodeck_native::process::install_signal_handlers() {
        eprintln!("{NAME}: {error}");
        return 1;
    }
    if let Err(error) = read_wad(Path::new(wad)) {
        eprintln!("{NAME}: {error}");
        return 1;
    }

    DIAGNOSTICS.store(
        std::env::var_os("RETRO_DECK_RUNTIME_DIAGNOSTICS").is_some(),
        Ordering::Relaxed,
    );

    let test_frames = std::env::var("RETRO_DECK_TEST_FRAMES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|frames| *frames > 0);
    if let Some(frames) = test_frames {
        TEST_FRAMES.store(frames, Ordering::Relaxed);
    }

    if let Err(error) = retrodeck_native::joypad::initialize(false) {
        eprintln!("{NAME}: {error}");
        eprintln!("{NAME}: continuing without controller input");
    }

    if let Err(error) = retrodeck_native::game_video::open(test_frames.is_some()) {
        eprintln!("{NAME}: {error}");
        return 1;
    }

    let volume = retrodeck_native::game_audio::volume_percent().unwrap_or_else(|error| {
        eprintln!("{NAME}: {error}");
        0
    });
    if test_frames.is_none() {
        match retrodeck_native::game_audio::open(SAMPLE_RATE, volume) {
            Ok(()) => AUDIO_RATE.store(SAMPLE_RATE, Ordering::Relaxed),
            Err(error) => eprintln!("{NAME}: sound disabled: {error}"),
        }
    } else {
        // Mix without a sound card so the test still covers the mixer.
        AUDIO_RATE.store(SAMPLE_RATE, Ordering::Relaxed);
    }

    *STARTED.lock().expect("clock lock") = Some(Instant::now());
    *STATE.lock().expect("state lock") = Some(State {
        clock: retrodeck_native::game_video::FrameClock::new(TICRATE),
        mapper: Mapper::new(),
        pending: Vec::new(),
        diagnostics_started: Instant::now(),
        previous_audio_frames: 0,
        shed_frames: 0,
    });

    // Savegames go where the dashboard says, which on a Deck is outside the
    // installed games directory: activation replaces that directory
    // wholesale and would delete a player's saves with it. The fallback
    // keeps a bare invocation, including the frame-hash test,
    // self-contained beside its WAD. The engine derives its savegame
    // directory from this one; its configuration writer is compiled out
    // upstream, so no config file is produced either way.
    let directory = std::env::var("RETRO_DECK_DOOM_STATE")
        .ok()
        .filter(|value| value.starts_with('/'))
        .unwrap_or_else(|| {
            Path::new(wad)
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map_or_else(|| ".".to_owned(), |parent| parent.display().to_string())
        });
    if let Err(error) = std::fs::create_dir_all(&directory) {
        eprintln!("{NAME}: cannot create the save directory {directory}: {error}");
    }
    // SAFETY: single-threaded startup, before the engine or its worker
    // threads exist.
    unsafe { std::env::set_var("RETRO_DECK_DOOM_DIR", &directory) };

    println!(
        "{NAME}: fbDOOM {SCREEN_WIDTH}x{SCREEN_HEIGHT}, {TICRATE:.0} fps, \
         {SAMPLE_RATE} Hz, volume {volume}%"
    );

    let program = CString::new(NAME).expect("program name");
    let iwad_flag = CString::new("-iwad").expect("iwad flag");
    let iwad_path = match CString::new(wad.as_str()) {
        Ok(path) => path,
        Err(_) => {
            eprintln!("{NAME}: WAD path contains a NUL byte");
            shutdown();
            return 1;
        }
    };
    let argv = [
        program.as_ptr(),
        iwad_flag.as_ptr(),
        iwad_path.as_ptr(),
        std::ptr::null(),
    ];

    // The engine runs its own loop and normally leaves through I_Quit,
    // which exits the process. Reaching the next line means it returned.
    let status = unsafe { doom_main(3, argv.as_ptr()) };
    shutdown();
    u8::try_from(status).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_milliseconds_matches_the_tic_rate() {
        assert_eq!(frame_milliseconds(), 28);
    }

    #[test]
    fn rejects_files_that_are_not_wads() {
        let directory = std::env::temp_dir().join(format!("doom-host-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("not-a-wad.wad");
        std::fs::write(&path, vec![b'X'; 8192]).expect("write");
        let error = read_wad(&path).expect_err("a headerless file must be rejected");
        assert!(error.contains("not a DOOM WAD"), "{error}");

        let short = directory.join("short.wad");
        std::fs::write(&short, b"IWAD").expect("write");
        assert!(read_wad(&short).is_err(), "a tiny file must be rejected");

        let good = directory.join("good.wad");
        let mut contents = b"IWAD".to_vec();
        contents.resize(8192, 0);
        std::fs::write(&good, contents).expect("write");
        assert!(read_wad(&good).is_ok());

        std::fs::remove_dir_all(&directory).ok();
    }
}
