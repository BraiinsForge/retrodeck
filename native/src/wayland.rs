use crate::input::TouchReport;
use crate::protocol::deck_widget::{deck_widget_manager_v1, deck_widget_surface_v1};
use crate::{canvas, controls, polling};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::os::fd::{AsFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use wayland_client::backend::WaylandError;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    wl_touch,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, delegate_noop};

const BUFFER_COUNT: usize = 3;
const CONFIGURE_TIMEOUT: Duration = Duration::from_secs(2);
const GAME_INSET: usize = 16;
const O_CLOEXEC: i32 = 0o2000000;
pub const WIDGET_HANDOFF_FD: RawFd = 9;
pub const WIDGET_HANDOFF_FD_ENV: &str = "RETRO_DECK_WIDGET_FD";
static SHM_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Mapping {
    pointer: *mut c_void,
    size: usize,
}

impl Mapping {
    fn new(size: usize) -> Result<(Self, OwnedFd), String> {
        let fd = anonymous_file()?;
        ftruncate(&fd, size as u64)
            .map_err(|error| format!("cannot size Wayland shared memory file: {error}"))?;
        let pointer = unsafe {
            mmap(
                ptr::null_mut(),
                size,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED,
                &fd,
                0,
            )
        }
        .map_err(|error| format!("cannot map Wayland frame buffer: {error}"))?;
        Ok((Self { pointer, size }, fd))
    }

    fn pixels(&mut self) -> &mut [u32] {
        unsafe { slice::from_raw_parts_mut(self.pointer.cast(), self.size / 4) }
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        unsafe {
            let _ = munmap(self.pointer, self.size);
        }
    }
}

fn anonymous_file() -> Result<OwnedFd, String> {
    if let Ok(fd) = memfd_create("retro-deck-wayland", MemfdFlags::CLOEXEC) {
        return Ok(fd);
    }
    let directory = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    for _ in 0..100 {
        let sequence = SHM_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(
            "retro-deck-wayland-{}-{sequence}",
            std::process::id()
        ));
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .custom_flags(O_CLOEXEC)
            .open(&path)
        {
            Ok(file) => {
                std::fs::remove_file(&path).map_err(|error| {
                    format!("cannot unlink Wayland shared memory file: {error}")
                })?;
                return Ok(file.into());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!("cannot create Wayland shared memory file: {error}"));
            }
        }
    }
    Err("cannot create a unique Wayland shared memory file".to_owned())
}

struct BufferSlot {
    mapping: Mapping,
    buffer: wl_buffer::WlBuffer,
    busy: bool,
}

#[derive(Default)]
struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    seat: Option<wl_seat::WlSeat>,
    touch: Option<wl_touch::WlTouch>,
    manager: Option<deck_widget_manager_v1::DeckWidgetManagerV1>,
    widget_surface: Option<deck_widget_surface_v1::DeckWidgetSurfaceV1>,
    surface: Option<wl_surface::WlSurface>,
    configured: bool,
    width: u32,
    height: u32,
    visible: bool,
    refresh_requested: bool,
    shutdown: bool,
    touch_x: i32,
    touch_y: i32,
    touch_down: bool,
    touches: VecDeque<TouchReport>,
    slots: Vec<BufferSlot>,
}

impl State {
    fn push_touch(&mut self, pressed: bool, released: bool) {
        let max_x = self.width.saturating_sub(1) as i32;
        let max_y = self.height.saturating_sub(1) as i32;
        self.touches.push_back(TouchReport {
            x: self.touch_x.clamp(0, max_x),
            y: self.touch_y.clamp(0, max_y),
            down: self.touch_down,
            pressed,
            released,
        });
    }
}

struct Widget {
    queue: EventQueue<State>,
    state: State,
}

