//! Libretro host: embeds one statically linked core per console binary.

use retrodeck_native::{joypad, snes_rom};
use std::ffi::{CStr, c_char, c_uint, c_void};
use std::os::raw::c_double;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

pub const API_VERSION: c_uint = 1;

const ENV_SET_PIXEL_FORMAT: c_uint = 10;
const ENV_GET_SYSTEM_DIRECTORY: c_uint = 9;
const ENV_GET_VARIABLE: c_uint = 15;
const ENV_SET_VARIABLES: c_uint = 16;
const ENV_GET_VARIABLE_UPDATE: c_uint = 17;
const ENV_GET_LOG_INTERFACE: c_uint = 27;
const ENV_GET_SAVE_DIRECTORY: c_uint = 31;
const ENV_SET_CONTROLLER_INFO: c_uint = 35;
const ENV_SET_MEMORY_MAPS: c_uint = 36;
const ENV_SET_GEOMETRY: c_uint = 37;
const ENV_GET_LANGUAGE: c_uint = 39;
const ENV_SET_SUPPORT_ACHIEVEMENTS: c_uint = 42;
const ENV_GET_INPUT_BITMASKS: c_uint = 51;
const ENV_GET_CORE_OPTIONS_VERSION: c_uint = 52;
const ENV_SET_CORE_OPTIONS: c_uint = 53;
const ENV_SET_CORE_OPTIONS_INTL: c_uint = 54;
const ENV_SET_CORE_OPTIONS_DISPLAY: c_uint = 55;
const ENV_SET_CORE_OPTIONS_V2: c_uint = 67;
const ENV_SET_CORE_OPTIONS_V2_INTL: c_uint = 68;
const ENV_GET_CAN_DUPE: c_uint = 3;
const ENV_GET_AUDIO_VIDEO_ENABLE: c_uint = 47 | 0x10000;
const ENV_SET_PERFORMANCE_LEVEL: c_uint = 8;
const ENV_SET_INPUT_DESCRIPTORS: c_uint = 11;
const ENV_SET_SUBSYSTEM_INFO: c_uint = 34;
const ENV_GET_RUMBLE_INTERFACE: c_uint = 23;
const ENV_GET_CONTENT_DIRECTORY: c_uint = 30;

const PIXEL_FORMAT_XRGB8888: c_uint = 1;
const PIXEL_FORMAT_RGB565: c_uint = 2;

const DEVICE_JOYPAD: c_uint = 1;
const DEVICE_KEYBOARD: c_uint = 3;
const DEVICE_ID_JOYPAD_MASK: c_uint = 256;
const DEVICE_ID_JOYPAD_R3: c_uint = 15;

const MEMORY_SAVE_RAM: c_uint = 0;
const MEMORY_RTC: c_uint = 1;

#[repr(C)]
struct SystemInfo {
    library_name: *const c_char,
    library_version: *const c_char,
    valid_extensions: *const c_char,
    need_fullpath: bool,
    block_extract: bool,
}

#[repr(C)]
struct GameGeometry {
    base_width: c_uint,
    base_height: c_uint,
    max_width: c_uint,
    max_height: c_uint,
    aspect_ratio: f32,
}

#[repr(C)]
struct SystemTiming {
    fps: c_double,
    sample_rate: c_double,
}

#[repr(C)]
struct SystemAvInfo {
    geometry: GameGeometry,
    timing: SystemTiming,
}

#[repr(C)]
struct GameInfo {
    path: *const c_char,
    data: *const c_void,
    size: usize,
    meta: *const c_char,
}

#[repr(C)]
struct Variable {
    key: *const c_char,
    value: *const c_char,
}

unsafe extern "C" {
    fn retro_set_environment(callback: unsafe extern "C" fn(c_uint, *mut c_void) -> bool);
    fn retro_set_video_refresh(
        callback: unsafe extern "C" fn(*const c_void, c_uint, c_uint, usize),
    );
    fn retro_set_audio_sample(callback: *const c_void);
    fn retro_set_audio_sample_batch(callback: unsafe extern "C" fn(*const i16, usize) -> usize);
    fn retro_set_input_poll(callback: unsafe extern "C" fn());
    fn retro_set_input_state(callback: unsafe extern "C" fn(c_uint, c_uint, c_uint, c_uint) -> i16);
    fn retro_set_controller_port_device(port: c_uint, device: c_uint);
    fn retro_init();
    fn retro_deinit();
    fn retro_api_version() -> c_uint;
    fn retro_get_system_info(info: *mut SystemInfo);
    fn retro_get_system_av_info(info: *mut SystemAvInfo);
    fn retro_load_game(game: *const GameInfo) -> bool;
    fn retro_unload_game();
    fn retro_run();
    fn retro_get_memory_data(id: c_uint) -> *mut c_void;
    fn retro_get_memory_size(id: c_uint) -> usize;
}

