//! Two-THEGamepad evdev input plus the medium-raw VT keyboard fallback,
//! sampled as held levels by the libretro input callback.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

pub const PAD_A: u32 = 1 << 0;
pub const PAD_B: u32 = 1 << 1;
pub const PAD_SELECT: u32 = 1 << 2;
pub const PAD_START: u32 = 1 << 3;
pub const PAD_UP: u32 = 1 << 4;
pub const PAD_DOWN: u32 = 1 << 5;
pub const PAD_LEFT: u32 = 1 << 6;
pub const PAD_RIGHT: u32 = 1 << 7;
pub const PAD_L: u32 = 1 << 8;
pub const PAD_R: u32 = 1 << 9;
/// Only ever set in the distinct state. The console mapping deliberately
/// folds X into A and Y into B; DOOM needs four separate face buttons.
pub const PAD_X: u32 = 1 << 10;
pub const PAD_Y: u32 = 1 << 11;

/// (pad bit, libretro joypad id). L and R are consumed only by the ZX build.
pub const RETRO_BUTTON_MAP: [(u32, u32); 8] = [
    (PAD_B, 0),
    (PAD_SELECT, 2),
    (PAD_START, 3),
    (PAD_UP, 4),
    (PAD_DOWN, 5),
    (PAD_LEFT, 6),
    (PAD_RIGHT, 7),
    (PAD_A, 8),
];
pub const RETRO_ZX_BUTTON_MAP: [(u32, u32); 2] = [(PAD_L, 10), (PAD_R, 11)];

const GAMEPAD_VENDOR: u16 = 0x1c59;
const GAMEPAD_PRODUCT: u16 = 0x0026;
const PLAYER_COUNT: usize = 2;

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const SYN_REPORT: u16 = 0;
const SYN_DROPPED: u16 = 3;
const BTN_TRIGGER: u16 = 0x120;
const BTN_BASE2: u16 = 0x127;
const ABS_X: u16 = 0;
const ABS_Y: u16 = 1;

const KEY_STATE_BYTES: usize = 96;

const fn ioc_read(nr: u32, size: u32) -> libc::c_ulong {
    ((2 << 30) | (size << 16) | (('E' as u32) << 8) | nr) as libc::c_ulong
}

const EVIOCGID: libc::c_ulong = ioc_read(0x02, 8);
const EVIOCGPHYS: libc::c_ulong = ioc_read(0x07, 256);
const EVIOCGKEY: libc::c_ulong = ioc_read(0x18, KEY_STATE_BYTES as u32);
const EVIOCGABS_X: libc::c_ulong = ioc_read(0x40 + ABS_X as u32, 24);
const EVIOCGABS_Y: libc::c_ulong = ioc_read(0x40 + ABS_Y as u32, 24);

const KDGKBTYPE: libc::c_ulong = 0x4b33;
const KDGKBMODE: libc::c_ulong = 0x4b44;
const KDSKBMODE: libc::c_ulong = 0x4b45;
const K_MEDIUMRAW: libc::c_int = 0x02;
const KB_84: libc::c_char = 0x01;
const KB_101: libc::c_char = 0x02;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct AbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

struct Gamepad {
    fd: Option<OwnedFd>,
    path: String,
    physical_path: String,
    x_info: AbsInfo,
    y_info: AbsInfo,
    x_value: i32,
    y_value: i32,
    raw_buttons: u8,
    state: u32,
    distinct: u32,
    dropping_events: bool,
}

