//! Emulator video: integer nearest-neighbour scaling into the rotated Deck
//! framebuffer, the normal BMC widget surface, or a headless test target.

use std::ffi::c_void;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const LOGICAL_WIDTH: usize = 1280;
const LOGICAL_HEIGHT: usize = 480;
const SAFE_INSET: usize = 16;

pub struct Layout {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub scale: usize,
}

pub fn compute_scaled_layout(source_width: usize, source_height: usize) -> Option<Layout> {
    let usable_width = LOGICAL_WIDTH - 2 * SAFE_INSET;
    let usable_height = LOGICAL_HEIGHT - 2 * SAFE_INSET;
    if source_width == 0
        || source_height == 0
        || source_width > usable_width
        || source_height > usable_height
    {
        return None;
    }
    let scale = (usable_width / source_width).min(usable_height / source_height);
    if scale < 1 {
        return None;
    }
    let width = source_width * scale;
    let height = source_height * scale;
    let x = (LOGICAL_WIDTH - width) / 2;
    let y = (LOGICAL_HEIGHT - height) / 2;
    (x >= SAFE_INSET
        && y >= SAFE_INSET
        && x + width <= LOGICAL_WIDTH - SAFE_INSET
        && y + height <= LOGICAL_HEIGHT - SAFE_INSET)
        .then_some(Layout {
            x,
            y,
            width,
            height,
            scale,
        })
}

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;
const FB_TYPE_PACKED_PIXELS: u32 = 0;
const FB_VISUAL_TRUECOLOR: u32 = 2;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FbBitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FbVarScreeninfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FbFixScreeninfo {
    id: [u8; 16],
    smem_start: libc::c_ulong,
    smem_len: u32,
    kind: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: libc::c_ulong,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

struct FbTarget {
    _fd: OwnedFd,
    memory: *mut u16,
    map_bytes: usize,
    row_words: usize,
    staging: Vec<u16>,
    exit_hint: bool,
}

unsafe impl Send for FbTarget {}

enum Target {
    Fb(FbTarget),
    WidgetLazy,
    Widget { converted: Vec<u32> },
    Headless { frame: Vec<u16>, hash: u64 },
}

struct Video {
    target: Target,
    last_source: (usize, usize),
    exit_hint: bool,
    exit_hold_started: Option<Instant>,
}

static VIDEO: Mutex<Option<Video>> = Mutex::new(None);

pub fn rgb888_to_565(color: u32) -> u16 {
    let red = (color >> 16) & 0xff;
    let green = (color >> 8) & 0xff;
    let blue = color & 0xff;
    (((red & 0xf8) << 8) | ((green & 0xfc) << 3) | (blue >> 3)) as u16
}

fn open_fb() -> Result<FbTarget, String> {
    let raw = unsafe { libc::open(c"/dev/fb0".as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
    if raw < 0 {
        return Err(format!(
            "cannot open /dev/fb0: {}",
            std::io::Error::last_os_error()
        ));
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut var = FbVarScreeninfo::default();
    let mut fix = unsafe { std::mem::zeroed::<FbFixScreeninfo>() };
    if unsafe { libc::ioctl(fd.as_raw_fd(), FBIOGET_VSCREENINFO, &mut var) } != 0
        || unsafe { libc::ioctl(fd.as_raw_fd(), FBIOGET_FSCREENINFO, &mut fix) } != 0
    {
        return Err(format!(
            "cannot query /dev/fb0: {}",
            std::io::Error::last_os_error()
        ));
    }
    let rows = if var.yres_virtual != 0 { var.yres_virtual } else { var.yres } as usize;
    let line_length = fix.line_length as usize;
    let supported = var.xres == 600
        && var.yres == 1280
        && var.bits_per_pixel == 16
        && var.xoffset == 0
        && var.yoffset == 0
        && rows >= 1280
        && fix.kind == FB_TYPE_PACKED_PIXELS
        && fix.visual == FB_VISUAL_TRUECOLOR
        && line_length >= 1200
        && line_length % 2 == 0
        && line_length != 0
        && rows <= usize::MAX / line_length
        && fix.smem_len as usize >= line_length * rows
        && (var.red.offset, var.red.length, var.red.msb_right) == (11, 5, 0)
        && (var.green.offset, var.green.length, var.green.msb_right) == (5, 6, 0)
        && (var.blue.offset, var.blue.length, var.blue.msb_right) == (0, 5, 0)
        && var.transp.length == 0;
    if !supported {
        return Err(
            "unsupported framebuffer; expected 600x1280 RGB565 with a valid stride".to_owned(),
        );
    }
    let map_bytes = fix.smem_len as usize;
    let memory = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            map_bytes,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd.as_raw_fd(),
            0,
        )
    };
    if memory == libc::MAP_FAILED {
        return Err(format!(
            "cannot map /dev/fb0: {}",
            std::io::Error::last_os_error()
        ));
    }
    unsafe { std::ptr::write_bytes(memory.cast::<u8>(), 0, map_bytes) };
    Ok(FbTarget {
        _fd: fd,
        memory: memory.cast(),
        map_bytes,
        row_words: line_length / 2,
        staging: vec![0_u16; map_bytes / 2],
        exit_hint: std::env::var("RETRO_DECK_EXIT_HINT").is_ok_and(|value| value == "1"),
    })
}

impl Drop for FbTarget {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.memory.cast::<c_void>(), self.map_bytes) };
    }
}