pub struct CoreConfig {
    pub frontend_name: &'static str,
    pub default_core_name: &'static str,
    pub rom_description: &'static str,
    pub rom_usage: &'static str,
    pub save_extension: &'static str,
    pub minimum_rom_bytes: usize,
    pub maximum_rom_bytes: usize,
    pub player_count: u32,
    pub has_rtc: bool,
    pub zx: bool,
    pub nes: bool,
    pub gba: bool,
    pub snes: bool,
}

pub const NES_CONFIG: CoreConfig = CoreConfig {
    frontend_name: "nes-deck",
    default_core_name: "FCEUmm",
    rom_description: "NES",
    rom_usage: "ROM.nes",
    save_extension: ".srm",
    minimum_rom_bytes: 16,
    maximum_rom_bytes: 8 * 1024 * 1024,
    player_count: 2,
    has_rtc: false,
    zx: false,
    nes: true,
    gba: false,
    snes: false,
};

pub const GB_CONFIG: CoreConfig = CoreConfig {
    frontend_name: "gb-deck",
    default_core_name: "Gambatte",
    rom_description: "Game Boy",
    rom_usage: "ROM.gb|ROM.gbc",
    save_extension: ".sav",
    minimum_rom_bytes: 0x150,
    maximum_rom_bytes: 8 * 1024 * 1024,
    player_count: 1,
    has_rtc: true,
    zx: false,
    nes: false,
    gba: false,
    snes: false,
};

pub const ZX_CONFIG: CoreConfig = CoreConfig {
    frontend_name: "zx-deck",
    default_core_name: "Fuse",
    rom_description: "ZX Spectrum",
    rom_usage: "ROM.tap",
    save_extension: ".sav",
    minimum_rom_bytes: 4,
    maximum_rom_bytes: 8 * 1024 * 1024,
    player_count: 2,
    has_rtc: false,
    zx: true,
    nes: false,
    gba: false,
    snes: false,
};

pub const GBA_CONFIG: CoreConfig = CoreConfig {
    frontend_name: "gba-deck",
    default_core_name: "gpSP",
    rom_description: "Game Boy Advance",
    rom_usage: "ROM.gba",
    save_extension: ".sav",
    minimum_rom_bytes: 192,
    maximum_rom_bytes: 32 * 1024 * 1024,
    player_count: 1,
    has_rtc: true,
    zx: false,
    nes: false,
    gba: true,
    snes: false,
};

pub const SNES_CONFIG: CoreConfig = CoreConfig {
    frontend_name: "snes-deck",
    default_core_name: "Supafaust",
    rom_description: "SNES",
    rom_usage: "ROM.sfc|ROM.smc",
    save_extension: ".srm",
    minimum_rom_bytes: snes_rom::MINIMUM_BYTES,
    maximum_rom_bytes: snes_rom::MAXIMUM_BYTES,
    player_count: 2,
    has_rtc: false,
    zx: false,
    nes: false,
    gba: false,
    snes: true,
};
static CONFIG: std::sync::OnceLock<&'static CoreConfig> = std::sync::OnceLock::new();
static SYSTEM_DIRECTORY: std::sync::OnceLock<std::ffi::CString> = std::sync::OnceLock::new();
static PIXEL_FORMAT: AtomicU32 = AtomicU32::new(PIXEL_FORMAT_XRGB8888);
static VIDEO_FAILED: AtomicBool = AtomicBool::new(false);
static PRESENT_SKIP: AtomicBool = AtomicBool::new(false);
static SNES_VIDEO_ENABLED: AtomicBool = AtomicBool::new(true);
static VIDEO_DIVISOR: AtomicU32 = AtomicU32::new(1);
static VIDEO_CALLBACKS: AtomicU64 = AtomicU64::new(0);
static AUDIO_FRAMES: AtomicU64 = AtomicU64::new(0);
static AUDIO_CALLBACKS: AtomicU64 = AtomicU64::new(0);

fn config() -> &'static CoreConfig {
    CONFIG.get().expect("libretro host configuration is unset")
}

fn nes_variable(key: &CStr) -> Option<&'static CStr> {
    match key.to_bytes() {
        b"fceumm_region" => Some(c"Auto"),
        b"fceumm_overscan_h_left" | b"fceumm_overscan_h_right" => Some(c"0"),
        b"fceumm_overscan_v_top" | b"fceumm_overscan_v_bottom" => Some(c"8"),
        _ => None,
    }
}

fn gba_variable(key: &CStr) -> Option<&'static CStr> {
    match key.to_bytes() {
        // The official BIOS is picked up from the ROM directory when the
        // owner has dropped it there; gpSP falls back to its HLE BIOS.
        b"gpsp_bios" => Some(c"auto"),
        b"gpsp_drc" => Some(c"enabled"),
        // 32768 Hz rides the same verified 32 kHz OSS path as Gambatte;
        // the 65536 Hz default underruns the Deck's audio device.
        b"gpsp_sound_rate" => Some(c"32768"),
        b"gpsp_frameskip" => Some(c"disabled"),
        _ => None,
    }
}