fn display_socket_path(display: &Path, runtime_dir: Option<&Path>) -> Result<PathBuf, String> {
    if display.as_os_str().is_empty() {
        return Err("Wayland display name is empty".to_owned());
    }
    if display.is_absolute() {
        return Ok(display.to_path_buf());
    }
    let runtime_dir = runtime_dir
        .filter(|path| path.is_absolute())
        .ok_or_else(|| "XDG_RUNTIME_DIR is unavailable or not absolute".to_owned())?;
    Ok(runtime_dir.join(display))
}

fn connect_to_display(display: &Path) -> Result<Connection, String> {
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let socket_path = display_socket_path(display, runtime_dir.as_deref())?;
    let stream = UnixStream::connect(socket_path)
        .map_err(|error| format!("cannot connect to the Wayland display: {error}"))?;
    Connection::from_socket(stream)
        .map_err(|error| format!("cannot connect to the Wayland display: {error}"))
}

fn initialize_connection(connection: Connection) -> Result<(EventQueue<State>, State), String> {
    let mut queue = connection.new_event_queue::<State>();
    let qh = queue.handle();
    connection.display().get_registry(&qh, ());
    let mut state = State {
        visible: true,
        ..State::default()
    };
    queue
        .roundtrip(&mut state)
        .map_err(|error| format!("cannot bind Wayland globals: {error}"))?;
    if state.compositor.is_none() || state.shm.is_none() {
        return Err("Wayland compositor globals are unavailable".to_owned());
    }
    Ok((queue, state))
}

fn connect(display: Option<&Path>) -> Result<(EventQueue<State>, State), String> {
    let connection = match display {
        Some(display) => connect_to_display(display),
        None => Connection::connect_to_env()
            .map_err(|error| format!("cannot connect to the Wayland display: {error}")),
    }?;
    initialize_connection(connection)
}

fn connect_from_fd(wayland_fd: OwnedFd) -> Result<(EventQueue<State>, State), String> {
    let stream = unsafe { UnixStream::from_raw_fd(wayland_fd.into_raw_fd()) };
    let connection = Connection::from_socket(stream)
        .map_err(|error| format!("cannot connect to the inherited Wayland display: {error}"))?;
    initialize_connection(connection)
}

fn take_widget_handoff() -> Result<Option<OwnedFd>, String> {
    let Some(value) = std::env::var_os(WIDGET_HANDOFF_FD_ENV) else {
        return Ok(None);
    };
    let descriptor = value
        .to_str()
        .and_then(|value| value.parse::<RawFd>().ok())
        .filter(|descriptor| *descriptor == WIDGET_HANDOFF_FD)
        .ok_or_else(|| "inherited BMC widget descriptor is invalid".to_owned())?;
    Ok(Some(unsafe { OwnedFd::from_raw_fd(descriptor) }))
}

/// Open a connection from the registered widget process for a child renderer.
/// BMC retains the process credentials of this connection after it is passed on.
pub fn connect_child_widget() -> Result<OwnedFd, String> {
    let display = std::env::var_os("WAYLAND_DISPLAY")
        .filter(|display| !display.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "WAYLAND_DISPLAY is unavailable for BMC widget handoff".to_owned())?;
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let socket_path = display_socket_path(&display, runtime_dir.as_deref())?;
    let stream = UnixStream::connect(socket_path)
        .map_err(|error| format!("cannot open BMC widget handoff: {error}"))?;
    Ok(unsafe { OwnedFd::from_raw_fd(stream.into_raw_fd()) })
}

impl Widget {
    fn open(display: Option<&Path>) -> Result<Self, String> {
        let (queue, state) = connect(display)?;
        Self::finish_open(queue, state)
    }

    fn open_from_fd(wayland_fd: OwnedFd) -> Result<Self, String> {
        let (queue, state) = connect_from_fd(wayland_fd)?;
        Self::finish_open(queue, state)
    }

