//! In-process wire-contract fixture, not a BMC compositor or hardware test.
//! Replacement follows upstream b2c945b's protocol/dispatch.rs: a second
//! connection with the configured instance key disconnects the first client.
use super::*;
use std::io::Read;
use std::os::unix::fs::FileExt;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use wayland_server::backend::{ClientId, DisconnectReason};
use wayland_server::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_region, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_server::{
    Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, New, Resource,
};

mod server_protocol {
    use wayland_server;
    use wayland_server::protocol::*;
    pub mod __interfaces {
        use wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("../protocol/deck-widget.xml");
    }
    use self::__interfaces::*;
    wayland_scanner::generate_server_code!("../protocol/deck-widget.xml");
}
use server_protocol::{
    deck_widget_manager_v3 as manager, deck_widget_surface_v2 as widget_surface,
};

const KEY: &str = "01234567-89ab-cdef-0123-456789abcdef";
const WAIT: Duration = Duration::from_secs(2);

#[derive(Debug)]
struct Frame {
    pixels: Vec<u8>,
    width: i32,
    height: i32,
}
struct Pool {
    file: Arc<std::fs::File>,
    size: Mutex<u64>,
}
struct Buffer {
    file: Arc<std::fs::File>,
    offset: u64,
    length: usize,
    width: i32,
    height: i32,
}
#[derive(Default)]
struct Surface(Mutex<Option<wl_buffer::WlBuffer>>);
struct Server {
    active: Option<(ClientId, widget_surface::DeckWidgetSurfaceV2)>,
    frames: mpsc::Sender<Frame>,
    peers: mpsc::Sender<UnixStream>,
    lifecycle: Option<widget_surface::LifecycleState>,
}

impl Server {
    // A socket is intentionally used instead of a memfd for these ignored
    // payloads: EOF on its peer proves closure of every transferred FD copy.
    fn payloads(&self, surface: &widget_surface::DeckWidgetSurfaceV2) {
        for kind in 0..3 {
            let (fd, peer) = UnixStream::pair().unwrap();
            peer.set_read_timeout(Some(WAIT)).unwrap();
            match kind {
                0 => surface.params(fd.as_fd(), 2),
                1 => surface.credentials(fd.as_fd(), 2),
                _ => surface.credential_secrets(fd.as_fd(), 2),
            }
            self.peers.send(peer).unwrap();
        }
    }
}

macro_rules! global {
    ($interface:ty, $after:expr) => {
        impl GlobalDispatch<$interface, ()> for Server {
            fn bind(
                _state: &mut Self,
                _handle: &DisplayHandle,
                _client: &Client,
                resource: New<$interface>,
                _data: &(),
                init: &mut DataInit<'_, Self>,
            ) {
                let resource = init.init(resource, ());
                ($after)(resource);
            }
        }
    };
}
global!(wl_compositor::WlCompositor, |_| {});
global!(wl_shm::WlShm, |shm: wl_shm::WlShm| shm
    .format(wl_shm::Format::Xrgb8888));
global!(manager::DeckWidgetManagerV3, |_| {});