fn zx_variable(key: &CStr) -> Option<&'static CStr> {
    match key.to_bytes() {
        b"fuse_machine" => Some(c"Spectrum 48K"),
        b"fuse_emulation_speed" => Some(c"100"),
        b"fuse_size_border" => Some(c"medium"),
        b"fuse_palette" => Some(c"Fuse Standard"),
        b"fuse_auto_load" | b"fuse_fast_load" => Some(c"enabled"),
        b"fuse_load_sound" | b"fuse_display_joystick_type" => Some(c"disabled"),
        b"fuse_speaker_type" => Some(c"tv speaker"),
        b"fuse_ay_stereo_separation" => Some(c"none"),
        b"fuse_key_ovrlay_transp" => Some(c"enabled"),
        b"fuse_key_hold_time" => Some(c"500"),
        b"fuse_joypad_start" => Some(c"Enter"),
        key if key.starts_with(b"fuse_joypad_") => Some(c"<none>"),
        _ => None,
    }
}

unsafe extern "C" fn environment_callback(command: c_uint, data: *mut c_void) -> bool {
    match command {
        ENV_GET_CORE_OPTIONS_VERSION => unsafe {
            data.cast::<c_uint>()
                .as_mut()
                .map(|value| *value = 2)
                .is_some()
        },
        ENV_GET_LANGUAGE => unsafe {
            data.cast::<c_uint>()
                .as_mut()
                .map(|value| *value = 0)
                .is_some()
        },
        ENV_GET_SYSTEM_DIRECTORY | ENV_GET_SAVE_DIRECTORY | ENV_GET_CONTENT_DIRECTORY => unsafe {
            data.cast::<*const c_char>()
                .as_mut()
                .map(|value| {
                    *value = SYSTEM_DIRECTORY
                        .get()
                        .map_or(std::ptr::null(), |directory| directory.as_ptr());
                })
                .is_some()
        },
        // Stable Rust cannot define the variadic log target; cores fall back
        // to their own stderr logging.
        ENV_GET_LOG_INTERFACE => false,
        ENV_GET_INPUT_BITMASKS => true,
        ENV_GET_AUDIO_VIDEO_ENABLE if config().snes => unsafe {
            data.cast::<i32>()
                .as_mut()
                .map(|value| {
                    let video = SNES_VIDEO_ENABLED.load(Ordering::Relaxed);
                    *value = 0b10 | i32::from(video); // Audio runs on every frame.
                })
                .is_some()
        },
        ENV_GET_CAN_DUPE => unsafe {
            data.cast::<bool>()
                .as_mut()
                .map(|value| *value = true)
                .is_some()
        },
        ENV_GET_VARIABLE_UPDATE => unsafe {
            data.cast::<bool>()
                .as_mut()
                .map(|value| *value = false)
                .is_some()
        },
        ENV_GET_VARIABLE => {
            let Some(variable) = (unsafe { data.cast::<Variable>().as_mut() }) else {
                return false;
            };
            if variable.key.is_null() {
                return false;
            }
            let key = unsafe { CStr::from_ptr(variable.key) };
            let value = if config().nes {
                nes_variable(key)
            } else if config().zx {
                zx_variable(key)
            } else if config().gba {
                gba_variable(key)
            } else {
                None
            };
            match value {
                Some(value) => {
                    variable.value = value.as_ptr();
                    true
                }
                None => {
                    variable.value = std::ptr::null();
                    false
                }
            }
        }
        ENV_GET_RUMBLE_INTERFACE => false,
        ENV_SET_PIXEL_FORMAT => {
            let Some(format) = (unsafe { data.cast::<c_uint>().as_ref() }) else {
                return false;
            };
            if *format != PIXEL_FORMAT_XRGB8888 && *format != PIXEL_FORMAT_RGB565 {
                return false;
            }
            PIXEL_FORMAT.store(*format, Ordering::Relaxed);
            true
        }
        ENV_SET_CORE_OPTIONS
        | ENV_SET_CORE_OPTIONS_INTL
        | ENV_SET_CORE_OPTIONS_V2
        | ENV_SET_CORE_OPTIONS_V2_INTL
        | ENV_SET_VARIABLES
        | ENV_SET_CORE_OPTIONS_DISPLAY
        | ENV_SET_CONTROLLER_INFO
        | ENV_SET_INPUT_DESCRIPTORS
        | ENV_SET_MEMORY_MAPS
        | ENV_SET_SUBSYSTEM_INFO
        | ENV_SET_SUPPORT_ACHIEVEMENTS
        | ENV_SET_GEOMETRY
        | ENV_SET_PERFORMANCE_LEVEL => true,
        _ => false,
    }
}