    fn finish_open(queue: EventQueue<State>, mut state: State) -> Result<Self, String> {
        let qh = queue.handle();
        let compositor = state.compositor.clone().unwrap();
        let manager = state
            .manager
            .clone()
            .ok_or_else(|| "Deck widget protocol is unavailable".to_owned())?;
        let surface = compositor.create_surface(&qh, ());
        state.widget_surface = Some(manager.get_widget_surface(&surface, &qh, ()));
        state.surface = Some(surface.clone());
        surface.commit();
        let mut widget = Self { queue, state };
        widget.wait_until_configured()?;
        Ok(widget)
    }

    fn wait_until_configured(&mut self) -> Result<(), String> {
        let deadline = Instant::now() + CONFIGURE_TIMEOUT;
        while !self.state.configured && !self.state.shutdown {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() || self.dispatch(duration_ms(remaining))? == 0 {
                return Err("timed out awaiting Wayland surface configure".to_owned());
            }
        }
        if self.state.shutdown {
            Err("Wayland surface was closed during configure".to_owned())
        } else {
            Ok(())
        }
    }

    fn dispatch(&mut self, timeout_ms: u32) -> Result<usize, String> {
        let dispatched = self
            .queue
            .dispatch_pending(&mut self.state)
            .map_err(|error| format!("cannot dispatch Wayland events: {error}"))?;
        if dispatched > 0 {
            return Ok(dispatched);
        }
        self.flush("cannot flush Wayland display")?;

        let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
        loop {
            let Some(guard) = self.queue.prepare_read() else {
                let dispatched = self
                    .queue
                    .dispatch_pending(&mut self.state)
                    .map_err(|error| format!("cannot dispatch Wayland events: {error}"))?;
                if dispatched > 0 {
                    return Ok(dispatched);
                }
                if Instant::now() >= deadline {
                    return Ok(0);
                }
                continue;
            };
            let remaining = deadline.saturating_duration_since(Instant::now());
            let timeout = Timespec {
                tv_sec: remaining.as_secs() as i64,
                tv_nsec: i64::from(remaining.subsec_nanos()),
            };
            let ready = {
                let mut descriptors = [PollFd::from_borrowed_fd(
                    guard.connection_fd(),
                    PollFlags::IN | PollFlags::ERR,
                )];
                match poll(&mut descriptors, Some(&timeout)) {
                    Ok(ready) => ready,
                    Err(rustix::io::Errno::INTR) if Instant::now() < deadline => continue,
                    Err(rustix::io::Errno::INTR) => return Ok(0),
                    Err(error) => {
                        return Err(format!("cannot poll Wayland display: {error}"));
                    }
                }
            };
            if ready == 0 {
                return Ok(0);
            }
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {
                    return Ok(0);
                }
                Err(error) => return Err(format!("cannot read Wayland events: {error}")),
            }
            return self
                .queue
                .dispatch_pending(&mut self.state)
                .map_err(|error| format!("cannot dispatch Wayland events: {error}"));
        }
    }

    fn dispatch_queued_inputs(
        &mut self,
        controls: &mut controls::Controls,
    ) -> Result<polling::InputDispatch, String> {
        let control_flags = {
            let mut descriptors = Vec::new();
            controls.append_poll_descriptors(&mut descriptors);
            let _ = polling::wait(&mut descriptors, 0)?;
            descriptors.iter().map(PollFd::revents).collect::<Vec<_>>()
        };
        controls.read_ready(&control_flags);
        Ok(self.input_dispatch(controls, true, false))
    }

    fn dispatch_inputs(
        &mut self,
        controls: &mut controls::Controls,
        timeout_ms: u32,
    ) -> Result<polling::InputDispatch, String> {
        let dispatched = self
            .queue
            .dispatch_pending(&mut self.state)
            .map_err(|error| format!("cannot dispatch Wayland events: {error}"))?;
        if dispatched > 0 || self.state.shutdown || !self.state.touches.is_empty() {
            return self.dispatch_queued_inputs(controls);
        }
        self.flush("cannot flush Wayland display")?;

        let effective_timeout = if controls.report_count() > 0 {
            0
        } else {
            timeout_ms
        };
        let deadline = Instant::now() + Duration::from_millis(u64::from(effective_timeout));
        loop {
            let Some(guard) = self.queue.prepare_read() else {
                let dispatched = self
                    .queue
                    .dispatch_pending(&mut self.state)
                    .map_err(|error| format!("cannot dispatch Wayland events: {error}"))?;
                if dispatched > 0 || self.state.shutdown || !self.state.touches.is_empty() {
                    return self.dispatch_queued_inputs(controls);
                }
                if Instant::now() >= deadline {
                    return Ok(self.input_dispatch(controls, false, false));
                }
                continue;
            };
            let remaining = deadline.saturating_duration_since(Instant::now());
            let (touch_flags, ready) = {
                let mut descriptors = vec![PollFd::from_borrowed_fd(
                    guard.connection_fd(),
                    PollFlags::IN | PollFlags::ERR,
                )];
                controls.append_poll_descriptors(&mut descriptors);
                let _ = polling::wait_for(&mut descriptors, remaining)?;
                let ready = descriptors.iter().map(PollFd::revents).collect::<Vec<_>>();
                (ready[0], ready)
            };

            controls.read_ready(&ready[1..]);
            let touch_ready = touch_flags
                .intersects(PollFlags::IN | PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL);
            if !touch_ready {
                drop(guard);
                return Ok(self.input_dispatch(controls, controls.report_count() > 0, false));
            }
            if !touch_flags.contains(PollFlags::IN) {
                drop(guard);
                eprintln!("retrodeck: Wayland display disconnected");
                return Ok(self.input_dispatch(controls, true, true));
            }

            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {
                    return Ok(self.input_dispatch(controls, true, false));
                }
                Err(error) => {
                    eprintln!("retrodeck: cannot read Wayland events: {error}");
                    return Ok(self.input_dispatch(controls, true, true));
                }
            }
            let touch_lost = match self.queue.dispatch_pending(&mut self.state) {
                Ok(_) => false,
                Err(error) => {
                    eprintln!("retrodeck: cannot dispatch Wayland events: {error}");
                    true
                }
            };
            return Ok(self.input_dispatch(controls, true, touch_lost));
        }
    }

    fn input_dispatch(
        &mut self,
        controls: &controls::Controls,
        ready: bool,
        touch_lost: bool,
    ) -> polling::InputDispatch {
        let refresh = std::mem::take(&mut self.state.refresh_requested);
        polling::InputDispatch {
            ready: ready
                || refresh
                || controls.report_count() > 0
                || !self.state.touches.is_empty(),
            control_count: controls.report_count(),
            touch_count: self.state.touches.len(),
            touch_lost,
            rescan: controls.rescan_requested(),
            shutdown: self.state.shutdown,
            refresh,
        }
    }

    fn flush(&self, context: &str) -> Result<(), String> {
        loop {
            match self.queue.flush() {
                Ok(()) => return Ok(()),
                Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {
                    return Ok(());
                }
                Err(WaylandError::Io(error)) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(format!("{context}: {error}")),
            }
        }
    }

    fn ensure_slots(&mut self) -> Result<(), String> {
        let width = self.state.width;
        let height = self.state.height;
        let size = frame_size(width, height)?;
        if self
            .state
            .slots
            .first()
            .is_some_and(|slot| slot.mapping.size == size)
        {
            return Ok(());
        }
        if self.state.slots.iter().any(|slot| slot.busy) {
            return Err("Wayland buffer size changed while buffers are in use".to_owned());
        }
        for slot in self.state.slots.drain(..) {
            slot.buffer.destroy();
        }

        let shm = self
            .state
            .shm
            .clone()
            .expect("checked while opening widget");
        let qh = self.queue.handle();
        let mut slots: Vec<BufferSlot> = Vec::with_capacity(BUFFER_COUNT);
        for index in 0..BUFFER_COUNT {
            let (mapping, buffer) = match create_buffer(&shm, &qh, width, height, index) {
                Ok(result) => result,
                Err(error) => {
                    for slot in slots {
                        slot.buffer.destroy();
                    }
                    return Err(error);
                }
            };
            slots.push(BufferSlot {
                mapping,
                buffer,
                busy: false,
            });
        }
        self.state.slots = slots;
        Ok(())
    }

    fn present_solid(&mut self, color: u32) -> Result<(), String> {
        self.present_frame(|pixels| pixels.fill(0xff00_0000 | (color & 0x00ff_ffff)))
    }

    fn present_rgba(&mut self, rgba: &[u8]) -> Result<(), String> {
        if self.state.width != canvas::WIDTH
            || self.state.height != canvas::HEIGHT
            || rgba.len() != canvas::WIDTH as usize * canvas::HEIGHT as usize * 4
        {
            return Err("Wayland surface does not match the native canvas".to_owned());
        }
        self.present_frame(|pixels| copy_rgba_to_xrgb(rgba, pixels))
    }

    fn present_rgb565(&mut self, rgb565: &[u16]) -> Result<(), String> {
        if self.state.width != canvas::WIDTH
            || self.state.height != canvas::HEIGHT
            || rgb565.len() != canvas::WIDTH as usize * canvas::HEIGHT as usize
        {
            return Err("Wayland surface does not match the RGB565 frame".to_owned());
        }
        self.present_frame(|pixels| copy_rgb565_to_xrgb(rgb565, pixels))
    }

    fn present_frame(&mut self, draw: impl FnOnce(&mut [u32])) -> Result<(), String> {
        if !self.state.configured {
            return Err("Wayland surface is not configured".to_owned());
        }
        self.ensure_slots()?;
        let _ = self.dispatch(0)?;
        let Some(index) = self.state.slots.iter().position(|slot| !slot.busy) else {
            return Ok(());
        };
        let surface = self
            .state
            .surface
            .as_ref()
            .expect("created while opening widget");
        let slot = &mut self.state.slots[index];
        draw(slot.mapping.pixels());
        slot.busy = true;
        surface.attach(Some(&slot.buffer), 0, 0);
        surface.damage(0, 0, self.state.width as i32, self.state.height as i32);
        surface.commit();
        if let Err(error) = self.flush("cannot flush Wayland frame") {
            self.state.slots[index].busy = false;
            return Err(error);
        }
        Ok(())
    }
}