impl Dispatch<manager::DeckWidgetManagerV3, ()> for Server {
    fn request(
        state: &mut Self,
        client: &Client,
        resource: &manager::DeckWidgetManagerV3,
        request: manager::Request,
        _: &(),
        handle: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let manager::Request::GetWidgetSurface {
            id,
            widget_instance_key,
            ..
        } = request
        {
            if widget_instance_key != KEY {
                resource.post_error(manager::Error::UnknownWidget, "not a configured instance");
                return;
            }
            let surface = init.init(id, ());
            if let Some((old, _)) = state.active.replace((client.id(), surface.clone())) {
                handle
                    .backend_handle()
                    .kill_client(old, DisconnectReason::ConnectionClosed);
            }
            surface.configure(
                canvas::WIDTH,
                canvas::HEIGHT,
                widget_surface::ViewportShape::Rectangular,
                "".to_owned(),
            );
            surface.display_info(
                canvas::WIDTH,
                canvas::HEIGHT,
                widget_surface::DisplayShape::Rectangular,
                96,
            );
            state.payloads(&surface);
            surface.timezone("Europe/Prague".to_owned());
            surface.night_mode(widget_surface::NightModeState::Off);
            surface.date_format(widget_surface::DateFormat::DdMmYyyyDot);
            surface.time_format(widget_surface::TimeFormat::Hour24);
            surface.number_format(widget_surface::NumberFormat::SpaceGroupCommaDecimal);
            surface.temperature_unit(widget_surface::TemperatureUnit::Celsius);
            surface.first_day_of_week(widget_surface::Weekday::Monday);
            surface.unit_system(widget_surface::UnitSystem::Metric);
            surface.next_alarm(widget_surface::Presence::Absent, 0, 0, "".to_owned());
            if surface.version() >= 2 {
                surface.language(Some("en-GB".to_owned()));
            }
            surface.configure_done();
            if let Some(lifecycle) = state.lifecycle {
                surface.lifecycle(lifecycle);
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for Server {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_compositor::WlCompositor,
        request: wl_compositor::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_compositor::Request::CreateSurface { id } => {
                init.init(id, Surface::default());
            }
            wl_compositor::Request::CreateRegion { id } => {
                init.init(id, ());
            }
            _ => {}
        }
    }
}
impl Dispatch<wl_shm::WlShm, ()> for Server {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_shm::WlShm,
        request: wl_shm::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm::Request::CreatePool { id, fd, size } = request {
            let file = Arc::new(std::fs::File::from(fd));
            let size = u64::try_from(size).unwrap();
            assert!(size > 0 && size <= file.metadata().unwrap().len());
            init.init(
                id,
                Pool {
                    file,
                    size: Mutex::new(size),
                },
            );
        }
    }
}
impl Dispatch<wl_shm_pool::WlShmPool, Pool> for Server {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_shm_pool::WlShmPool,
        request: wl_shm_pool::Request,
        pool: &Pool,
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_shm_pool::Request::CreateBuffer {
                id,
                offset,
                width,
                height,
                stride,
                format,
            } => {
                assert_eq!(
                    format,
                    wayland_server::WEnum::Value(wl_shm::Format::Xrgb8888)
                );
                let length =
                    buffer_length(offset, width, height, stride, *pool.size.lock().unwrap())
                        .expect("buffer must fit the declared SHM pool");
                init.init(
                    id,
                    Buffer {
                        file: pool.file.clone(),
                        offset: offset as u64,
                        length,
                        width,
                        height,
                    },
                );
            }
            wl_shm_pool::Request::Resize { size } => {
                let size = u64::try_from(size).unwrap();
                let mut current = pool.size.lock().unwrap();
                assert!(size >= *current && size <= pool.file.metadata().unwrap().len());
                *current = size;
            }
            _ => {}
        }
    }
}

fn buffer_length(
    offset: i32,
    width: i32,
    height: i32,
    stride: i32,
    pool_size: u64,
) -> Option<usize> {
    let offset = u64::try_from(offset).ok()?;
    let width = u64::try_from(width).ok()?;
    let height = u64::try_from(height).ok()?;
    let stride = u64::try_from(stride).ok()?;
    if width == 0 || height == 0 || stride != width.checked_mul(4)? {
        return None;
    }
    let length = height.checked_mul(stride)?;
    (offset.checked_add(length)? <= pool_size).then_some(usize::try_from(length).ok()?)
}
impl Dispatch<wl_surface::WlSurface, Surface> for Server {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &wl_surface::WlSurface,
        request: wl_surface::Request,
        data: &Surface,
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_surface::Request::Attach { buffer, .. } => *data.0.lock().unwrap() = buffer,
            wl_surface::Request::Commit => {
                if let Some(buffer) = data.0.lock().unwrap().take() {
                    let data = buffer.data::<Buffer>().unwrap();
                    let mut pixels = vec![0; data.length];
                    data.file.read_exact_at(&mut pixels, data.offset).unwrap();
                    state
                        .frames
                        .send(Frame {
                            pixels,
                            width: data.width,
                            height: data.height,
                        })
                        .unwrap();
                    buffer.release();
                }
            }
            wl_surface::Request::Frame { callback } => {
                init.init(callback, ()).done(0);
            }
            _ => {}
        }
    }
}