impl Gamepad {
    const fn empty() -> Self {
        Self {
            fd: None,
            path: String::new(),
            physical_path: String::new(),
            x_info: AbsInfo {
                value: 0,
                minimum: 0,
                maximum: 0,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            y_info: AbsInfo {
                value: 0,
                minimum: 0,
                maximum: 0,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
            x_value: 0,
            y_value: 0,
            raw_buttons: 0,
            state: 0,
            distinct: 0,
            dropping_events: false,
        }
    }
}

struct Keyboard {
    fd: RawFd,
    owned: bool,
    saved_mode: libc::c_int,
    saved_termios: libc::termios,
    saved_flags: libc::c_int,
}

struct Inner {
    gamepads: [Gamepad; PLAYER_COUNT],
    keyboard: Option<Keyboard>,
    keyboard_keys: [bool; 128],
    keyboard_mask: u32,
    zx: bool,
    diagnostics: bool,
}

struct Shared {
    inner: Mutex<Inner>,
    pad_state: Mutex<[u32; PLAYER_COUNT]>,
    /// Gamepads only, with X and Y kept apart from A and B. The keyboard
    /// fallback is deliberately absent: DOOM reads real keys instead.
    pad_distinct: Mutex<[u32; PLAYER_COUNT]>,
    stopping: AtomicBool,
}

static SHARED: OnceLock<&'static Shared> = OnceLock::new();
static WORKER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

fn shared() -> &'static Shared {
    SHARED.get_or_init(|| {
        Box::leak(Box::new(Shared {
            inner: Mutex::new(Inner {
                gamepads: [Gamepad::empty(), Gamepad::empty()],
                keyboard: None,
                keyboard_keys: [false; 128],
                keyboard_mask: 0,
                zx: false,
                diagnostics: false,
            }),
            pad_state: Mutex::new([0; PLAYER_COUNT]),
            pad_distinct: Mutex::new([0; PLAYER_COUNT]),
            stopping: AtomicBool::new(false),
        }))
    })
}

fn keycode_to_pad(zx: bool, keycode: u8) -> u32 {
    if zx {
        return 0;
    }
    match keycode {
        0x67 | 0x11 => PAD_UP,
        0x6c | 0x1f => PAD_DOWN,
        0x69 | 0x1e => PAD_LEFT,
        0x6a | 0x20 => PAD_RIGHT,
        0x39 | 0x2c | 0x24 => PAD_A,
        0x2a | 0x36 | 0x2d | 0x25 => PAD_B,
        0x1c => PAD_START,
        0x1d | 0x61 => PAD_SELECT,
        _ => 0,
    }
}

fn gamepad_key_to_pad(code: u16) -> u32 {
    match code {
        0x120 | 0x121 => PAD_B,
        0x122 | 0x123 => PAD_A,
        0x124 => PAD_L,
        0x125 => PAD_R,
        0x126 => PAD_SELECT,
        0x127 => PAD_START,
        _ => 0,
    }
}

fn axis_to_pad(value: i32, minimum: i32, maximum: i32, negative: u32, positive: u32) -> u32 {
    if maximum <= minimum {
        return 0;
    }
    let span = i64::from(maximum) - i64::from(minimum);
    let low = i64::from(minimum) + span / 3;
    let high = i64::from(maximum) - span / 3;
    if i64::from(value) <= low {
        negative
    } else if i64::from(value) >= high {
        positive
    } else {
        0
    }
}

/// Reports X and Y as themselves instead of folding them onto A and B.
fn gamepad_key_to_pad_distinct(code: u16) -> u32 {
    match code {
        0x120 => PAD_B,
        0x121 => PAD_Y,
        0x122 => PAD_A,
        0x123 => PAD_X,
        other => gamepad_key_to_pad(other),
    }
}

fn gamepad_state(gamepad: &Gamepad) -> u32 {
    gamepad_state_mapped(gamepad, gamepad_key_to_pad)
}

fn gamepad_distinct_state(gamepad: &Gamepad) -> u32 {
    gamepad_state_mapped(gamepad, gamepad_key_to_pad_distinct)
}

fn gamepad_state_mapped(gamepad: &Gamepad, key_to_pad: fn(u16) -> u32) -> u32 {
    let mut state = 0;
    for bit in 0..8_u16 {
        if gamepad.raw_buttons & (1 << bit) != 0 {
            state |= key_to_pad(BTN_TRIGGER + bit);
        }
    }
    state |= axis_to_pad(
        gamepad.x_value,
        gamepad.x_info.minimum,
        gamepad.x_info.maximum,
        PAD_LEFT,
        PAD_RIGHT,
    );
    state |= axis_to_pad(
        gamepad.y_value,
        gamepad.y_info.minimum,
        gamepad.y_info.maximum,
        PAD_UP,
        PAD_DOWN,
    );
    state
}

fn publish_states(inner: &Inner) {
    let mut published = shared().pad_state.lock().expect("pad state lock");
    let next = [
        inner.keyboard_mask | inner.gamepads[0].state,
        inner.gamepads[1].state,
    ];
    if inner.diagnostics && *published != next {
        for (player, state) in next.iter().enumerate() {
            if published[player] != *state {
                eprintln!("Retro Deck: input diagnostic P{player} state=0x{state:02x}");
            }
        }
    }
    *published = next;
    drop(published);

    let mut distinct = shared().pad_distinct.lock().expect("pad distinct lock");
    *distinct = [inner.gamepads[0].distinct, inner.gamepads[1].distinct];
}

fn resynchronize(gamepad: &mut Gamepad) -> bool {
    let Some(fd) = gamepad.fd.as_ref().map(AsRawFd::as_raw_fd) else {
        return false;
    };
    let mut keys = [0_u8; KEY_STATE_BYTES];
    let mut x_info = AbsInfo::default();
    let mut y_info = AbsInfo::default();
    if unsafe { libc::ioctl(fd, EVIOCGKEY, keys.as_mut_ptr()) } < 0
        || unsafe { libc::ioctl(fd, EVIOCGABS_X, &mut x_info) } < 0
        || unsafe { libc::ioctl(fd, EVIOCGABS_Y, &mut y_info) } < 0
    {
        return false;
    }
    gamepad.x_info = x_info;
    gamepad.y_info = y_info;
    gamepad.x_value = x_info.value;
    gamepad.y_value = y_info.value;
    gamepad.raw_buttons = 0;
    for bit in 0..8_u16 {
        let code = (BTN_TRIGGER + bit) as usize;
        if keys[code / 8] & (1 << (code % 8)) != 0 {
            gamepad.raw_buttons |= 1 << bit;
        }
    }
    gamepad.state = gamepad_state(gamepad);
    gamepad.distinct = gamepad_distinct_state(gamepad);
    true
}

fn close_gamepad(gamepad: &mut Gamepad, remember_physical_path: bool) {
    gamepad.fd = None;
    gamepad.path.clear();
    if !remember_physical_path {
        gamepad.physical_path.clear();
    }
    gamepad.raw_buttons = 0;
    gamepad.state = 0;
    gamepad.distinct = 0;
    gamepad.dropping_events = false;
}

struct Candidate {
    fd: OwnedFd,
    path: String,
    physical_path: String,
    x_info: AbsInfo,
    y_info: AbsInfo,
}

fn probe_candidate(path: &str) -> Option<Candidate> {
    let name = std::ffi::CString::new(path).ok()?;
    let raw = unsafe {
        libc::open(
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if raw < 0 {
        return None;
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut id = InputId::default();
    let mut x_info = AbsInfo::default();
    let mut y_info = AbsInfo::default();
    if unsafe { libc::ioctl(raw, EVIOCGID, &mut id) } < 0
        || id.vendor != GAMEPAD_VENDOR
        || id.product != GAMEPAD_PRODUCT
        || unsafe { libc::ioctl(raw, EVIOCGABS_X, &mut x_info) } < 0
        || unsafe { libc::ioctl(raw, EVIOCGABS_Y, &mut y_info) } < 0
    {
        return None;
    }
    let mut physical = [0_u8; 256];
    let length = unsafe { libc::ioctl(raw, EVIOCGPHYS, physical.as_mut_ptr()) };
    let physical_path = if length > 0 {
        let bytes = &physical[..(length as usize).min(physical.len())];
        let end = bytes.iter().position(|&byte| byte == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    } else {
        String::new()
    };
    Some(Candidate {
        fd,
        path: path.to_owned(),
        physical_path: if physical_path.is_empty() {
            path.to_owned()
        } else {
            physical_path
        },
        x_info,
        y_info,
    })
}

fn attach_candidate(gamepad: &mut Gamepad, candidate: Candidate, player: usize) {
    gamepad.fd = Some(candidate.fd);
    gamepad.path = candidate.path;
    gamepad.physical_path = candidate.physical_path;
    gamepad.x_info = candidate.x_info;
    gamepad.y_info = candidate.y_info;
    gamepad.x_value = candidate.x_info.value;
    gamepad.y_value = candidate.y_info.value;
    gamepad.raw_buttons = 0;
    gamepad.state = 0;
    gamepad.distinct = 0;
    gamepad.dropping_events = false;
    resynchronize(gamepad);
    eprintln!(
        "Retro Deck: Player {} THEGamepad on {} ({})",
        player + 1,
        gamepad.path,
        gamepad.physical_path
    );
}

fn scan_gamepads(inner: &mut Inner) {
    let Ok(entries) = std::fs::read_dir("/dev/input") else {
        return;
    };
    let mut candidates: Vec<Candidate> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(suffix) = name.strip_prefix("event") else {
            continue;
        };
        if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let path = format!("/dev/input/{name}");
        if inner
            .gamepads
            .iter()
            .any(|gamepad| gamepad.fd.is_some() && gamepad.path == path)
        {
            continue;
        }
        if let Some(candidate) = probe_candidate(&path) {
            candidates.push(candidate);
        }
    }
    candidates.sort_by(|left, right| {
        left.physical_path
            .cmp(&right.physical_path)
            .then_with(|| left.path.cmp(&right.path))
    });

    let mut remaining: Vec<Option<Candidate>> = candidates.into_iter().map(Some).collect();
    // Pass 1: remembered ports reconnect to their player.
    for player in 0..PLAYER_COUNT {
        let gamepad = &inner.gamepads[player];
        if gamepad.fd.is_some() || gamepad.physical_path.is_empty() {
            continue;
        }
        if let Some(slot) = remaining.iter_mut().find(|slot| {
            slot.as_ref()
                .is_some_and(|candidate| candidate.physical_path == gamepad.physical_path)
        }) {
            let candidate = slot.take().expect("checked above");
            attach_candidate(&mut inner.gamepads[player], candidate, player);
        }
    }
    // Pass 2: never-used slots take the next candidate in stable order.
    for player in 0..PLAYER_COUNT {
        let gamepad = &inner.gamepads[player];
        if gamepad.fd.is_some() || !gamepad.physical_path.is_empty() {
            continue;
        }
        if let Some(slot) = remaining.iter_mut().find(|slot| slot.is_some()) {
            let candidate = slot.take().expect("checked above");
            attach_candidate(&mut inner.gamepads[player], candidate, player);
        }
    }
    // Pass 3: leftovers may move ports into any disconnected slot.
    for player in 0..PLAYER_COUNT {
        if inner.gamepads[player].fd.is_some() {
            continue;
        }
        if let Some(slot) = remaining.iter_mut().find(|slot| slot.is_some()) {
            let candidate = slot.take().expect("checked above");
            inner.gamepads[player].physical_path.clear();
            attach_candidate(&mut inner.gamepads[player], candidate, player);
        }
    }
    publish_states(inner);
}

fn drain_gamepad(gamepad: &mut Gamepad) -> bool {
    let Some(fd) = gamepad.fd.as_ref().map(AsRawFd::as_raw_fd) else {
        return false;
    };
    loop {
        let mut events = [libc::input_event {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            type_: 0,
            code: 0,
            value: 0,
        }; 32];
        let amount = unsafe {
            libc::read(
                fd,
                events.as_mut_ptr().cast(),
                std::mem::size_of_val(&events),
            )
        };
        if amount < 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            if error.raw_os_error() == Some(libc::EAGAIN) {
                break;
            }
            return false;
        }
        let event_size = std::mem::size_of::<libc::input_event>();
        if amount == 0 || amount as usize % event_size != 0 {
            return false;
        }
        for event in &events[..amount as usize / event_size] {
            if gamepad.dropping_events {
                if event.type_ == EV_SYN && event.code == SYN_REPORT {
                    gamepad.dropping_events = false;
                    if !resynchronize(gamepad) {
                        return false;
                    }
                }
                continue;
            }
            match (event.type_, event.code) {
                (EV_SYN, SYN_DROPPED) => gamepad.dropping_events = true,
                (EV_KEY, code) if (BTN_TRIGGER..=BTN_BASE2).contains(&code) => {
                    let bit = 1 << (code - BTN_TRIGGER);
                    if event.value != 0 {
                        gamepad.raw_buttons |= bit;
                    } else {
                        gamepad.raw_buttons &= !bit;
                    }
                }
                (EV_ABS, ABS_X) => gamepad.x_value = event.value,
                (EV_ABS, ABS_Y) => gamepad.y_value = event.value,
                _ => {}
            }
        }
        gamepad.state = gamepad_state(gamepad);
        gamepad.distinct = gamepad_distinct_state(gamepad);
    }
    true
}

fn is_console_keyboard(fd: RawFd) -> bool {
    let mut kind: libc::c_char = 0;
    (unsafe { libc::ioctl(fd, KDGKBTYPE, &mut kind) }) == 0 && (kind == KB_84 || kind == KB_101)
}

fn initialize_keyboard() -> Option<Keyboard> {
    let mut fd = -1;
    let mut owned = false;
    for path in [c"/dev/tty", c"/dev/tty0", c"/dev/console"] {
        let raw = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
        if raw >= 0 {
            if is_console_keyboard(raw) {
                fd = raw;
                owned = true;
                eprintln!(
                    "Retro Deck: Using keyboard on {}",
                    path.to_str().unwrap_or("?")
                );
                break;
            }
            unsafe { libc::close(raw) };
        }
    }
    if fd < 0 {
        for standard in 0..3 {
            if is_console_keyboard(standard) {
                fd = standard;
                owned = false;
                break;
            }
        }
    }
    if fd < 0 {
        return None;
    }
    let mut saved_mode: libc::c_int = 0;
    let mut saved_termios = unsafe { std::mem::zeroed::<libc::termios>() };
    if unsafe { libc::ioctl(fd, KDGKBMODE, &mut saved_mode) } != 0
        || unsafe { libc::tcgetattr(fd, &mut saved_termios) } != 0
    {
        if owned {
            unsafe { libc::close(fd) };
        }
        return None;
    }
    let mut raw_termios = saved_termios;
    raw_termios.c_iflag = 0;
    raw_termios.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG);
    if unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &raw_termios) } != 0 {
        if owned {
            unsafe { libc::close(fd) };
        }
        return None;
    }
    if unsafe { libc::ioctl(fd, KDSKBMODE, K_MEDIUMRAW as libc::c_long) } != 0 {
        unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &saved_termios) };
        if owned {
            unsafe { libc::close(fd) };
        }
        return None;
    }
    let saved_flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if saved_flags < 0
        || unsafe { libc::fcntl(fd, libc::F_SETFL, saved_flags | libc::O_NONBLOCK) } != 0
    {
        unsafe {
            libc::ioctl(fd, KDSKBMODE, saved_mode as libc::c_long);
            libc::tcsetattr(fd, libc::TCSAFLUSH, &saved_termios);
        }
        if owned {
            unsafe { libc::close(fd) };
        }
        return None;
    }
    Some(Keyboard {
        fd,
        owned,
        saved_mode,
        saved_termios,
        saved_flags,
    })
}