thread_local! {
    static WIDGET: RefCell<Option<Widget>> = const { RefCell::new(None) };
}

pub fn open_widget() -> Result<(), String> {
    open_widget_for(None)
}

pub fn open_widget_at(display: &Path) -> Result<(), String> {
    open_widget_for(Some(display))
}

/// Open the game's normal BMC widget surface through the descriptor handed
/// down by the registered dashboard process.
pub fn open_game_widget() -> Result<(), String> {
    close();
    let handoff = take_widget_handoff()?
        .ok_or_else(|| "BMC widget handoff is unavailable for gameplay".to_owned())?;
    let widget = Widget::open_from_fd(handoff)?;
    WIDGET.with(|current| *current.borrow_mut() = Some(widget));
    Ok(())
}

pub fn present_game_xrgb(
    frame: &[u32],
    source_width: usize,
    source_height: usize,
) -> Result<(), String> {
    with_widget(|widget| {
        let width = widget.state.width as usize;
        let height = widget.state.height as usize;
        widget.present_frame(|pixels| {
            fill_gameplay_background(
                pixels,
                std::env::var_os("RETRO_DECK_EXIT_HINT").is_some(),
            );
            scale_game_frame(frame, source_width, source_height, pixels, width, height);
        })
    })
}

fn scale_game_frame(
    frame: &[u32],
    source_width: usize,
    source_height: usize,
    pixels: &mut [u32],
    target_width: usize,
    target_height: usize,
) {
    if pixels.len() < target_width * target_height
        || frame.len() < source_width * source_height
        || source_width == 0
        || source_height == 0
        || target_width <= GAME_INSET * 2
        || target_height <= GAME_INSET * 2
    {
        return;
    }
    let scale = ((target_width - GAME_INSET * 2) / source_width)
        .min((target_height - GAME_INSET * 2) / source_height)
        .max(1);
    let width = source_width * scale;
    let height = source_height * scale;
    if width > target_width || height > target_height {
        return;
    }
    let offset_x = (target_width - width) / 2;
    let offset_y = (target_height - height) / 2;
    for source_y in 0..source_height {
        let first_row = (offset_y + source_y * scale) * target_width + offset_x;
        for source_x in 0..source_width {
            let value = frame[source_y * source_width + source_x];
            let start = first_row + source_x * scale;
            pixels[start..start + scale].fill(value);
        }
        for duplicate in 1..scale {
            let row = first_row + duplicate * target_width;
            let (head, tail) = pixels.split_at_mut(row);
            tail[..width].copy_from_slice(&head[first_row..first_row + width]);
        }
    }
}