// Commands and barriers avoid timing assumptions and process-wide environment
// changes. The server's hard deadline bounds even a stuck client roundtrip.
enum Command {
    Connect(UnixStream),
    Lifecycle(widget_surface::LifecycleState),
    Language(Option<String>),
    Payloads,
    Transition,
    Shutdown,
    Barrier(mpsc::Sender<()>),
    Stop,
}
struct Fixture {
    commands: mpsc::Sender<Command>,
    frames: mpsc::Receiver<Frame>,
    peers: mpsc::Receiver<UnixStream>,
    worker: Option<JoinHandle<()>>,
}
impl Fixture {
    fn new() -> Self {
        Self::with_protocol_version(2)
    }
    fn with_protocol_version(version: u32) -> Self {
        Self::with_initial_lifecycle(version, Some(widget_surface::LifecycleState::Visible))
    }
    fn with_initial_lifecycle(
        version: u32,
        lifecycle: Option<widget_surface::LifecycleState>,
    ) -> Self {
        let (commands, incoming) = mpsc::channel();
        let (frames_tx, frames) = mpsc::channel();
        let (peers_tx, peers) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut display = Display::<Server>::new().unwrap();
            let mut handle = display.handle();
            handle.create_global::<Server, wl_compositor::WlCompositor, _>(4, ());
            handle.create_global::<Server, wl_shm::WlShm, _>(1, ());
            handle.create_global::<Server, manager::DeckWidgetManagerV3, _>(version, ());
            let mut state = Server {
                active: None,
                frames: frames_tx,
                peers: peers_tx,
                lifecycle,
            };
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut clients = Vec::new();
            let mut stopping = false;
            while !stopping && Instant::now() < deadline {
                for command in incoming.try_iter() {
                    match command {
                        Command::Connect(stream) => {
                            clients.push(handle.insert_client(stream, Arc::new(())).unwrap())
                        }
                        Command::Lifecycle(lifecycle) => {
                            state.lifecycle = Some(lifecycle);
                            state.active.as_ref().unwrap().1.lifecycle(lifecycle);
                        }
                        Command::Language(language) => {
                            let surface = &state.active.as_ref().unwrap().1;
                            if surface.version() >= 2 {
                                surface.language(language);
                            }
                        }
                        Command::Payloads => state.payloads(&state.active.as_ref().unwrap().1),
                        Command::Transition => {
                            state.active.as_ref().unwrap().1.transition_incoming()
                        }
                        Command::Shutdown => state.active.as_ref().unwrap().1.shutdown(),
                        Command::Barrier(done) => {
                            display.flush_clients().unwrap();
                            done.send(()).unwrap();
                        }
                        Command::Stop => stopping = true,
                    }
                }
                display.dispatch_clients(&mut state).unwrap();
                display.flush_clients().unwrap();
                thread::sleep(Duration::from_millis(1));
            }
            // Explicitly close clients on deadline, including those waiting in
            // initialize_connection's real registry roundtrip.
            for client in clients {
                handle
                    .backend_handle()
                    .kill_client(client.id(), DisconnectReason::ConnectionClosed);
            }
        });
        Self {
            commands,
            frames,
            peers,
            worker: Some(worker),
        }
    }
    fn open_key(&self, key: &str) -> Result<Widget, String> {
        let (client, server) = UnixStream::pair().unwrap();
        self.commands.send(Command::Connect(server)).unwrap();
        let (queue, state) = initialize_connection(Connection::from_socket(client).unwrap())?;
        let mut widget = Widget::finish_open(queue, state, key)?;
        widget
            .queue
            .roundtrip(&mut widget.state)
            .map_err(|error| error.to_string())?;
        Ok(widget)
    }
    fn open(&self) -> Widget {
        self.open_key(KEY).unwrap()
    }
    fn send(&self, widget: &mut Widget, command: Command) {
        self.commands.send(command).unwrap();
        let (done, received) = mpsc::channel();
        self.commands.send(Command::Barrier(done)).unwrap();
        received.recv_timeout(WAIT).unwrap();
        widget.queue.roundtrip(&mut widget.state).unwrap();
    }
    fn assert_payloads_closed(&self) {
        for _ in 0..3 {
            let mut peer = self.peers.recv_timeout(WAIT).unwrap();
            assert_eq!(
                peer.read(&mut [0]).unwrap(),
                0,
                "client leaked a payload FD"
            );
        }
    }
    fn assert_frame(&self, widget: &mut Widget, color: u32) {
        widget.present_solid(color).unwrap();
        let frame = self.frames.recv_timeout(WAIT).unwrap();
        let expected = (0xff00_0000 | color).to_ne_bytes();
        assert!(frame.pixels.chunks_exact(4).all(|pixel| pixel == expected));
        assert_eq!(
            frame.pixels.len(),
            canvas::WIDTH as usize * canvas::HEIGHT as usize * 4
        );
        assert_eq!(
            (frame.width, frame.height),
            (canvas::WIDTH as i32, canvas::HEIGHT as i32)
        );
        widget.queue.roundtrip(&mut widget.state).unwrap();
        assert!(widget.state.slots.iter().all(|slot| !slot.busy));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let result = worker.join();
            if !thread::panicking() {
                result.unwrap();
            }
        }
    }
}

