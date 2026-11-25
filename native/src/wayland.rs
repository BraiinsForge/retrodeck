use crate::protocol::deck_widget::{deck_widget_manager_v3, deck_widget_surface_v2};
use crate::{canvas, controls, polling};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{OsStr, c_void};
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use wayland_client::backend::WaylandError;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface, wl_touch,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, delegate_noop};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TouchReport {
    pub x: i32,
    pub y: i32,
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
}
const BUFFER_COUNT: usize = 3;
const CONFIGURE_TIMEOUT: Duration = Duration::from_secs(2);
const GAME_INSET: usize = 16;
const O_CLOEXEC: i32 = 0o2000000;
pub const WIDGET_KEY_ENV: &str = "BMC_WIDGET_KEY";
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

#[derive(Clone, Copy, PartialEq, Eq)]
struct GameLayout {
    target: (usize, usize),
    source: (usize, usize),
    exit_hint: bool,
}

/// Surface-local coordinates; our SHM buffers use unit scale and no transform.
struct DamageRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}
struct BufferSlot {
    mapping: Mapping,
    buffer: wl_buffer::WlBuffer,
    busy: bool,
    game_layout: Option<GameLayout>,
}

#[derive(Default)]
struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    seat: Option<wl_seat::WlSeat>,
    touch: Option<wl_touch::WlTouch>,
    manager: Option<deck_widget_manager_v3::DeckWidgetManagerV3>,
    widget_surface: Option<deck_widget_surface_v2::DeckWidgetSurfaceV2>,
    surface: Option<wl_surface::WlSurface>,
    configured: bool,
    width: u32,
    height: u32,
    lifecycle: Option<deck_widget_surface_v2::LifecycleState>,
    focus_loss_count: u64,
    last_buffer: Option<wl_buffer::WlBuffer>,
    refresh_requested: bool,
    retry_frame: bool,
    shutdown: bool,
    touch_x: i32,
    touch_y: i32,
    touch_down: bool,
    touches: VecDeque<TouchReport>,
    slots: Vec<BufferSlot>,
}

impl State {
    fn focused(&self) -> bool {
        self.lifecycle == Some(deck_widget_surface_v2::LifecycleState::Visible)
    }

    fn needs_preview(&self) -> bool {
        !self.shutdown
            && self.lifecycle == Some(deck_widget_surface_v2::LifecycleState::Prepared)
            && self.last_buffer.is_none()
            && self.slots.is_empty()
    }

    fn renderable(&self) -> bool {
        !self.shutdown && (self.focused() || self.needs_preview())
    }

    fn trim_buffers(&mut self) {
        if self.focused() {
            return;
        }
        // BMC holds the latest submission until it is replaced or becomes dormant.
        // Keep that frame for the swipe, and don't touch any buffer still in use.
        self.slots.retain(|slot| {
            if slot.busy || self.last_buffer.as_ref() == Some(&slot.buffer) {
                true
            } else {
                slot.buffer.destroy();
                false
            }
        });
    }
    fn widget_event(&mut self, event: deck_widget_surface_v2::Event) {
        use deck_widget_surface_v2::{Event, LifecycleState};
        match event {
            Event::Configure { width, height, .. } => {
                self.width = width;
                self.height = height;
            }
            Event::ConfigureDone => self.configured = frame_size(self.width, self.height).is_ok(),
            Event::Params { fd, .. }
            | Event::Credentials { fd, .. }
            | Event::CredentialSecrets { fd, .. } => {
                // No params or credential slots are declared in our manifest.
                // Even unused payloads own an FD, including at runtime.
                drop(fd);
            }
            Event::Lifecycle { state } => {
                let was_focused = self.focused();
                self.lifecycle = match state {
                    WEnum::Value(state) => Some(state),
                    _ => None,
                };
                if !self.focused() {
                    self.focus_loss_count += u64::from(was_focused);
                    self.touch_down = false;
                    self.touches.clear();
                }
                if matches!(self.lifecycle, None | Some(LifecycleState::Dormant))
                    && self.last_buffer.take().is_some()
                    && let Some(surface) = &self.surface
                {
                    // A frame may have crossed the compositor's Dormant event in
                    // flight. Detach it so we get a release even in that case.
                    surface.attach(None, 0, 0);
                    surface.commit();
                }
                self.trim_buffers();
                self.retry_frame = false;
                self.refresh_requested = self.renderable();
            }
            Event::TransitionIncoming => self.refresh_requested |= self.renderable(),
            Event::Shutdown => self.shutdown = true,
            _ => {}
        }
    }

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

fn widget_instance_key(value: Option<&OsStr>) -> Result<String, String> {
    let value = value
        .and_then(OsStr::to_str)
        .ok_or_else(|| "BMC_WIDGET_KEY is unavailable or is not UTF-8".to_owned())?;
    let canonical = value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        });
    if !canonical {
        return Err("BMC_WIDGET_KEY must be a canonical configured-widget UUID".to_owned());
    }
    Ok(value.to_owned())
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
    let mut state = State::default();
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