fn draw_exit_hint(memory: *mut u16, row_words: usize) {
    if row_words < 600 {
        return;
    }
    let fill = |logical_x: isize, logical_y: isize, size: isize, color: u16| {
        for y in logical_y..logical_y + size {
            for x in logical_x..logical_x + size {
                if (0..1280).contains(&x) && (0..480).contains(&y) {
                    let row = 1279 - x as usize;
                    unsafe {
                        *memory.add(row * row_words + y as usize) = color;
                    }
                }
            }
        }
    };
    for step in 0..9_isize {
        fill(20 + step * 4 - 2, 20 + step * 4 - 2, 8, 0x0000);
        fill(20 + (8 - step) * 4 - 2, 20 + step * 4 - 2, 8, 0x0000);
    }
    for step in 0..9_isize {
        fill(20 + step * 4, 20 + step * 4, 4, 0xffff);
        fill(20 + (8 - step) * 4, 20 + step * 4, 4, 0xffff);
    }
}

impl FbTarget {
    fn begin_frame(&mut self, width: usize, height: usize, last: &mut (usize, usize)) {
        if *last != (width, height) {
            self.staging.fill(0);
            unsafe { std::ptr::write_bytes(self.memory.cast::<u8>(), 0, self.map_bytes) };
            *last = (width, height);
        }
    }

    fn publish(&mut self, layout: &Layout) {
        let first_physical_row = LOGICAL_WIDTH - layout.x - layout.width;
        for row in 0..layout.width {
            let offset = (first_physical_row + row) * self.row_words + layout.y;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.staging.as_ptr().add(offset),
                    self.memory.add(offset),
                    layout.height,
                );
            }
        }
        if self.exit_hint {
            draw_exit_hint(self.memory, self.row_words);
        }
    }

    fn draw_rgb565(&mut self, data: *const c_void, layout: &Layout, height: usize, pitch: usize) {
        let source = data.cast::<u8>();
        for source_x in 0..layout.width / layout.scale {
            let first_physical_row = 1279 - layout.x - source_x * layout.scale;
            for duplicate in 0..layout.scale {
                let row = first_physical_row - duplicate;
                let mut offset = row * self.row_words + layout.y;
                for source_y in 0..height {
                    let pixel = unsafe {
                        source
                            .add(source_y * pitch + source_x * 2)
                            .cast::<u16>()
                            .read_unaligned()
                    };
                    for _ in 0..layout.scale {
                        self.staging[offset] = pixel;
                        offset += 1;
                    }
                }
            }
        }
    }

    fn draw_xrgb(&mut self, data: *const c_void, layout: &Layout, height: usize, pitch: usize) {
        let source = data.cast::<u8>();
        for source_x in 0..layout.width / layout.scale {
            let first_physical_row = 1279 - layout.x - source_x * layout.scale;
            for source_y in 0..height {
                let color = unsafe {
                    source
                        .add(source_y * pitch + source_x * 4)
                        .cast::<u32>()
                        .read_unaligned()
                };
                let pixel = rgb888_to_565(color);
                for duplicate in 0..layout.scale {
                    let row = first_physical_row - duplicate;
                    let mut offset = row * self.row_words + layout.y + source_y * layout.scale;
                    for _ in 0..layout.scale {
                        self.staging[offset] = pixel;
                        offset += 1;
                    }
                }
            }
        }
    }
}