fn close_keyboard(inner: &mut Inner) {
    if let Some(keyboard) = inner.keyboard.take() {
        unsafe {
            libc::fcntl(keyboard.fd, libc::F_SETFL, keyboard.saved_flags);
            libc::ioctl(keyboard.fd, KDSKBMODE, keyboard.saved_mode as libc::c_long);
            libc::tcsetattr(keyboard.fd, libc::TCSAFLUSH, &keyboard.saved_termios);
            if keyboard.owned {
                libc::close(keyboard.fd);
            }
        }
    }
    inner.keyboard_keys = [false; 128];
    inner.keyboard_mask = 0;
    publish_states(inner);
}

fn drain_keyboard(inner: &mut Inner) {
    let Some(fd) = inner.keyboard.as_ref().map(|keyboard| keyboard.fd) else {
        return;
    };
    let mut buffer = [0_u8; 64];
    let amount = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
    if amount <= 0 {
        return;
    }
    let zx = inner.zx;
    for &byte in &buffer[..amount as usize] {
        let released = byte & 0x80 != 0;
        let keycode = byte & 0x7f;
        inner.keyboard_keys[usize::from(keycode)] = !released;
        let pad = keycode_to_pad(zx, keycode);
        if pad != 0 {
            if released {
                inner.keyboard_mask &= !pad;
            } else {
                inner.keyboard_mask |= pad;
            }
        }
    }
    publish_states(inner);
}