impl Widget {
    fn open(display: Option<&Path>) -> Result<Self, String> {
        let key = widget_instance_key(std::env::var_os(WIDGET_KEY_ENV).as_deref())?;
        let (queue, state) = connect(display)?;
        Self::finish_open(queue, state, &key)
    }

    fn finish_open(queue: EventQueue<State>, mut state: State, key: &str) -> Result<Self, String> {
        let qh = queue.handle();
        let compositor = state.compositor.clone().unwrap();
        let manager = state
            .manager
            .clone()
            .ok_or_else(|| "Deck widget protocol is unavailable".to_owned())?;
        let surface = compositor.create_surface(&qh, ());
        state.widget_surface = Some(manager.get_widget_surface(key.to_owned(), &surface, &qh, ()));
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

    fn dispatch_pending(&mut self) -> Result<usize, String> {
        let dispatched = self
            .queue
            .dispatch_pending(&mut self.state)
            .map_err(|error| format!("cannot dispatch Wayland events: {error}"))?;
        if dispatched > 0 {
            self.flush("cannot flush Wayland display")?;
        }
        Ok(dispatched)
    }

    fn dispatch(&mut self, timeout_ms: u32) -> Result<usize, String> {
        let dispatched = self.dispatch_pending()?;
        if dispatched > 0 {
            return Ok(dispatched);
        }
        self.flush("cannot flush Wayland display")?;

        let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
        loop {
            let Some(guard) = self.queue.prepare_read() else {
                let dispatched = self.dispatch_pending()?;
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
            return self.dispatch_pending();
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
        let dispatched = self.dispatch_pending()?;
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
                let dispatched = self.dispatch_pending()?;
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
            let touch_lost = match self.dispatch_pending() {
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

    fn available_slot(&mut self) -> Result<Option<usize>, String> {
        let (width, height) = (self.state.width, self.state.height);
        let size = frame_size(width, height)?;
        self.state.slots.retain(|slot| {
            if !slot.busy && slot.mapping.size != size {
                slot.buffer.destroy();
                false
            } else {
                true
            }
        });
        if let Some(index) = self.state.slots.iter().position(|slot| !slot.busy) {
            return Ok(Some(index));
        }
        let limit = if self.state.focused() {
            BUFFER_COUNT
        } else {
            1
        };
        if self.state.slots.len() >= limit {
            return Ok(None);
        }
        let shm = self
            .state
            .shm
            .as_ref()
            .expect("checked while opening widget");
        let (mapping, buffer) = create_buffer(shm, &self.queue.handle(), width, height)?;
        let index = self.state.slots.len();
        self.state.slots.push(BufferSlot {
            mapping,
            buffer,
            busy: false,
            game_layout: None,
        });
        Ok(Some(index))
    }

    #[cfg(test)]
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
        self.present_slot(|slot| {
            slot.game_layout = None;
            draw(slot.mapping.pixels());
            None
        })
    }

    fn present_slot(
        &mut self,
        draw: impl FnOnce(&mut BufferSlot) -> Option<DamageRect>,
    ) -> Result<(), String> {
        if !self.state.configured {
            return Err("Wayland surface is not configured".to_owned());
        }
        let _ = self.dispatch(0)?;
        if !self.state.renderable() {
            return Ok(());
        }
        let Some(index) = self.available_slot()? else {
            self.state.retry_frame = true;
            return Ok(());
        };
        let surface = self
            .state
            .surface
            .as_ref()
            .expect("created while opening widget");
        // Damage is relative to the last surface commit, not just this SHM slot.
        let previous_layout = self.state.last_buffer.as_ref().and_then(|last| {
            self.state
                .slots
                .iter()
                .find(|slot| &slot.buffer == last)?
                .game_layout
        });
        let slot = &mut self.state.slots[index];
        let damage = draw(slot)
            .filter(|_| slot.game_layout == previous_layout)
            .unwrap_or(DamageRect {
                x: 0,
                y: 0,
                width: self.state.width as i32,
                height: self.state.height as i32,
            });
        slot.busy = true;
        surface.attach(Some(&slot.buffer), 0, 0);
        surface.damage(damage.x, damage.y, damage.width, damage.height);
        surface.commit();
        if let Err(error) = self.flush("cannot flush Wayland frame") {
            self.state.slots[index].busy = false;
            return Err(error);
        }
        self.state.last_buffer = Some(self.state.slots[index].buffer.clone());
        self.state.retry_frame = false;
        Ok(())
    }
}

thread_local! {
    static WIDGET: RefCell<Option<Widget>> = const { RefCell::new(None) };
}

pub fn open_widget_at(display: &Path) -> Result<(), String> {
    open_widget_for(Some(display))
}

/// Connect the game to the configured BMC widget instance inherited from its parent.
pub fn open_game_widget() -> Result<(), String> {
    open_widget_for(None)
}

pub fn present_game_xrgb(
    frame: &[u32],
    source_width: usize,
    source_height: usize,
) -> Result<(), String> {
    with_widget(|widget| {
        let width = widget.state.width as usize;
        let height = widget.state.height as usize;
        let exit_hint = std::env::var_os("RETRO_DECK_EXIT_HINT").is_some();
        let layout = GameLayout {
            target: (width, height),
            source: (source_width, source_height),
            exit_hint,
        };
        widget.present_slot(|slot| {
            let pixels = slot.mapping.pixels();
            // Repaint the letterbox only when this buffer's layout changes.
            let repaint_background = slot.game_layout != Some(layout);
            if repaint_background {
                fill_gameplay_background(pixels, exit_hint);
                slot.game_layout = Some(layout);
            }
            let viewport =
                scale_game_frame(frame, source_width, source_height, pixels, width, height);
            viewport.filter(|_| !repaint_background)
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
) -> Option<DamageRect> {
    if pixels.len() < target_width * target_height
        || frame.len() < source_width * source_height
        || source_width == 0
        || source_height == 0
        || target_width <= GAME_INSET * 2
        || target_height <= GAME_INSET * 2
    {
        return None;
    }
    let scale = ((target_width - GAME_INSET * 2) / source_width)
        .min((target_height - GAME_INSET * 2) / source_height)
        .max(1);
    let width = source_width * scale;
    let height = source_height * scale;
    if width > target_width || height > target_height {
        return None;
    }
    let offset_x = (target_width - width) / 2;
    let offset_y = (target_height - height) / 2;
    for (source_y, source) in frame
        .chunks_exact(source_width)
        .take(source_height)
        .enumerate()
    {
        let first_row = (offset_y + source_y * scale) * target_width + offset_x;
        let destination = &mut pixels[first_row..first_row + width];
        match scale {
            1 => destination.copy_from_slice(source),
            2 => {
                for (pair, &color) in destination.chunks_exact_mut(2).zip(source) {
                    pair[0] = color;
                    pair[1] = color;
                }
            }
            _ => {
                for (block, &color) in destination.chunks_exact_mut(scale).zip(source) {
                    block.fill(color);
                }
            }
        }
        for duplicate in 1..scale {
            let row = first_row + duplicate * target_width;
            pixels.copy_within(first_row..first_row + width, row);
        }
    }
    Some(DamageRect {
        x: offset_x as i32,
        y: offset_y as i32,
        width: width as i32,
        height: height as i32,
    })
}

fn open_widget_for(display: Option<&Path>) -> Result<(), String> {
    close();
    let widget = Widget::open(display)?;
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

pub fn frame_renderable() -> bool {
    WIDGET.with(|current| {
        current
            .borrow()
            .as_ref()
            .is_some_and(|widget| widget.state.renderable())
    })
}

pub(crate) fn frame_active() -> bool {
    WIDGET.with(|current| {
        current
            .borrow()
            .as_ref()
            .is_some_and(|widget| !widget.state.shutdown && widget.state.focused())
    })
}

pub fn needs_preview() -> bool {
    WIDGET.with(|current| {
        current
            .borrow()
            .as_ref()
            .is_some_and(|widget| widget.state.needs_preview())
    })
}

/// Only Visible can advance emulation. Count losses even if the widget
/// returned to Visible in the same dispatch batch.
pub(crate) fn game_focus() -> (bool, u64) {
    WIDGET.with(|current| {
        current.borrow().as_ref().map_or((false, 0), |widget| {
            (widget.state.focused(), widget.state.focus_loss_count)
        })
    })
}

pub(crate) fn discard_game_touches() {
    WIDGET.with(|current| {
        if let Some(widget) = current.borrow_mut().as_mut() {
            widget.state.touch_down = false;
            widget.state.touches.clear();
        }
    });
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
        (),
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
                "deck_widget_manager_v3" if state.manager.is_none() => {
                    state.manager = Some(registry.bind(name, version.min(2), qh, ()));
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
delegate_noop!(State: ignore deck_widget_manager_v3::DeckWidgetManagerV3);

impl Dispatch<wl_buffer::WlBuffer, ()> for State {
    fn event(
        state: &mut Self,
        buffer: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            if let Some(slot) = state.slots.iter_mut().find(|slot| &slot.buffer == buffer) {
                slot.busy = false;
            }
            state.trim_buffers();
            state.refresh_requested |= state.retry_frame || state.needs_preview();
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

impl Dispatch<deck_widget_surface_v2::DeckWidgetSurfaceV2, ()> for State {
    fn event(
        state: &mut Self,
        _: &deck_widget_surface_v2::DeckWidgetSurfaceV2,
        event: deck_widget_surface_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.widget_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_canonical_configured_widget_keys() {
        use std::os::unix::ffi::OsStrExt;
        let key = "77c5424d-8dc4-4b22-a14b-d9ee90a2af76";
        assert_eq!(widget_instance_key(Some(OsStr::new(key))).unwrap(), key);
        assert!(widget_instance_key(None).is_err());
        for value in [
            "",
            "retro-deck",
            "77C5424D-8DC4-4B22-A14B-D9EE90A2AF76",
            "77c5424d8dc44b22a14bd9ee90a2af76",
            "77c5424d-8dc4-4b22-a14b-d9ee90a2af7g",
        ] {
            assert!(widget_instance_key(Some(OsStr::new(value))).is_err());
        }
        assert!(widget_instance_key(Some(OsStr::from_bytes(&[0xff]))).is_err());
    }

    #[test]
    fn waits_for_the_complete_configuration_batch_and_replays_lifecycle() {
        use deck_widget_surface_v2::{Event, LifecycleState, ViewportShape};
        let mut state = State::default();
        state.widget_event(Event::Configure {
            width: 1280,
            height: 480,
            viewport_shape: WEnum::Value(ViewportShape::Rectangular),
            token: "test-instance".into(),
        });
        assert!(!state.configured);
        state.widget_event(Event::ConfigureDone);
        assert!(state.configured);
        state.widget_event(Event::Lifecycle {
            state: WEnum::Value(LifecycleState::Dormant),
        });
        assert!(!state.renderable());
        state.widget_event(Event::Lifecycle {
            state: WEnum::Value(LifecycleState::Prepared),
        });
        assert!(state.needs_preview() && state.refresh_requested);
        state.refresh_requested = false;
        state.widget_event(Event::TransitionIncoming);
        assert!(state.refresh_requested);
        for lifecycle in [LifecycleState::Entering, LifecycleState::Leaving] {
            state.widget_event(Event::Lifecycle {
                state: WEnum::Value(lifecycle),
            });
            state.widget_event(Event::TransitionIncoming);
            assert!(!state.renderable() && !state.refresh_requested);
        }
        state.widget_event(Event::Shutdown);
        assert!(state.shutdown);
    }

    #[test]
    fn closes_unused_configuration_and_credential_descriptors() {
        use deck_widget_surface_v2::Event;
        use std::io::Read;
        let events: [fn(OwnedFd) -> Event; 3] = [
            |fd| Event::Params { fd, size: 2 },
            |fd| Event::Credentials { fd, size: 2 },
            |fd| Event::CredentialSecrets { fd, size: 2 },
        ];
        let mut state = State::default();
        for event in events {
            let (descriptor, mut peer) = UnixStream::pair().unwrap();
            peer.set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            state.widget_event(event(descriptor.into()));
            assert_eq!(peer.read(&mut [0]).unwrap(), 0);
        }
    }

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

#[cfg(test)]
#[path = "wayland_wire_tests.rs"]
mod wire_tests;