fn convert_to_xrgb(
    data: *const c_void,
    width: usize,
    height: usize,
    pitch: usize,
    rgb565: bool,
    output: &mut Vec<u32>,
) {
    output.clear();
    output.reserve(width * height);
    let source = data.cast::<u8>();
    for y in 0..height {
        for x in 0..width {
            let value = if rgb565 {
                let pixel = unsafe { source.add(y * pitch + x * 2).cast::<u16>().read_unaligned() };
                let red = u32::from((pixel >> 11) & 0x1f);
                let green = u32::from((pixel >> 5) & 0x3f);
                let blue = u32::from(pixel & 0x1f);
                0xff00_0000 | ((red * 255 / 31) << 16) | ((green * 255 / 63) << 8)
                    | (blue * 255 / 31)
            } else {
                let pixel = unsafe { source.add(y * pitch + x * 4).cast::<u32>().read_unaligned() };
                0xff00_0000 | (pixel & 0x00ff_ffff)
            };
            output.push(value);
        }
    }
}

pub fn open(headless: bool) -> Result<(), String> {
    let exit_hint = std::env::var_os("RETRO_DECK_EXIT_HINT").is_some();
    let target = if headless {
        Target::Headless {
            frame: Vec::new(),
            hash: 0xcbf2_9ce4_8422_2325,
        }
    } else if std::env::var("RETRO_DECK_PRESENTATION").is_ok_and(|value| value == "widget") {
        Target::WidgetLazy
    } else {
        Target::Fb(open_fb()?)
    };
    *VIDEO.lock().expect("video lock") = Some(Video {
        target,
        last_source: (0, 0),
        exit_hint,
        exit_hold_started: None,
    });
    Ok(())
}

pub fn present(
    data: *const c_void,
    width: u32,
    height: u32,
    pitch: usize,
    rgb565: bool,
) -> Result<(), String> {
    let width = width as usize;
    let height = height as usize;
    let minimum_pitch = if rgb565 { width * 2 } else { width * 4 };
    if pitch < minimum_pitch {
        return Err("video frame pitch is too small".to_owned());
    }
    let mut guard = VIDEO.lock().expect("video lock");
    let video = guard
        .as_mut()
        .ok_or_else(|| "video target is not open".to_owned())?;
    match &mut video.target {
        Target::Headless { frame, hash } => {
            frame.clear();
            for y in 0..height {
                for x in 0..width {
                    let pixel = if rgb565 {
                        unsafe {
                            data.cast::<u8>()
                                .add(y * pitch + x * 2)
                                .cast::<u16>()
                                .read_unaligned()
                        }
                    } else {
                        let color = unsafe {
                            data.cast::<u8>()
                                .add(y * pitch + x * 4)
                                .cast::<u32>()
                                .read_unaligned()
                        };
                        rgb888_to_565(color)
                    };
                    frame.push(pixel);
                }
            }
            for byte in frame.iter().flat_map(|pixel| pixel.to_le_bytes()) {
                *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            }
            Ok(())
        }
        Target::WidgetLazy => {
            crate::wayland::open_game_widget()?;
            video.target = Target::Widget {
                converted: Vec::new(),
            };
            let Target::Widget { converted } = &mut video.target else {
                unreachable!();
            };
            convert_to_xrgb(data, width, height, pitch, rgb565, converted);
            crate::wayland::present_game_xrgb(converted, width, height)
        }
        Target::Widget { converted } => {
            convert_to_xrgb(data, width, height, pitch, rgb565, converted);
            crate::wayland::present_game_xrgb(converted, width, height)
        }
        Target::Fb(fb) => {
            let layout = compute_scaled_layout(width, height)
                .ok_or_else(|| "video frame does not fit the Deck safe area".to_owned())?;
            fb.begin_frame(width, height, &mut video.last_source);
            if rgb565 {
                fb.draw_rgb565(data, &layout, height, pitch);
            } else {
                fb.draw_xrgb(data, &layout, height, pitch);
            }
            fb.publish(&layout);
            Ok(())
        }
    }
}

fn update_exit_hold(
    started: &mut Option<Instant>,
    down: bool,
    x: i32,
    y: i32,
    now: Instant,
) -> bool {
    let inside = down
        && x >= 0
        && x < LOGICAL_WIDTH as i32
        && y >= 0
        && y < LOGICAL_HEIGHT as i32;
    if !inside {
        *started = None;
        return false;
    }
    let began = started.get_or_insert(now);
    now.duration_since(*began) >= Duration::from_secs(2)
}

/// Check the BMC widget surface for the console/DOOM return hold.
///
/// The game owns the normal widget surface, so its parent dashboard cannot
/// receive this touch stream after BMC switches the render surface.
pub fn exit_requested() -> bool {
    let mut guard = VIDEO.lock().expect("video lock");
    let Some(video) = guard.as_mut() else {
        return false;
    };
    if !video.exit_hint || !matches!(&video.target, Target::Widget { .. }) {
        return false;
    }
    if let Err(error) = crate::wayland::dispatch(0) {
        eprintln!("retrodeck: game widget input unavailable: {error}");
        video.exit_hold_started = None;
        return false;
    }
    if crate::wayland::shutdown_requested() {
        return true;
    }
    let now = Instant::now();
    let mut complete = false;
    while let Some(report) = crate::wayland::next_touch() {
        complete |= update_exit_hold(
            &mut video.exit_hold_started,
            report.down,
            report.x,
            report.y,
            now,
        );
    }
    complete
        || video
            .exit_hold_started
            .is_some_and(|started| now.duration_since(started) >= Duration::from_secs(2))
}