fn open_widget_for(display: Option<&Path>) -> Result<(), String> {
    close();
    let widget = match take_widget_handoff()? {
        Some(handoff) => Widget::open_from_fd(handoff)?,
        None => Widget::open(display)?,
    };
    WIDGET.with(|current| *current.borrow_mut() = Some(widget));
    Ok(())
}

pub fn close() {
    WIDGET.with(|current| {
        current.borrow_mut().take();
    });
}

pub fn dispatch(timeout_ms: u32) -> Result<usize, String> {
    with_widget(|widget| widget.dispatch(timeout_ms))
}

pub(crate) fn dispatch_inputs(timeout_ms: u32) -> Result<polling::InputDispatch, String> {
    controls::with_controls(|controls| {
        with_widget(|widget| widget.dispatch_inputs(controls, timeout_ms))
    })
}

pub fn present_solid(color: u32) -> Result<(), String> {
    with_widget(|widget| widget.present_solid(color))
}

pub fn present_rgba(rgba: &[u8]) -> Result<(), String> {
    with_widget(|widget| widget.present_rgba(rgba))
}

pub fn present_rgb565(rgb565: &[u16]) -> Result<(), String> {
    with_widget(|widget| widget.present_rgb565(rgb565))
}

pub fn next_touch() -> Option<TouchReport> {
    WIDGET.with(|current| {
        current
            .borrow_mut()
            .as_mut()
            .and_then(|widget| widget.state.touches.pop_front())
    })
}