fn input_thread() {
    let shared = shared();
    let mut last_scan = std::time::Instant::now() - std::time::Duration::from_secs(2);
    while !shared.stopping.load(Ordering::Acquire) {
        {
            let mut inner = shared.inner.lock().expect("input lock");
            if last_scan.elapsed() >= std::time::Duration::from_secs(1) {
                scan_gamepads(&mut inner);
                last_scan = std::time::Instant::now();
            }
        }
        let mut descriptors: Vec<libc::pollfd> = Vec::with_capacity(1 + PLAYER_COUNT);
        let mut keyboard_slot = None;
        let mut player_slots = [None; PLAYER_COUNT];
        {
            let inner = shared.inner.lock().expect("input lock");
            if let Some(keyboard) = inner.keyboard.as_ref() {
                keyboard_slot = Some(descriptors.len());
                descriptors.push(libc::pollfd {
                    fd: keyboard.fd,
                    events: libc::POLLIN,
                    revents: 0,
                });
            }
            for (player, gamepad) in inner.gamepads.iter().enumerate() {
                if let Some(fd) = gamepad.fd.as_ref() {
                    player_slots[player] = Some(descriptors.len());
                    descriptors.push(libc::pollfd {
                        fd: fd.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    });
                }
            }
        }
        let ready = unsafe {
            libc::poll(
                if descriptors.is_empty() {
                    std::ptr::null_mut()
                } else {
                    descriptors.as_mut_ptr()
                },
                descriptors.len() as libc::nfds_t,
                100,
            )
        };
        if ready < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }
        if ready == 0 {
            continue;
        }
        let active = libc::POLLIN | libc::POLLERR | libc::POLLHUP | libc::POLLNVAL;
        let mut inner = shared.inner.lock().expect("input lock");
        if let Some(slot) = keyboard_slot
            && descriptors[slot].revents & active != 0
        {
            drain_keyboard(&mut inner);
        }
        for (player, slot) in player_slots.iter().enumerate() {
            if let Some(slot) = *slot
                && descriptors[slot].revents & active != 0
                && !drain_gamepad(&mut inner.gamepads[player])
            {
                eprintln!("Retro Deck: Player {} gamepad disconnected", player + 1);
                close_gamepad(&mut inner.gamepads[player], true);
            }
        }
        publish_states(&inner);
    }
}