thread_local! {
    static SNES_FRAME: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn snes_video_steps(width: u32, height: u32) -> (usize, usize) {
    // Supafaust doubles the native width and/or line count in hi-res and
    // interlaced modes. Return to the base grid for both Deck renderers.
    (
        if width == 512 { 2 } else { 1 },
        if height >= 448 { 2 } else { 1 },
    )
}

unsafe fn present_snes(
    data: *const c_void,
    width: u32,
    height: u32,
    pitch: usize,
    rgb565: bool,
) -> Result<(), String> {
    if width == 0 || width > 512 || height == 0 || height > 480 {
        return Err("invalid SNES frame dimensions".to_owned());
    }
    let (step_x, step_y) = snes_video_steps(width, height);
    let output_height = height / step_y as u32;
    let output_pitch = pitch
        .checked_mul(step_y)
        .ok_or("SNES frame pitch overflow")?;
    if step_x == 1 {
        return unsafe {
            retrodeck_native::game_video::present(data, width, output_height, output_pitch, rgb565)
        };
    }

    let bytes_per_pixel = if rgb565 { 2 } else { 4 };
    let row_bytes = width as usize * bytes_per_pixel;
    if pitch < row_bytes {
        return Err("invalid SNES frame geometry".to_owned());
    }
    let input_bytes = (height as usize - 1)
        .checked_mul(pitch)
        .and_then(|offset| offset.checked_add(row_bytes))
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or("SNES frame dimensions overflow")?;
    // libretro provides the complete frame through the callback's return.
    let input = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), input_bytes) };
    let stride = row_bytes / 2;
    SNES_FRAME.with_borrow_mut(|frame| {
        frame.resize(stride * output_height as usize, 0);
        for (y, row) in frame.chunks_exact_mut(stride).enumerate() {
            let start = y * output_pitch;
            blend_snes_row(&input[start..start + row_bytes], row, rgb565);
        }
        unsafe {
            retrodeck_native::game_video::present(
                frame.as_ptr().cast(),
                width / 2,
                output_height,
                stride,
                rgb565,
            )
        }
    })
}

fn blend_snes_row(input: &[u8], output: &mut [u8], rgb565: bool) {
    if rgb565 {
        for (pair, pixel) in input.chunks_exact(4).zip(output.chunks_exact_mut(2)) {
            let a = u16::from_ne_bytes([pair[0], pair[1]]);
            let b = u16::from_ne_bytes([pair[2], pair[3]]);
            // Average RGB565 components without carrying between them.
            let average = (a & b) + (((a ^ b) & 0xf7de) >> 1);
            pixel.copy_from_slice(&average.to_ne_bytes());
        }
    } else {
        for (pair, pixel) in input.chunks_exact(8).zip(output.chunks_exact_mut(4)) {
            for channel in 0..4 {
                pixel[channel] = pair[channel].midpoint(pair[channel + 4]);
            }
        }
    }
}

unsafe extern "C" fn video_callback(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    if data.is_null() || VIDEO_FAILED.load(Ordering::Relaxed) {
        return;
    }
    let calls = VIDEO_CALLBACKS.fetch_add(1, Ordering::Relaxed) + 1;
    let divisor = u64::from(VIDEO_DIVISOR.load(Ordering::Relaxed));
    if !config().snes && divisor > 1 && calls % divisor != 0 {
        return;
    }
    // The pacing loop marks frames that are already late; shedding their
    // present keeps the audio fed instead of chasing a frame the panel
    // would drop anyway.
    if PRESENT_SKIP.swap(false, Ordering::Relaxed) {
        return;
    }
    let rgb565 = PIXEL_FORMAT.load(Ordering::Relaxed) == PIXEL_FORMAT_RGB565;
    // libretro keeps the callback's frame buffer readable until we return.
    let result = if config().snes {
        unsafe { present_snes(data, width, height, pitch, rgb565) }
    } else {
        unsafe { retrodeck_native::game_video::present(data, width, height, pitch, rgb565) }
    };
    if let Err(error) = result {
        eprintln!("{}: video error: {error}", config().frontend_name);
        VIDEO_FAILED.store(true, Ordering::Relaxed);
    }
}

unsafe extern "C" fn audio_callback(data: *const i16, frames: usize) -> usize {
    AUDIO_FRAMES.fetch_add(frames as u64, Ordering::Relaxed);
    AUDIO_CALLBACKS.fetch_add(1, Ordering::Relaxed);
    if !data.is_null() && frames > 0 {
        let samples = unsafe { std::slice::from_raw_parts(data, frames * 2) };
        retrodeck_native::game_audio::write_stereo(samples);
    }
    frames
}

fn input_mask(state: u32, mapping: &[(u32, u32)]) -> u16 {
    mapping.iter().fold(0, |mask, &(pad_bit, id)| {
        if state & pad_bit == 0 {
            mask
        } else {
            mask | (1 << id)
        }
    })
}

fn snes_input_mask(gamepad: u32, keyboard: u32) -> u16 {
    input_mask(gamepad | keyboard, &joypad::RETRO_BUTTON_MAP)
        | input_mask(
            gamepad,
            &[
                (joypad::PAD_Y, 1),
                (joypad::PAD_X, 9),
                (joypad::PAD_L, 10),
                (joypad::PAD_R, 11),
            ],
        )
}