pub fn size() -> Option<(u32, u32)> {
    WIDGET.with(|current| {
        current
            .borrow()
            .as_ref()
            .map(|widget| (widget.state.width, widget.state.height))
    })
}

pub fn visible() -> bool {
    WIDGET.with(|current| {
        current
            .borrow()
            .as_ref()
            .is_some_and(|widget| widget.state.visible)
    })
}

pub fn shutdown_requested() -> bool {
    WIDGET.with(|current| {
        current
            .borrow()
            .as_ref()
            .is_some_and(|widget| widget.state.shutdown)
    })
}

fn with_widget<T>(function: impl FnOnce(&mut Widget) -> Result<T, String>) -> Result<T, String> {
    WIDGET.with(|current| {
        let mut current = current.borrow_mut();
        let widget = current
            .as_mut()
            .ok_or_else(|| "Wayland widget is not open".to_owned())?;
        function(widget)
    })
}

fn duration_ms(duration: Duration) -> u32 {
    duration.as_millis().clamp(1, u128::from(u32::MAX)) as u32
}

fn frame_size(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 || width > i32::MAX as u32 / 4 || height > i32::MAX as u32 {
        return Err("Wayland buffer dimensions are invalid".to_owned());
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|size| *size <= i32::MAX as usize)
        .ok_or_else(|| "Wayland buffer dimensions are invalid".to_owned())
}