pub fn initialize(zx: bool) -> Result<(), String> {
    let shared = shared();
    {
        let mut inner = shared.inner.lock().expect("input lock");
        inner.zx = zx;
        inner.diagnostics = std::env::var("RETRO_DECK_INPUT_DIAGNOSTICS")
            .is_ok_and(|value| value == "1");
        inner.keyboard = initialize_keyboard();
        if inner.keyboard.is_none() {
            eprintln!("Retro Deck: No raw keyboard available; gamepads remain enabled");
        }
        scan_gamepads(&mut inner);
    }
    shared.stopping.store(false, Ordering::Release);
    let worker = std::thread::Builder::new()
        .name("retro-deck-input".to_owned())
        .spawn(input_thread)
        .map_err(|error| format!("cannot start input thread: {error}"))?;
    *WORKER.lock().expect("input worker lock") = Some(worker);
    Ok(())
}

pub fn shutdown() {
    let shared = shared();
    shared.stopping.store(true, Ordering::Release);
    if let Some(worker) = WORKER.lock().expect("input worker lock").take() {
        let _ = worker.join();
    }
    let mut inner = shared.inner.lock().expect("input lock");
    close_keyboard(&mut inner);
    for gamepad in inner.gamepads.iter_mut() {
        close_gamepad(gamepad, false);
    }
}