unsafe extern "C" fn input_poll_callback() {}

unsafe extern "C" fn input_state_callback(
    port: c_uint,
    device: c_uint,
    index: c_uint,
    id: c_uint,
) -> i16 {
    let configuration = config();
    if configuration.zx && device == DEVICE_KEYBOARD && index == 0 {
        let keycode = retrodeck_native::joypad::zx_linux_keycode(id);
        return i16::from(keycode != 0 && retrodeck_native::joypad::keyboard_key_held(keycode));
    }
    if port >= configuration.player_count || device != DEVICE_JOYPAD || index != 0 {
        return 0;
    }
    let result = if configuration.snes {
        snes_input_mask(
            joypad::joypad_distinct_state(port),
            joypad::keyboard_pad_state(port),
        )
    } else {
        input_mask(joypad::joypad_state(port), &joypad::RETRO_BUTTON_MAP)
    };
    if id == DEVICE_ID_JOYPAD_MASK {
        return result as i16;
    }
    if id > DEVICE_ID_JOYPAD_R3 {
        return 0;
    }
    i16::from(result & (1 << id) != 0)
}

fn read_rom(path: &Path, configuration: &CoreConfig) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("cannot stat ROM {}: {error}", path.display()))?;
    let size = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if !metadata.is_file()
        || size < configuration.minimum_rom_bytes
        || size > configuration.maximum_rom_bytes
        || (configuration.snes && !snes_rom::valid_size(size))
    {
        return Err(format!(
            "{} ROM has an invalid size",
            configuration.rom_description
        ));
    }
    let rom = std::fs::read(path).map_err(|error| {
        format!(
            "cannot read complete {} ROM {}: {error}",
            configuration.rom_description,
            path.display()
        )
    })?;
    if configuration.snes && !snes_rom::valid_size(rom.len()) {
        return Err("SNES ROM has an invalid size".to_owned());
    }
    if configuration.nes && (rom.len() < 4 || &rom[0..4] != b"NES\x1a") {
        return Err("NES ROM is missing its iNES header".to_owned());
    }
    Ok(rom)
}

fn save_base(path: &str) -> String {
    let separator = path.rfind('/');
    match path.rfind('.') {
        Some(dot) if separator.is_none_or(|slash| dot > slash) => path[..dot].to_owned(),
        _ => path.to_owned(),
    }
}

fn save_memory_file(path: &str, data: *const c_void, size: usize) -> std::io::Result<()> {
    if data.is_null() || size == 0 {
        return Ok(());
    }
    let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) };
    let temporary = format!("{path}.tmp.{}", std::process::id());
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        std::io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

// False protects an existing save that could not be loaded from later writes.
fn load_memory_file(
    path: &str,
    data: *mut c_void,
    size: usize,
    configuration: &CoreConfig,
) -> bool {
    if data.is_null() || size == 0 {
        return false;
    }
    let name = configuration.frontend_name;
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
        Err(error) => {
            eprintln!("{name}: cannot stat save {path}: {error}");
            return false;
        }
    };
    if !metadata.is_file() {
        eprintln!("{name}: save is not a regular file: {path}");
        return false;
    }
    let destination = unsafe { std::slice::from_raw_parts_mut(data.cast::<u8>(), size) };
    if metadata.len() != size as u64 {
        if configuration.nes
            && metadata.len() > 0
            && metadata.len() <= retrodeck_native::nes_sram::maximum_encoded_size(size) as u64
            && let Ok(encoded) = std::fs::read(path)
            && retrodeck_native::nes_sram::decode(&encoded, destination)
        {
            eprintln!("{name}: migrated encoded InfoNES save: {path}");
            return true;
        }
        eprintln!("{name}: protecting save with unexpected size: {path}");
        return false;
    }
    match std::fs::read(path) {
        Ok(bytes) if bytes.len() == size => {
            destination.copy_from_slice(&bytes);
            true
        }
        Ok(_) => {
            eprintln!("{name}: incomplete save read: {path}");
            false
        }
        Err(error) => {
            eprintln!("{name}: cannot read save {path}: {error}");
            false
        }
    }
}

struct PersistentMemory {
    save_ram_writable: bool,
    rtc_writable: bool,
}

fn load_persistent_memory(base: &str, configuration: &CoreConfig) -> PersistentMemory {
    unsafe {
        PersistentMemory {
            save_ram_writable: load_memory_file(
                &format!("{base}{}", configuration.save_extension),
                retro_get_memory_data(MEMORY_SAVE_RAM),
                retro_get_memory_size(MEMORY_SAVE_RAM),
                configuration,
            ),
            rtc_writable: configuration.has_rtc
                && load_memory_file(
                    &format!("{base}.rtc"),
                    retro_get_memory_data(MEMORY_RTC),
                    retro_get_memory_size(MEMORY_RTC),
                    configuration,
                ),
        }
    }
}