fn create_buffer(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<State>,
    width: u32,
    height: u32,
    data: usize,
) -> Result<(Mapping, wl_buffer::WlBuffer), String> {
    let size = frame_size(width, height)?;
    let (mapping, fd) = Mapping::new(size)?;
    let pool = shm.create_pool(fd.as_fd(), size as i32, qh, ());
    let buffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        (width * 4) as i32,
        wl_shm::Format::Xrgb8888,
        qh,
        data,
    );
    pool.destroy();
    Ok((mapping, buffer))
}

fn fill_gameplay_background(pixels: &mut [u32], exit_hint: bool) {
    pixels.fill(0xff00_0000);
    if exit_hint {
        for step in 0..9 {
            for y in 0..4 {
                for x in 0..4 {
                    let row = (20 + step * 4 + y) * canvas::WIDTH as usize;
                    pixels[row + 20 + step * 4 + x] = 0xffff_ffff;
                    pixels[row + 20 + (8 - step) * 4 + x] = 0xffff_ffff;
                }
            }
        }
    }
}

fn rgba_to_xrgb(color: &[u8]) -> u32 {
    let red = u32::from(color[0] >> 3);
    let green = u32::from(color[1] >> 2);
    let blue = u32::from(color[2] >> 3);
    0xff00_0000 | ((red * 255 / 31) << 16) | ((green * 255 / 63) << 8) | (blue * 255 / 31)
}

fn copy_rgba_to_xrgb(rgba: &[u8], pixels: &mut [u32]) {
    debug_assert_eq!(rgba.len(), pixels.len() * 4);
    for (pixel, color) in pixels.iter_mut().zip(rgba.chunks_exact(4)) {
        *pixel = rgba_to_xrgb(color);
    }
}

fn copy_rgb565_to_xrgb(rgb565: &[u16], pixels: &mut [u32]) {
    debug_assert_eq!(rgb565.len(), pixels.len());
    for (destination, source) in pixels.iter_mut().zip(rgb565) {
        let red = u32::from((source >> 11) & 0x1f);
        let green = u32::from((source >> 5) & 0x3f);
        let blue = u32::from(source & 0x1f);
        *destination =
            0xff00_0000 | ((red * 255 / 31) << 16) | ((green * 255 / 63) << 8) | (blue * 255 / 31);
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" if state.compositor.is_none() => {
                    state.compositor = Some(registry.bind(name, version.min(4), qh, ()));
                }
                "wl_shm" if state.shm.is_none() => {
                    state.shm = Some(registry.bind(name, 1, qh, ()));
                }
                "wl_seat" if state.seat.is_none() => {
                    state.seat = Some(registry.bind(name, version.min(7), qh, ()));
                }
                "deck_widget_manager_v1" if state.manager.is_none() => {
                    state.manager = Some(registry.bind(name, 1, qh, ()));
                }
                _ => {}
            }
        }
    }
}

delegate_noop!(State: ignore wl_compositor::WlCompositor);
delegate_noop!(State: ignore wl_surface::WlSurface);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore deck_widget_manager_v1::DeckWidgetManagerV1);

impl Dispatch<wl_buffer::WlBuffer, usize> for State {
    fn event(
        state: &mut Self,
        _: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            if let Some(slot) = state.slots.get_mut(*index) {
                slot.busy = false;
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            let have_touch = capabilities.contains(wl_seat::Capability::Touch);
            if have_touch && state.touch.is_none() {
                state.touch = Some(seat.get_touch(qh, ()));
            } else if !have_touch {
                if let Some(touch) = state.touch.take() {
                    if touch.version() >= 3 {
                        touch.release();
                    } else if let Some(backend) = touch.backend().upgrade() {
                        let _ = backend.destroy_object(&touch.id());
                    }
                }
                state.touch_down = false;
            }
        }
    }
}

