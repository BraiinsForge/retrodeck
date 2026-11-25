//! Emulator video via the keyed BMC widget surface, with a headless test target.

use std::ffi::c_void;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const LOGICAL_WIDTH: usize = 1280;
const LOGICAL_HEIGHT: usize = 480;
enum Target {
    Widget {
        converted: Vec<u32>,
        width: usize,
        height: usize,
    },
    Headless {
        frame: Vec<u16>,
        hash: u64,
    },
}

struct Video {
    target: Target,
    exit_hint: bool,
    exit_hold_started: Option<Instant>,
    focus: WidgetFocus,
}

static VIDEO: Mutex<Option<Video>> = Mutex::new(None);

fn rgb888_to_565(color: u32) -> u16 {
    let red = (color >> 16) & 0xff;
    let green = (color >> 8) & 0xff;
    let blue = color & 0xff;
    (((red & 0xf8) << 8) | ((green & 0xfc) << 3) | (blue >> 3)) as u16
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
                0xff00_0000
                    | ((red * 255 / 31) << 16)
                    | ((green * 255 / 63) << 8)
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
    } else {
        crate::wayland::open_game_widget()?;
        Target::Widget {
            converted: Vec::new(),
            width: 0,
            height: 0,
        }
    };
    *VIDEO.lock().expect("video lock") = Some(Video {
        target,
        exit_hint,
        exit_hold_started: None,
        focus: WidgetFocus::default(),
    });
    Ok(())
}

/// Present a frame synchronously without retaining its source buffer.
///
/// # Safety
/// `data` must point to initialized, readable memory covering every row's
/// `width` pixels at the supplied byte `pitch`, for the duration of this call.
/// The buffer must not be mutated concurrently. Pixels need not be aligned.
pub unsafe fn present(
    data: *const c_void,
    width: u32,
    height: u32,
    pitch: usize,
    rgb565: bool,
) -> Result<(), String> {
    let width = width as usize;
    let height = height as usize;
    let minimum_pitch = width
        .checked_mul(if rgb565 { 2 } else { 4 })
        .ok_or_else(|| "video frame dimensions overflow".to_owned())?;
    if pitch < minimum_pitch {
        return Err("video frame pitch is too small".to_owned());
    }
    let bytes = height
        .saturating_sub(1)
        .checked_mul(pitch)
        .and_then(|offset| offset.checked_add(minimum_pitch))
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or_else(|| "video frame dimensions overflow".to_owned())?;
    if bytes != 0 && data.is_null() {
        return Err("video frame data is null".to_owned());
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
        Target::Widget {
            converted,
            width: last_width,
            height: last_height,
        } => {
            *last_width = width;
            *last_height = height;
            convert_to_xrgb(data, width, height, pitch, rgb565, converted);
            crate::wayland::present_game_xrgb(converted, width, height)
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
    let inside = down && x >= 0 && x < LOGICAL_WIDTH as i32 && y >= 0 && y < LOGICAL_HEIGHT as i32;
    if !inside {
        *started = None;
        return false;
    }
    let began = started.get_or_insert(now);
    now.duration_since(*began) >= Duration::from_secs(2)
}

/// Wait before advancing the game. Returns false on shutdown or the return hold,
/// and propagates a lost connection as an error. The game owns the touch stream.
/// `on_resume` receives the time spent waiting for Visible.
pub fn before_frame(on_resume: impl FnOnce(Duration)) -> Result<bool, String> {
    if crate::process::shutdown_requested() {
        return Ok(false);
    }
    let mut guard = VIDEO.lock().expect("video lock");
    let Some(video) = guard.as_mut() else {
        return Ok(true);
    };
    let Target::Widget {
        converted,
        width,
        height,
    } = &video.target
    else {
        return Ok(true);
    };
    match video
        .focus
        .wait(|| crate::wayland::present_game_xrgb(converted, *width, *height))?
    {
        Focus::Shutdown => return Ok(false),
        Focus::Resumed(paused) => {
            video.exit_hold_started = None;
            on_resume(paused);
        }
        Focus::Continuous => {}
    }
    if !video.exit_hint {
        while crate::wayland::next_touch().is_some() {}
        return Ok(true);
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
    Ok(!(complete
        || video
            .exit_hold_started
            .is_some_and(|started| now.duration_since(started) >= Duration::from_secs(2))))
}

pub enum Focus {
    Continuous,
    Resumed(Duration),
    Shutdown,
}

/// One focus gate per game surface. While paused, only Wayland is serviced.
#[derive(Default)]
pub struct WidgetFocus {
    focus_loss_count: u64,
    started: bool,
}

impl WidgetFocus {
    pub fn wait(
        &mut self,
        mut preview: impl FnMut() -> Result<(), String>,
    ) -> Result<Focus, String> {
        let started = Instant::now();
        crate::wayland::dispatch(0)?;
        let (initially_focused, losses) = crate::wayland::game_focus();
        let paused = !initially_focused || self.focus_loss_count != losses;
        let resumed = !self.started || paused;
        if paused {
            crate::canvas::discard_pixels();
            crate::game_audio::discard_pending();
            crate::wayland::discard_game_touches();
        }
        loop {
            if crate::wayland::shutdown_requested() || crate::process::shutdown_requested() {
                return Ok(Focus::Shutdown);
            }
            let (focused, losses) = crate::wayland::game_focus();
            if focused {
                self.focus_loss_count = losses;
                self.started = true;
                return Ok(if resumed {
                    if !initially_focused {
                        crate::wayland::discard_game_touches();
                    }
                    Focus::Resumed(started.elapsed())
                } else {
                    Focus::Continuous
                });
            }
            if crate::wayland::needs_preview() {
                preview()?;
                crate::canvas::discard_pixels();
            }
            crate::wayland::dispatch(100)?;
        }
    }
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

#[allow(clippy::unnecessary_cast, reason = "timespec fields are 32-bit on ARM")]
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

    pub fn reset(&mut self) {
        self.start_nanoseconds = monotonic_now();
        self.frame_number = 0;
    }
    /// Nanoseconds the loop is running behind the next frame deadline;
    /// zero or negative when on schedule.
    pub fn lateness(&self) -> i64 {
        if self.frame_nanoseconds <= 0 {
            return 0;
        }
        let deadline = self.start_nanoseconds + (self.frame_number + 1) * self.frame_nanoseconds;
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