#[test]
fn keyed_wire_repeated_game_replacement_and_dashboard_reconnection() {
    let fixture = Fixture::new();
    let mut dashboard = fixture.open();
    for cycle in 0..4 {
        assert!(dashboard.state.configured && dashboard.state.visible);
        fixture.assert_payloads_closed();
        fixture.assert_frame(&mut dashboard, 0x123400 + cycle);
        // Yielding means no dispatch on the dashboard while the game owns the
        // key. Detect the old socket's closure only after the simulated return.
        let mut game = fixture.open();
        fixture.assert_payloads_closed();
        fixture.assert_frame(&mut game, 0x345600 + cycle);
        assert!(dashboard.dispatch(200).is_err());
        drop(dashboard);
        // Closing without an orderly protocol destroy models a crashed game.
        drop(game);
        dashboard = fixture.open();
    }
    fixture.assert_payloads_closed();
    fixture.assert_frame(&mut dashboard, 0xabcdef);
}

#[test]
fn keyed_wire_runtime_payloads_lifecycle_and_shutdown() {
    let fixture = Fixture::new();
    let mut widget = fixture.open();
    fixture.assert_payloads_closed();
    widget.state.refresh_requested = false;
    fixture.send(
        &mut widget,
        Command::Lifecycle(widget_surface::LifecycleState::Dormant),
    );
    assert!(!widget.state.visible && !widget.state.refresh_requested);
    drop(widget);
    let mut widget = fixture.open();
    assert!(widget.state.configured && !widget.state.visible);
    fixture.assert_payloads_closed();
    widget.state.refresh_requested = false;
    fixture.send(&mut widget, Command::Payloads);
    fixture.assert_payloads_closed();
    fixture.send(&mut widget, Command::Transition);
    assert!(widget.state.refresh_requested && !widget.state.visible);
    fixture.send(
        &mut widget,
        Command::Lifecycle(widget_surface::LifecycleState::Visible),
    );
    assert!(widget.state.visible);
    fixture.assert_frame(&mut widget, 0x112233);
    fixture.send(&mut widget, Command::Shutdown);
    assert!(widget.state.shutdown);
}

#[test]
fn keyed_wire_negotiates_v1_and_v2_with_english_fallback() {
    for version in [1, 2] {
        let fixture = Fixture::with_protocol_version(version);
        let mut widget = fixture.open();
        assert_eq!(
            widget.state.widget_surface.as_ref().unwrap().version(),
            version
        );
        fixture.assert_payloads_closed();
        if version == 2 {
            fixture.send(&mut widget, Command::Language(Some("cs-CZ".to_owned())));
            fixture.send(&mut widget, Command::Language(None));
        }
        fixture.assert_frame(&mut widget, 0x123456);
    }
}