impl Dispatch<wl_touch::WlTouch, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_touch::WlTouch,
        event: wl_touch::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_touch::Event::Down { surface, x, y, .. } => {
                if state.surface.as_ref() == Some(&surface) {
                    state.touch_x = x as i32;
                    state.touch_y = y as i32;
                    state.touch_down = true;
                    state.push_touch(true, false);
                }
            }
            wl_touch::Event::Up { .. } if state.touch_down => {
                state.touch_down = false;
                state.push_touch(false, true);
            }
            wl_touch::Event::Motion { x, y, .. } if state.touch_down => {
                state.touch_x = x as i32;
                state.touch_y = y as i32;
                state.push_touch(false, false);
            }
            wl_touch::Event::Cancel if state.touch_down => {
                state.touch_down = false;
                state.touches.push_back(TouchReport {
                    x: -1,
                    y: -1,
                    down: false,
                    pressed: false,
                    released: true,
                });
            }
            _ => {}
        }
    }
}

impl Dispatch<deck_widget_surface_v1::DeckWidgetSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &deck_widget_surface_v1::DeckWidgetSurfaceV1,
        event: deck_widget_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            deck_widget_surface_v1::Event::Configure { width, height, .. } => {
                state.width = width;
                state.height = height;
            }
            deck_widget_surface_v1::Event::ConfigureDone => state.configured = true,
            deck_widget_surface_v1::Event::Lifecycle { state: lifecycle } => {
                state.visible = lifecycle != 0;
                // Leaving dormancy reclaims the widget's render target;
                // without a fresh commit the scene shows black.
                if lifecycle != 0 {
                    state.refresh_requested = true;
                }
            }
            deck_widget_surface_v1::Event::TransitionIncoming => {
                state.refresh_requested = true;
            }
            deck_widget_surface_v1::Event::Shutdown => state.shutdown = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_explicit_display_paths() {
        assert_eq!(
            display_socket_path(Path::new("/run/wayland-7"), None).unwrap(),
            PathBuf::from("/run/wayland-7")
        );
        assert_eq!(
            display_socket_path(Path::new("wayland-7"), Some(Path::new("/run/user/1000"))).unwrap(),
            PathBuf::from("/run/user/1000/wayland-7")
        );
        assert!(display_socket_path(Path::new(""), Some(Path::new("/run"))).is_err());
        assert!(display_socket_path(Path::new("wayland-7"), None).is_err());
        assert!(
            display_socket_path(Path::new("wayland-7"), Some(Path::new("run/user/1000"))).is_err()
        );
    }

    #[test]
    fn validates_frame_geometry() {
        assert_eq!(frame_size(1280, 480).unwrap(), 2_457_600);
        assert!(frame_size(0, 480).is_err());
        assert!(frame_size(i32::MAX as u32, 2).is_err());
        let rgba = [0xfe, 0x6c, 0x27, 0xff, 0xec, 0xb6, 0xe7, 0xff];
        let mut pixels = [0; 2];
        copy_rgba_to_xrgb(&rgba, &mut pixels);
        assert_eq!(pixels, [0xffff_6d20, 0xffee_b6e6]);
        copy_rgb565_to_xrgb(&[0xfb64, 0xffff], &mut pixels);
        assert_eq!(pixels, [0xffff_6d20, 0xffff_ffff]);
        let mut background = vec![0; canvas::WIDTH as usize * canvas::HEIGHT as usize];
        fill_gameplay_background(&mut background, true);
        assert_eq!(background[20 * canvas::WIDTH as usize + 20], 0xffff_ffff);
        assert_eq!(background[20 * canvas::WIDTH as usize + 40], 0xff00_0000);
    }

    #[test]
    fn touch_reports_match_dashboard_clamping() {
        let mut state = State {
            width: 1280,
            height: 480,
            touch_x: 1300,
            touch_y: -3,
            touch_down: true,
            ..State::default()
        };
        state.push_touch(true, false);
        assert_eq!(
            state.touches.pop_front(),
            Some(TouchReport {
                x: 1279,
                y: 0,
                down: true,
                pressed: true,
                released: false,
            })
        );
    }
}