pub fn joypad_state(port: u32) -> u32 {
    shared()
        .pad_state
        .lock()
        .map(|states| states.get(port as usize).copied().unwrap_or(0))
        .unwrap_or(0)
}

/// Held gamepad buttons with X and Y reported separately from A and B, and
/// without the keyboard fallback folded in. Only the DOOM host uses this.
pub fn joypad_distinct_state(port: u32) -> u32 {
    shared()
        .pad_distinct
        .lock()
        .map(|states| states.get(port as usize).copied().unwrap_or(0))
        .unwrap_or(0)
}

pub fn keyboard_key_held(keycode: u16) -> bool {
    if keycode >= 128 {
        return false;
    }
    shared()
        .inner
        .lock()
        .map(|inner| inner.keyboard_keys[usize::from(keycode)])
        .unwrap_or(false)
}

const RETROK_BACKSPACE: u32 = 8;
const RETROK_RETURN: u32 = 13;
const RETROK_SPACE: u32 = 32;
const RETROK_0: u32 = 48;
const RETROK_9: u32 = 57;
const RETROK_A: u32 = 97;
const RETROK_Z: u32 = 122;
const RETROK_UP: u32 = 273;
const RETROK_DOWN: u32 = 274;
const RETROK_RIGHT: u32 = 275;
const RETROK_LEFT: u32 = 276;
const RETROK_RSHIFT: u32 = 303;
const RETROK_LSHIFT: u32 = 304;
const RETROK_RCTRL: u32 = 305;
const RETROK_LCTRL: u32 = 306;
const RETROK_RALT: u32 = 307;
const RETROK_LALT: u32 = 308;
const RETROK_LSUPER: u32 = 310;
const RETROK_RSUPER: u32 = 311;