fn save_persistent_memory(
    base: &str,
    configuration: &CoreConfig,
    memory: &PersistentMemory,
) -> std::io::Result<()> {
    unsafe {
        if memory.save_ram_writable {
            save_memory_file(
                &format!("{base}{}", configuration.save_extension),
                retro_get_memory_data(MEMORY_SAVE_RAM),
                retro_get_memory_size(MEMORY_SAVE_RAM),
            )?;
        }
        if memory.rtc_writable {
            save_memory_file(
                &format!("{base}.rtc"),
                retro_get_memory_data(MEMORY_RTC),
                retro_get_memory_size(MEMORY_RTC),
            )?;
        }
    }
    Ok(())
}

use std::os::unix::fs::OpenOptionsExt;

pub fn run_host(configuration: &'static CoreConfig, arguments: &[String]) -> u8 {
    let name = configuration.frontend_name;
    if arguments.len() != 1 {
        eprintln!("Usage: {name} {}", configuration.rom_usage);
        return 2;
    }
    CONFIG.set(configuration).ok();
    if let Err(error) = retrodeck_native::process::install_signal_handlers() {
        eprintln!("{name}: {error}");
        return 1;
    }

    let rom_path = &arguments[0];
    let rom = match read_rom(Path::new(rom_path), configuration) {
        Ok(rom) => rom,
        Err(error) => {
            eprintln!("{name}: {error}");
            return 1;
        }
    };

    let parent = Path::new(rom_path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let directory =
        std::ffi::CString::new(parent.to_string_lossy().into_owned()).unwrap_or_default();
    SYSTEM_DIRECTORY.set(directory).ok();
    if configuration.gba {
        let bios = if parent.join("gba_bios.bin").is_file() {
            "official BIOS found beside the ROM"
        } else {
            "no gba_bios.bin beside the ROM; using the built-in HLE BIOS"
        };
        eprintln!("{name}: {bios}");
    }

    if let Ok(divisor) = std::env::var("RETRO_DECK_VIDEO_DIVISOR")
        && std::env::var_os("RETRO_DECK_RUNTIME_DIAGNOSTICS").is_some()
        && let Ok(parsed) = divisor.parse::<u32>()
        && (1..=60).contains(&parsed)
    {
        VIDEO_DIVISOR.store(parsed, Ordering::Relaxed);
    }

    unsafe {
        retro_set_environment(environment_callback);
        retro_set_video_refresh(video_callback);
        retro_set_audio_sample(std::ptr::null());
        retro_set_audio_sample_batch(audio_callback);
        retro_set_input_poll(input_poll_callback);
        retro_set_input_state(input_state_callback);
        retro_init();
    }
    if configuration.zx {
        // Kempston for Player 1 and Sinclair 2 for Player 2, as before.
        unsafe {
            retro_set_controller_port_device(0, (1 << 8) | DEVICE_JOYPAD);
            retro_set_controller_port_device(1, (3 << 8) | DEVICE_JOYPAD);
        }
    }

    let mut system_info = unsafe { std::mem::zeroed::<SystemInfo>() };
    unsafe { retro_get_system_info(&mut system_info) };
    if unsafe { retro_api_version() } != API_VERSION {
        eprintln!("{name}: incompatible libretro API");
        unsafe { retro_deinit() };
        return 1;
    }

    let path = std::ffi::CString::new(rom_path.as_str()).unwrap_or_default();
    let game = GameInfo {
        path: path.as_ptr(),
        data: rom.as_ptr().cast(),
        size: rom.len(),
        meta: std::ptr::null(),
    };
    if !unsafe { retro_load_game(&game) } {
        eprintln!(
            "{name}: {} core rejected the ROM",
            configuration.rom_description
        );
        unsafe { retro_deinit() };
        return 1;
    }

    let _input_session = match retrodeck_native::joypad::Session::start(configuration.zx) {
        Ok(session) => Some(session),
        Err(error) => {
            eprintln!("{name}: {error}");
            eprintln!("{name}: continuing without controller input");
            None
        }
    };

    let test_frames = std::env::var("RETRO_DECK_TEST_FRAMES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    if let Err(error) = retrodeck_native::game_video::open(test_frames.is_some()) {
        eprintln!("{name}: {error}");
        unsafe {
            retro_unload_game();
            retro_deinit();
        }
        return 1;
    }

    let mut av_info = unsafe { std::mem::zeroed::<SystemAvInfo>() };
    unsafe { retro_get_system_av_info(&mut av_info) };
    let sample_rate = (av_info.timing.sample_rate + 0.5) as u32;
    let volume = retrodeck_native::game_audio::volume_percent().unwrap_or_else(|error| {
        eprintln!("{name}: {error}");
        0
    });
    if test_frames.is_none()
        && let Err(error) = retrodeck_native::game_audio::open(sample_rate, volume)
    {
        eprintln!("{name}: sound disabled: {error}");
    }

    let persistent_base = save_base(rom_path);
    let persistent_memory = load_persistent_memory(&persistent_base, configuration);
    let library = if system_info.library_name.is_null() {
        configuration.default_core_name.to_owned()
    } else {
        unsafe { CStr::from_ptr(system_info.library_name) }
            .to_string_lossy()
            .into_owned()
    };
    let version = if system_info.library_version.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(system_info.library_version) }
            .to_string_lossy()
            .into_owned()
    };
    println!(
        "{name}: {library} {version}, {:.3} fps, {sample_rate} Hz, volume {volume}%",
        av_info.timing.fps
    );

    let mut clock = retrodeck_native::game_video::FrameClock::new(av_info.timing.fps);
    let mut frames: u64 = 0;
    let diagnostics = std::env::var_os("RETRO_DECK_RUNTIME_DIAGNOSTICS").is_some();
    let mut diagnostics_started = std::time::Instant::now();
    let mut previous_audio_frames = 0_u64;
    let mut previous_audio_callbacks = 0_u64;
    let mut focus_failed = false;
    let snes_video_interval = if test_frames.is_some() {
        1
    } else {
        // At full NTSC cadence, uploading every SHM frame slows the core.
        // Draw alternate frames at a steady cadence without dropping audio
        // or emulation work. The diagnostic divisor can reduce this further.
        u64::from(VIDEO_DIVISOR.load(Ordering::Relaxed)).max(2)
    };
    let mut snes_frame = 0_u64;
    while !retrodeck_native::process::shutdown_requested() && !VIDEO_FAILED.load(Ordering::Relaxed)
    {
        match retrodeck_native::game_video::before_frame(|_| {
            clock.reset();
            snes_frame = 0;
            PRESENT_SKIP.store(false, Ordering::Relaxed);
            diagnostics_started = std::time::Instant::now();
        }) {
            Ok(true) => {}
            Ok(false) => break,
            Err(error) => {
                eprintln!("{name}: game widget unavailable: {error}");
                focus_failed = true;
                break;
            }
        }
        if configuration.snes {
            SNES_VIDEO_ENABLED.store(snes_frame % snes_video_interval == 0, Ordering::Relaxed);
            snes_frame += 1;
        } else if test_frames.is_none() && clock.lateness() > clock.frame_nanoseconds() / 2 {
            PRESENT_SKIP.store(true, Ordering::Relaxed);
        }
        unsafe { retro_run() };
        frames += 1;
        if let Some(limit) = test_frames
            && frames >= limit
        {
            println!(
                "{name}: test frames={frames} video={} audio={} hash={:016x}",
                VIDEO_CALLBACKS.load(Ordering::Relaxed),
                AUDIO_FRAMES.load(Ordering::Relaxed),
                retrodeck_native::game_video::frame_hash()
            );
            break;
        }
        if diagnostics && frames % 60 == 0 {
            let elapsed = diagnostics_started.elapsed().as_secs_f64();
            let audio_frames = AUDIO_FRAMES.load(Ordering::Relaxed);
            let audio_calls = AUDIO_CALLBACKS.load(Ordering::Relaxed);
            println!(
                "{name}: diagnostics video=60 wall={elapsed:.3} audio={} callbacks={} queued={} dropped={}",
                audio_frames - previous_audio_frames,
                audio_calls - previous_audio_callbacks,
                retrodeck_native::game_audio::queued_frames(),
                retrodeck_native::game_audio::dropped_frames()
            );
            diagnostics_started = std::time::Instant::now();
            previous_audio_frames = audio_frames;
            previous_audio_callbacks = audio_calls;
        }
        if frames % 600 == 0
            && let Err(error) =
                save_persistent_memory(&persistent_base, configuration, &persistent_memory)
        {
            eprintln!("{name}: periodic save failed: {error}");
        }
        if test_frames.is_none() {
            clock.wait_for_next_frame();
        }
    }

    if let Err(error) = save_persistent_memory(&persistent_base, configuration, &persistent_memory)
    {
        eprintln!("{name}: final save failed: {error}");
    }
    retrodeck_native::game_audio::close();
    retrodeck_native::game_video::close();
    unsafe {
        retro_unload_game();
        retro_deinit();
    }
    u8::from(focus_failed || VIDEO_FAILED.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_save_bases_like_the_cpp_frontend() {
        assert_eq!(save_base("/roms/nes/mario.nes"), "/roms/nes/mario");
        assert_eq!(save_base("/roms/nes/mario"), "/roms/nes/mario");
        assert_eq!(save_base("mario.nes"), "mario");
        assert_eq!(save_base("/dotted.dir/rom"), "/dotted.dir/rom");
    }

    thread_local! {
        static SRAM: std::cell::UnsafeCell<[u8; 8]> = const { std::cell::UnsafeCell::new([0xA5; 8]) };
    }

    // Only the memory ABI is needed by these host-side persistence tests.
    #[unsafe(no_mangle)]
    extern "C" fn retro_get_memory_data(_id: c_uint) -> *mut c_void {
        SRAM.with(|memory| memory.get().cast())
    }

    #[unsafe(no_mangle)]
    extern "C" fn retro_get_memory_size(_id: c_uint) -> usize {
        8
    }

    #[test]
    fn protects_rejected_saves_and_writes_valid_or_new_saves() {
        let directory =
            std::env::temp_dir().join(format!("retro-host-save-{}", std::process::id()));
        std::fs::create_dir(&directory).expect("create isolated saves");
        let base = directory.join("game").to_str().unwrap().to_owned();
        let path = format!("{base}.srm");
        for rejected in [&[0xFF, 0x42][..], &[7, 7, 1, 200]] {
            std::fs::write(&path, rejected).unwrap();
            let memory = load_persistent_memory(&base, &NES_CONFIG);
            assert!(!memory.save_ram_writable);
            SRAM.with(|sram| assert_eq!(unsafe { *sram.get() }, [0xA5; 8]));
            // Both the periodic and final save use this same guarded path.
            save_persistent_memory(&base, &NES_CONFIG, &memory).unwrap();
            save_persistent_memory(&base, &NES_CONFIG, &memory).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), rejected);
        }
        std::fs::write(&path, [7, 7, 42, 7]).unwrap();
        let memory = load_persistent_memory(&base, &NES_CONFIG);
        assert!(memory.save_ram_writable);
        save_persistent_memory(&base, &NES_CONFIG, &memory).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), [42; 8]);

        std::fs::write(&path, [0x53; 8]).unwrap();
        let memory = load_persistent_memory(&base, &NES_CONFIG);
        assert!(memory.save_ram_writable);
        SRAM.with(|sram| assert_eq!(unsafe { *sram.get() }, [0x53; 8]));

        std::fs::remove_file(&path).unwrap();
        let memory = load_persistent_memory(&base, &NES_CONFIG);
        assert!(memory.save_ram_writable);
        save_persistent_memory(&base, &NES_CONFIG, &memory).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), [0x53; 8]);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_pixel_formats_without_changing_the_negotiated_format() {
        for accepted in [PIXEL_FORMAT_RGB565, PIXEL_FORMAT_XRGB8888] {
            let mut format = accepted;
            assert!(unsafe {
                environment_callback(ENV_SET_PIXEL_FORMAT, (&mut format as *mut c_uint).cast())
            });
            let mut unsupported: c_uint = 0;
            assert!(!unsafe {
                environment_callback(
                    ENV_SET_PIXEL_FORMAT,
                    (&mut unsupported as *mut c_uint).cast(),
                )
            });
            assert!(!unsafe { environment_callback(ENV_SET_PIXEL_FORMAT, std::ptr::null_mut()) });
            assert_eq!(PIXEL_FORMAT.load(Ordering::Relaxed), accepted);
        }
    }

    #[test]
    fn snes_retropad_keeps_face_buttons_distinct_and_preserves_keyboard() {
        for (button, id) in [
            (joypad::PAD_B, 0),
            (joypad::PAD_Y, 1),
            (joypad::PAD_SELECT, 2),
            (joypad::PAD_START, 3),
            (joypad::PAD_UP, 4),
            (joypad::PAD_DOWN, 5),
            (joypad::PAD_LEFT, 6),
            (joypad::PAD_RIGHT, 7),
            (joypad::PAD_A, 8),
            (joypad::PAD_X, 9),
            (joypad::PAD_L, 10),
            (joypad::PAD_R, 11),
        ] {
            assert_eq!(snes_input_mask(button, 0), 1 << id);
        }
        assert_eq!(
            snes_input_mask(joypad::PAD_Y, joypad::PAD_A | joypad::PAD_UP),
            0x112
        );
        assert_eq!(snes_input_mask(joypad::PAD_X, joypad::PAD_B), 0x201);
    }

    #[test]
    fn snes_video_normalizes_doubled_modes() {
        assert_eq!(snes_video_steps(256, 224), (1, 1));
        assert_eq!(snes_video_steps(256, 239), (1, 1));
        assert_eq!(snes_video_steps(512, 224), (2, 1));
        assert_eq!(snes_video_steps(256, 448), (1, 2));
        assert_eq!(snes_video_steps(512, 478), (2, 2));
        let mut rgb565 = [0; 2];
        let pair = [0xf800u16.to_ne_bytes(), 0x07e0u16.to_ne_bytes()].concat();
        blend_snes_row(&pair, &mut rgb565, true);
        assert_eq!(u16::from_ne_bytes(rgb565), 0x7be0);
        let mut xrgb = [0; 4];
        blend_snes_row(&[0, 2, 4, 0, 8, 12, 16, 0], &mut xrgb, false);
        assert_eq!(xrgb, [4, 7, 10, 0]);
    }
}