pub fn frame_hash() -> u64 {
    let guard = VIDEO.lock().expect("video lock");
    match guard.as_ref().map(|video| &video.target) {
        Some(Target::Headless { hash, .. }) => *hash,
        _ => 0,
    }
}

pub fn close() {
    let target = VIDEO.lock().expect("video lock").take();
    if let Some(video) = target
        && matches!(video.target, Target::Widget { .. })
    {
        crate::wayland::close();
    }
}

pub struct FrameClock {
    start_nanoseconds: i64,
    frame_nanoseconds: i64,
    frame_number: i64,
}

fn monotonic_now() -> i64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return 0;
    }
    time.tv_sec as i64 * 1_000_000_000 + time.tv_nsec as i64
}

impl FrameClock {
    pub fn new(fps: f64) -> Self {
        Self {
            start_nanoseconds: monotonic_now(),
            frame_nanoseconds: if fps > 0.0 {
                (1_000_000_000.0 / fps) as i64
            } else {
                0
            },
            frame_number: 0,
        }
    }

    /// Nanoseconds the loop is running behind the next frame deadline;
    /// zero or negative when on schedule.
    pub fn lateness(&self) -> i64 {
        if self.frame_nanoseconds <= 0 {
            return 0;
        }
        let deadline =
            self.start_nanoseconds + (self.frame_number + 1) * self.frame_nanoseconds;
        monotonic_now() - deadline
    }

    pub fn frame_nanoseconds(&self) -> i64 {
        self.frame_nanoseconds
    }

    pub fn wait_for_next_frame(&mut self) {
        if self.frame_nanoseconds <= 0 {
            return;
        }
        self.frame_number += 1;
        let deadline = self.start_nanoseconds + self.frame_number * self.frame_nanoseconds;
        let target = libc::timespec {
            tv_sec: (deadline / 1_000_000_000) as libc::time_t,
            tv_nsec: (deadline % 1_000_000_000) as libc::c_long,
        };
        loop {
            let result = unsafe {
                libc::clock_nanosleep(
                    libc::CLOCK_MONOTONIC,
                    libc::TIMER_ABSTIME,
                    &target,
                    std::ptr::null_mut(),
                )
            };
            if result != libc::EINTR {
                break;
            }
        }
        let now = monotonic_now();
        if now - deadline > self.frame_nanoseconds * 5 {
            self.start_nanoseconds = now;
            self.frame_number = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_cpp_layout_fixture() {
        let cases = [
            (160, 144, 3, 400, 24, 480, 432),
            (256, 224, 2, 384, 16, 512, 448),
            (288, 216, 2, 352, 24, 576, 432),
            (64, 32, 14, 192, 16, 896, 448),
            (128, 64, 7, 192, 16, 896, 448),
        ];
        for (sw, sh, scale, x, y, w, h) in cases {
            let layout = compute_scaled_layout(sw, sh).unwrap();
            assert_eq!(
                (layout.scale, layout.x, layout.y, layout.width, layout.height),
                (scale, x, y, w, h)
            );
        }
        assert!(compute_scaled_layout(2000, 1000).is_none());
        assert!(compute_scaled_layout(0, 100).is_none());
    }

    #[test]
    fn converts_colors_with_truncation() {
        assert_eq!(rgb888_to_565(0xff0000), 0xf800);
        assert_eq!(rgb888_to_565(0x00ff00), 0x07e0);
        assert_eq!(rgb888_to_565(0x0000ff), 0x001f);
        assert_eq!(rgb888_to_565(0xffffff), 0xffff);
    }

    #[test]
    fn keeps_a_widget_touch_hold_until_two_seconds() {
        let now = Instant::now();
        let mut started = None;
        assert!(!update_exit_hold(&mut started, true, 20, 20, now));
        assert!(!update_exit_hold(
            &mut started,
            true,
            20,
            20,
            now + Duration::from_millis(1999),
        ));
        assert!(update_exit_hold(
            &mut started,
            true,
            20,
            20,
            now + Duration::from_secs(2),
        ));
        assert!(!update_exit_hold(
            &mut started,
            false,
            20,
            20,
            now + Duration::from_secs(2),
        ));
    }
}