const ZX_DIGIT_KEYCODES: [u16; 10] = [11, 2, 3, 4, 5, 6, 7, 8, 9, 10];
const ZX_LETTER_KEYCODES: [u16; 26] = [
    30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17,
    45, 21, 44,
];

pub fn zx_linux_keycode(retro_key: u32) -> u16 {
    match retro_key {
        RETROK_0..=RETROK_9 => ZX_DIGIT_KEYCODES[(retro_key - RETROK_0) as usize],
        RETROK_A..=RETROK_Z => ZX_LETTER_KEYCODES[(retro_key - RETROK_A) as usize],
        RETROK_RETURN => 28,
        RETROK_SPACE => 57,
        RETROK_BACKSPACE => 14,
        RETROK_LSHIFT => 42,
        RETROK_RSHIFT => 54,
        RETROK_LCTRL => 29,
        RETROK_RCTRL => 97,
        RETROK_LALT => 56,
        RETROK_RALT => 100,
        RETROK_LSUPER => 125,
        RETROK_RSUPER => 126,
        RETROK_UP => 103,
        RETROK_DOWN => 108,
        RETROK_LEFT => 105,
        RETROK_RIGHT => 106,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_gamepad_buttons_like_the_cpp_frontend() {
        let expected = [
            (0x120, PAD_B),
            (0x121, PAD_B),
            (0x122, PAD_A),
            (0x123, PAD_A),
            (0x124, PAD_L),
            (0x125, PAD_R),
            (0x126, PAD_SELECT),
            (0x127, PAD_START),
        ];
        for (code, pad) in expected {
            assert_eq!(gamepad_key_to_pad(code), pad);
        }
        assert_eq!(gamepad_key_to_pad(0x128), 0);
    }

    #[test]
    fn applies_the_inclusive_thirds_rule() {
        assert_eq!(axis_to_pad(0, 0, 255, PAD_LEFT, PAD_RIGHT), PAD_LEFT);
        assert_eq!(axis_to_pad(85, 0, 255, PAD_LEFT, PAD_RIGHT), PAD_LEFT);
        assert_eq!(axis_to_pad(127, 0, 255, PAD_LEFT, PAD_RIGHT), 0);
        assert_eq!(axis_to_pad(170, 0, 255, PAD_LEFT, PAD_RIGHT), PAD_RIGHT);
        assert_eq!(axis_to_pad(255, 0, 255, PAD_LEFT, PAD_RIGHT), PAD_RIGHT);
        assert_eq!(axis_to_pad(-32767, -32767, 32767, PAD_UP, PAD_DOWN), PAD_UP);
        assert_eq!(axis_to_pad(0, -32767, 32767, PAD_UP, PAD_DOWN), 0);
        assert_eq!(axis_to_pad(32767, -32767, 32767, PAD_UP, PAD_DOWN), PAD_DOWN);
        assert_eq!(axis_to_pad(5, 5, 5, PAD_UP, PAD_DOWN), 0);
    }

    #[test]
    fn keeps_the_keyboard_fallback_map_and_zx_override() {
        assert_eq!(keycode_to_pad(false, 0x67), PAD_UP);
        assert_eq!(keycode_to_pad(false, 0x39), PAD_A);
        assert_eq!(keycode_to_pad(false, 0x2a), PAD_B);
        assert_eq!(keycode_to_pad(false, 0x1c), PAD_START);
        assert_eq!(keycode_to_pad(false, 0x1d), PAD_SELECT);
        assert_eq!(keycode_to_pad(false, 0x63), 0);
        for keycode in 0..128_u8 {
            assert_eq!(keycode_to_pad(true, keycode), 0);
        }
    }

    #[test]
    fn translates_retro_keys_to_linux_keycodes() {
        assert_eq!(zx_linux_keycode(RETROK_0), 11);
        assert_eq!(zx_linux_keycode(RETROK_0 + 1), 2);
        assert_eq!(zx_linux_keycode(RETROK_A), 30);
        assert_eq!(zx_linux_keycode(RETROK_A + 25), 44);
        assert_eq!(zx_linux_keycode(RETROK_RETURN), 28);
        assert_eq!(zx_linux_keycode(282), 0);
    }
}