#[test]
fn keyed_wire_waits_for_delayed_lifecycle_before_allocating_or_drawing() {
    let fixture = Fixture::with_initial_lifecycle(2, None);
    let mut widget = fixture.open();
    assert!(widget.state.configured && !widget.state.visible);
    fixture.assert_payloads_closed();
    widget.present_solid(0x123456).unwrap();
    widget.queue.roundtrip(&mut widget.state).unwrap();
    assert!(widget.state.slots.is_empty());
    assert!(fixture.frames.try_recv().is_err());
    fixture.send(
        &mut widget,
        Command::Lifecycle(widget_surface::LifecycleState::Prepared),
    );
    fixture.assert_frame(&mut widget, 0x123456);
    assert!(!widget.state.active && !widget.state.renderable());
    widget.present_solid(0x654321).unwrap();
    widget.queue.roundtrip(&mut widget.state).unwrap();
    assert!(fixture.frames.try_recv().is_err());
    fixture.send(
        &mut widget,
        Command::Lifecycle(widget_surface::LifecycleState::Dormant),
    );
    widget.present_solid(0x654321).unwrap();
    widget.queue.roundtrip(&mut widget.state).unwrap();
    assert!(fixture.frames.try_recv().is_err());
    fixture.send(
        &mut widget,
        Command::Lifecycle(widget_surface::LifecycleState::Visible),
    );
    fixture.assert_frame(&mut widget, 0x654321);
}

#[test]
fn keyed_wire_game_wait_resumes_or_stops_on_shutdown_and_disconnect() {
    for (lifecycle, stop) in [
        (widget_surface::LifecycleState::Dormant, false),
        (widget_surface::LifecycleState::Dormant, true),
        (widget_surface::LifecycleState::Prepared, false),
        (widget_surface::LifecycleState::Prepared, true),
    ] {
        let fixture = Fixture::new();
        let commands = fixture.commands.clone();
        let (ready, started) = mpsc::channel();
        let (done, finished) = mpsc::channel();
        let client = thread::spawn(move || {
            let mut widget = fixture.open();
            fixture.assert_payloads_closed();
            fixture.send(&mut widget, Command::Lifecycle(lifecycle));
            WIDGET.with(|slot| *slot.borrow_mut() = Some(widget));
            let mut hold = Some(Instant::now());
            ready.send(()).unwrap();
            let result = crate::game_video::wait_for_widget_activity(&mut hold);
            assert!(hold.is_none());
            assert!(result.is_ok());
            assert_eq!(shutdown_requested(), stop);
            close();
            done.send(()).unwrap();
        });
        started.recv_timeout(WAIT).unwrap();
        assert!(matches!(
            finished.recv_timeout(Duration::from_millis(20)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        commands
            .send(if stop {
                Command::Shutdown
            } else {
                Command::Lifecycle(widget_surface::LifecycleState::Visible)
            })
            .unwrap();
        finished.recv_timeout(WAIT).unwrap();
        client.join().unwrap();
    }
    let fixture = Fixture::new();
    let widget = fixture.open();
    fixture.assert_payloads_closed();
    WIDGET.with(|slot| *slot.borrow_mut() = Some(widget));
    drop(fixture);
    assert!(crate::game_video::wait_for_widget_activity(&mut None).is_err());
    close();
}

#[test]
fn shm_buffer_extent_must_fit_the_declared_pool() {
    assert_eq!(buffer_length(4, 2, 3, 8, 28), Some(24));
    for layout in [
        (-1, 2, 3, 8),
        (0, 0, 3, 0),
        (0, 2, -1, 8),
        (0, 2, 3, 4),
        (5, 2, 3, 8),
    ] {
        assert_eq!(
            buffer_length(layout.0, layout.1, layout.2, layout.3, 28),
            None
        );
    }
}

#[test]
fn keyed_wire_rejected_connection_does_not_prevent_dashboard_recovery() {
    let fixture = Fixture::new();
    assert!(
        fixture
            .open_key("fedcba98-7654-3210-fedc-ba9876543210")
            .is_err()
    );
    let mut dashboard = fixture.open();
    fixture.assert_payloads_closed();
    fixture.assert_frame(&mut dashboard, 0x654321);
}

macro_rules! ignore_requests {
    ($interface:ty, $data:ty) => {
        impl Dispatch<$interface, $data> for Server {
            fn request(
                _: &mut Self,
                _: &Client,
                _: &$interface,
                _: <$interface as Resource>::Request,
                _: &$data,
                _: &DisplayHandle,
                _: &mut DataInit<'_, Self>,
            ) {
            }
        }
    };
}
ignore_requests!(wl_region::WlRegion, ());
ignore_requests!(wl_callback::WlCallback, ());
ignore_requests!(wl_buffer::WlBuffer, Buffer);
ignore_requests!(widget_surface::DeckWidgetSurfaceV2, ());
