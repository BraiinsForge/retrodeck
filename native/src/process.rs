use crate::{fbdev, input, wayland};
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(40);
const TOUCH_HOLD: Duration = Duration::from_secs(2);
const TERM_GRACE: Duration = Duration::from_secs(4);
const TOUCH_RECONNECT: Duration = Duration::from_secs(1);
const CONSOLE_FRAME_INTERVAL: Duration = Duration::from_millis(100);
const KDGKBMODE: libc::c_ulong = 0x4b44;
const KDSKBMODE: libc::c_ulong = 0x4b45;
const DISPLAY_STATE: &[u8] = b"\x1b[?25l\x1b[13]\x1b[9;0]";
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

extern "C" fn request_shutdown(_: libc::c_int) {
    SHUTDOWN_REQUESTED.store(true, Ordering::Relaxed);
}

pub fn install_signal_handlers() -> Result<(), String> {
    SHUTDOWN_REQUESTED.store(false, Ordering::Relaxed);
    let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
    action.sa_sigaction = request_shutdown as *const () as usize;
    unsafe { libc::sigemptyset(&mut action.sa_mask) };
    for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } != 0 {
            return Err(format!(
                "cannot install signal handler: {}",
                io::Error::last_os_error()
            ));
        }
    }
    if unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) } == libc::SIG_ERR {
        return Err(format!(
            "cannot ignore SIGPIPE: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(())
}

pub fn shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::Relaxed)
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct ChildResult {
    pub started: bool,
    pub exited_for_touch: bool,
    pub shutdown_requested: bool,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub error: Option<String>,
}

struct TtySnapshot {
    file: Option<File>,
    termios: Option<libc::termios>,
    keyboard_mode: Option<libc::c_int>,
}

impl TtySnapshot {
    fn capture() -> Self {
        let Ok(file) = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open("/dev/tty0")
        else {
            return Self {
                file: None,
                termios: None,
                keyboard_mode: None,
            };
        };
        let fd = file.as_raw_fd();
        let mut termios = MaybeUninit::<libc::termios>::uninit();
        let termios = (unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } == 0)
            .then(|| unsafe { termios.assume_init() });
        let mut keyboard_mode = 0;
        let keyboard_mode = (unsafe { libc::ioctl(fd, KDGKBMODE, &mut keyboard_mode) } == 0)
            .then_some(keyboard_mode);
        Self {
            file: Some(file),
            termios,
            keyboard_mode,
        }
    }

    fn restore(&self) {
        if let Some(file) = &self.file {
            let fd = file.as_raw_fd();
            if let Some(mode) = self.keyboard_mode {
                unsafe {
                    libc::ioctl(fd, KDSKBMODE, mode);
                }
            }
            if let Some(termios) = &self.termios {
                unsafe {
                    libc::tcsetattr(fd, libc::TCSAFLUSH, termios);
                }
            }
        }
        if let Ok(mut console) = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_CLOEXEC)
            .open("/dev/tty0")
        {
            let _ = console.write_all(DISPLAY_STATE);
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum StopRequest {
    None,
    Touch,
    Shutdown,
}

struct TouchHold {
    active_since: Option<Instant>,
}

impl TouchHold {
    fn new() -> Self {
        Self { active_since: None }
    }

    fn update(&mut self, down: bool, x: i32, y: i32) {
        let inside = down
            && x >= 0
            && x < crate::canvas::WIDTH as i32
            && y >= 0
            && y < crate::canvas::HEIGHT as i32;
        if !inside {
            if self.active_since.take().is_some() {
                eprintln!("retrodeck: return hold cancelled at {x},{y}");
            }
        } else if self.active_since.is_none() {
            self.active_since = Some(Instant::now());
            eprintln!("retrodeck: return hold started at {x},{y}");
        }
    }

    fn reset(&mut self) {
        self.active_since = None;
    }

    fn complete(&self, now: Instant) -> bool {
        self.active_since
            .is_some_and(|started| now.duration_since(started) >= TOUCH_HOLD)
    }
}

struct ChildInteraction {
    uses_wayland: bool,
    touch_supervision: bool,
    mirror_console: bool,
    next_console_frame: Instant,
    last_touch_attempt: Option<Instant>,
    last_touch_error: String,
    hold: TouchHold,
}

impl ChildInteraction {
    fn new(touch_supervision: bool, mirror_console: bool) -> Self {
        let uses_wayland = wayland::size().is_some();
        if !uses_wayland {
            fbdev::close();
            if !touch_supervision {
                input::close_touch();
            }
        }
        Self {
            uses_wayland,
            touch_supervision,
            mirror_console: uses_wayland && mirror_console,
            next_console_frame: Instant::now(),
            last_touch_attempt: None,
            last_touch_error: String::new(),
            hold: TouchHold::new(),
        }
    }

    fn yield_widget_to_child(&mut self) {
        // Stock BMC replaces the entire prior client connection when a child
        // attaches for this instance. Only the child may dispatch or render it.
        self.uses_wayland = false;
        self.touch_supervision = false;
        self.mirror_console = false;
        self.hold.reset();
    }

    fn step(&mut self, timeout: Duration) -> StopRequest {
        if self.uses_wayland {
            if let Err(error) = wayland::dispatch(duration_ms(timeout)) {
                eprintln!("retrodeck: {error}");
                self.hold.reset();
            }
            while let Some(report) = wayland::next_touch() {
                if self.touch_supervision {
                    self.hold.update(report.down, report.x, report.y);
                }
            }
        } else if self.touch_supervision {
            self.poll_evdev(timeout);
        } else {
            thread::sleep(timeout);
        }

        let now = Instant::now();
        if self.mirror_console && now >= self.next_console_frame {
            let result =
                fbdev::read_console_scanout().and_then(|frame| wayland::present_rgb565(&frame));
            if let Err(error) = result {
                eprintln!("retrodeck: terminal display unavailable: {error}");
                self.mirror_console = false;
            }
            self.next_console_frame = now + CONSOLE_FRAME_INTERVAL;
        }
        if shutdown_requested() || (self.uses_wayland && wayland::shutdown_requested()) {
            StopRequest::Shutdown
        } else if self.touch_supervision && self.hold.complete(now) {
            StopRequest::Touch
        } else {
            StopRequest::None
        }
    }

    fn poll_evdev(&mut self, timeout: Duration) {
        let now = Instant::now();
        let reconnect_due = self
            .last_touch_attempt
            .is_none_or(|last| now.duration_since(last) >= TOUCH_RECONNECT);
        if !input::touch_open() && reconnect_due {
            self.last_touch_attempt = Some(now);
            if let Err(error) = input::open_touch()
                && error != self.last_touch_error
            {
                eprintln!("retrodeck: {error}");
                self.last_touch_error = error;
            }
        }
        if input::touch_open() {
            if let Err(error) = input::dispatch_touch(duration_ms(timeout)) {
                eprintln!("retrodeck: {error}");
                input::close_touch();
                self.hold.reset();
            }
            let mut received_report = false;
            while let Some(report) = input::next_touch() {
                received_report = true;
                self.hold.update(report.down, report.x, report.y);
            }
            if !received_report && let Some(report) = input::current_touch() {
                self.hold.update(report.down, report.x, report.y);
            }
        } else {
            thread::sleep(timeout);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelperPhase {
    Complete,
    Start,
    Input,
    Wait,
}

#[derive(Debug, Eq, PartialEq)]
pub struct HelperResult {
    pub phase: HelperPhase,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

pub fn run_helper(executable: &Path, input: &[u8]) -> HelperResult {
    run_helper_until(
        executable,
        input,
        Duration::from_secs(10),
        shutdown_requested,
    )
}

fn run_helper_until(
    executable: &Path,
    input: &[u8],
    timeout: Duration,
    cancelled: impl Fn() -> bool,
) -> HelperResult {
    let failure = |phase, error: String| HelperResult {
        phase,
        exit_code: None,
        error: Some(error),
    };
    // Isolate descendants and suppress helper output, which can contain credentials.
    let mut child = match Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return failure(HelperPhase::Start, error.to_string()),
    };
    let deadline = Instant::now() + timeout;
    let mut stdin = child.stdin.take().expect("piped helper stdin");
    let fd = stdin.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    let mut input_error =
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            Some(io::Error::last_os_error().to_string())
        } else {
            None
        };
    let mut written = 0;
    while written < input.len() && input_error.is_none() {
        if cancelled() || Instant::now() >= deadline {
            input_error = Some("helper input cancelled or timed out".into());
            break;
        }
        match stdin.write(&input[written..]) {
            Ok(0) => input_error = Some("helper stdin closed".into()),
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(
                    POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                );
            }
            Err(error) => input_error = Some(error.to_string()),
        }
    }
    drop(stdin);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return HelperResult {
                    phase: if input_error.is_some() {
                        HelperPhase::Input
                    } else {
                        HelperPhase::Complete
                    },
                    exit_code: status.code(),
                    error: input_error,
                };
            }
            Ok(None) if !cancelled() && Instant::now() < deadline => {
                thread::sleep(
                    POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                );
                continue;
            }
            Ok(None) => {}
            Err(error) => {
                stop_helper(&mut child);
                return failure(HelperPhase::Wait, error.to_string());
            }
        }
        stop_helper(&mut child);
        return failure(
            if input_error.is_some() {
                HelperPhase::Input
            } else {
                HelperPhase::Wait
            },
            input_error.unwrap_or_else(|| "helper cancelled or timed out".into()),
        );
    }
}

fn stop_helper(child: &mut std::process::Child) {
    // Kill the whole private group before reaping the leader.
    unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
    let _ = child.kill();
    let _ = child.wait();
}

fn widget_child_requested(environment: &[(OsString, OsString)]) -> bool {
    environment.iter().any(|(name, value)| {
        name.as_os_str() == OsStr::new("RETRO_DECK_PRESENTATION")
            && value.as_os_str() == OsStr::new("widget")
    })
}

pub fn run_child(
    executable: &Path,
    arguments: &[OsString],
    environment: &[(OsString, OsString)],
    label: &str,
    touch_supervision: bool,
    mirror_console: bool,
) -> ChildResult {
    let mut interaction = ChildInteraction::new(touch_supervision, mirror_console);
    let widget_child = widget_child_requested(environment);
    if widget_child && !interaction.uses_wayland {
        return ChildResult {
            error: Some("BMC widget launch requires an open dashboard widget".to_owned()),
            ..ChildResult::default()
        };
    }
    if widget_child {
        interaction.yield_widget_to_child();
    }
    let tty = TtySnapshot::capture();
    eprintln!("retrodeck: launching {label}");
    let result = spawn_and_supervise(
        executable,
        arguments,
        environment,
        label,
        |timeout| interaction.step(timeout),
        POLL_INTERVAL,
        TERM_GRACE,
    );
    tty.restore();
    if widget_child {
        // Discard the replaced connection. Dashboard recovery opens a fresh
        // surface with the same BMC_WIDGET_KEY after the child has exited.
        wayland::close();
    }
    result
}

fn spawn_and_supervise<F>(
    executable: &Path,
    arguments: &[OsString],
    environment: &[(OsString, OsString)],
    label: &str,
    mut step: F,
    poll_interval: Duration,
    term_grace: Duration,
) -> ChildResult
where
    F: FnMut(Duration) -> StopRequest,
{
    let mut command = Command::new(executable);
    command.args(arguments).envs(environment.iter().cloned());
    // Capture before fork: checking only getppid() in the child would miss a
    // dashboard death between fork and installation of the parent-death signal.
    let parent_pid = unsafe { libc::getpid() };
    // The hook uses only libc calls and errno-based errors, with no allocation.
    unsafe {
        command.pre_exec(move || reset_child_process(parent_pid));
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return ChildResult {
                error: Some(format!("cannot start {label}: {error}")),
                ..ChildResult::default()
            };
        }
    };
    let mut result = ChildResult {
        started: true,
        ..ChildResult::default()
    };
    let Ok(pid) = libc::pid_t::try_from(child.id()) else {
        result.error = Some("child process id is out of range".to_owned());
        let _ = child.kill();
        let _ = child.wait();
        return result;
    };
    if unsafe { libc::setpgid(pid, pid) } != 0 {
        let error = io::Error::last_os_error();
        if !matches!(error.raw_os_error(), Some(libc::EACCES | libc::ESRCH)) {
            eprintln!("retrodeck: warning: cannot establish child process group: {error}");
        }
    }

    let mut term_sent_at = None;
    let mut kill_sent = false;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                set_status(&mut result, status);
                if let Some(sent_at) = term_sent_at
                    && finish_stopped_group(pid, sent_at, term_grace, poll_interval, &mut step)
                {
                    result.exited_for_touch = false;
                    result.shutdown_requested = true;
                }
                break;
            }
            Ok(None) => {}
            Err(error) => {
                result.error = Some(format!("waitpid failed: {error}"));
                signal_child_group(pid, libc::SIGKILL);
                if let Ok(status) = child.wait() {
                    set_status(&mut result, status);
                }
                break;
            }
        }

        let request = step(poll_interval);
        if term_sent_at.is_none() && request != StopRequest::None {
            eprintln!("retrodeck: stopping {label}");
            signal_child_group(pid, libc::SIGTERM);
            term_sent_at = Some(Instant::now());
            result.exited_for_touch = request == StopRequest::Touch;
            result.shutdown_requested = request == StopRequest::Shutdown;
        }
        if !kill_sent && term_sent_at.is_some_and(|sent| sent.elapsed() >= term_grace) {
            signal_child_group(pid, libc::SIGKILL);
            kill_sent = true;
        }
    }

    if let Some(code) = result.exit_code {
        eprintln!("retrodeck: {label} exited with status {code}");
    } else if let Some(signal) = result.signal {
        eprintln!("retrodeck: {label} stopped by signal {signal}");
    }
    result
}

fn reset_child_process(parent_pid: libc::pid_t) -> io::Result<()> {
    // A vanished dashboard cannot supervise a graceful exit. SIGKILL also stops
    // apps that ignore SIGTERM; normal shutdown still uses the TERM/KILL grace.
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::getppid() } != parent_pid {
        return Err(io::Error::from_raw_os_error(libc::ESRCH));
    }
    for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP, libc::SIGPIPE] {
        if unsafe { libc::signal(signal, libc::SIG_DFL) } == libc::SIG_ERR {
            return Err(io::Error::last_os_error());
        }
    }
    if unsafe { libc::setpgid(0, 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn signal_child_group(pid: libc::pid_t, signal: libc::c_int) {
    if unsafe { libc::kill(-pid, signal) } != 0
        && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    {
        unsafe {
            libc::kill(pid, signal);
        }
    }
}

fn child_group_exists(pid: libc::pid_t) -> bool {
    (unsafe { libc::kill(-pid, 0) }) == 0
        || io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

fn finish_stopped_group<F>(
    pid: libc::pid_t,
    term_sent_at: Instant,
    term_grace: Duration,
    poll_interval: Duration,
    step: &mut F,
) -> bool
where
    F: FnMut(Duration) -> StopRequest,
{
    let mut shutdown = false;
    while child_group_exists(pid) && term_sent_at.elapsed() < term_grace {
        let timeout = poll_interval.min(term_grace.saturating_sub(term_sent_at.elapsed()));
        shutdown |= step(timeout) == StopRequest::Shutdown;
    }
    if child_group_exists(pid) {
        signal_child_group(pid, libc::SIGKILL);
    }
    shutdown
}

fn set_status(result: &mut ChildResult, status: ExitStatus) {
    result.exit_code = status.code();
    result.signal = status.signal();
}

fn duration_ms(duration: Duration) -> u32 {
    duration.as_millis().clamp(1, u128::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use HelperPhase::{Complete, Input, Start, Wait};
    use std::env;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn yields_all_widget_input_and_presentation_to_the_child() {
        let mut interaction = ChildInteraction {
            uses_wayland: true,
            touch_supervision: true,
            mirror_console: true,
            next_console_frame: Instant::now(),
            last_touch_attempt: None,
            last_touch_error: String::new(),
            hold: TouchHold::new(),
        };
        interaction.hold.update(true, 10, 10);
        interaction.yield_widget_to_child();
        assert!(!interaction.uses_wayland);
        assert!(!interaction.touch_supervision);
        assert!(!interaction.mirror_console);
        assert!(interaction.hold.active_since.is_none());
    }

    #[test]
    fn managed_child_fixture() {
        let Ok(action) = env::var("RETRODECK_PROCESS_FIXTURE") else {
            return;
        };
        assert_eq!(env::var("RETRO_DECK_KEYMAP").unwrap(), "cz");
        match action.as_str() {
            "clean" => {}
            "exit-7" => std::process::exit(7),
            "signal" => unsafe {
                libc::raise(libc::SIGUSR1);
            },
            "wait" => unsafe {
                loop {
                    libc::pause();
                }
            },
            "death-wait" => {
                unsafe {
                    libc::signal(libc::SIGTERM, libc::SIG_IGN);
                }
                std::fs::write(
                    env::var_os("RETRODECK_PROCESS_PIDS").unwrap(),
                    std::process::id().to_string(),
                )
                .unwrap();
                loop {
                    unsafe { libc::pause() };
                }
            }
            "supervisor" => {
                let extra = [(
                    OsString::from("RETRODECK_PROCESS_PIDS"),
                    env::var_os("RETRODECK_PROCESS_PIDS").unwrap(),
                )];
                let (executable, arguments, environment) = fixture_command("death-wait", &extra);
                let result =
                    supervise_fixture(&executable, &arguments, &environment, StopRequest::None);
                panic!("parent-death fixture exited before supervisor was killed: {result:?}");
            }
            "leader-exits" => wait_with_grandchild(false),
            "group" => wait_with_grandchild(true),
            other => panic!("unknown fixture {other}"),
        }
    }

    fn wait_with_grandchild(leader_ignores_term: bool) -> ! {
        unsafe {
            if leader_ignores_term {
                libc::signal(libc::SIGTERM, libc::SIG_IGN);
            }
            let grandchild = libc::fork();
            if grandchild == 0 {
                libc::signal(libc::SIGTERM, libc::SIG_IGN);
                loop {
                    libc::pause();
                }
            }
            assert!(grandchild > 0);
            std::fs::write(
                env::var_os("RETRODECK_PROCESS_PIDS").unwrap(),
                format!("{} {grandchild}\n", std::process::id()),
            )
            .unwrap();
            loop {
                libc::pause();
            }
        }
    }

    fn fixture_command(
        action: &str,
        extra: &[(OsString, OsString)],
    ) -> (PathBuf, Vec<OsString>, Vec<(OsString, OsString)>) {
        let executable = env::current_exe().unwrap();
        let arguments = vec![
            "--exact".into(),
            "process::tests::managed_child_fixture".into(),
            "--test-threads=1".into(),
        ];
        let mut environment = vec![
            environment_variable("RETRODECK_PROCESS_FIXTURE", action),
            environment_variable("RETRO_DECK_KEYMAP", "cz"),
        ];
        environment.extend_from_slice(extra);
        (executable, arguments, environment)
    }

    fn temporary_path(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "retrodeck-{label}-{}-{}",
            std::process::id(),
            FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn helper_script(body: &str) -> PathBuf {
        let path = temporary_path("helper");
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn environment_variable(name: &str, value: &str) -> (OsString, OsString) {
        (name.into(), value.into())
    }

    type HelperOutcome = (HelperPhase, Option<i32>, bool);

    const HELPER_EXIT_7: HelperOutcome = (Complete, Some(7), false);
    const HELPER_SIGTERM: HelperOutcome = (Complete, None, false);
    const HELPER_INPUT_EXIT_7: HelperOutcome = (Input, Some(7), true);

    fn run_script_fixture(path: &Path, input: &[u8]) -> HelperResult {
        let busy = io::Error::from_raw_os_error(libc::ETXTBSY).to_string();
        for attempt in 0..5 {
            let result = run_helper(path, input);
            // A parallel fork can briefly inherit the temporary script's writer
            // descriptor before exec. Retry only that transient start failure.
            if result.phase != Start
                || result.error.as_deref() != Some(busy.as_str())
                || attempt == 4
            {
                return result;
            }
            thread::sleep(Duration::from_millis(10));
        }
        unreachable!()
    }

    fn assert_script(body: &str, input: &[u8], expected: HelperOutcome) {
        let helper = helper_script(body);
        let result = run_script_fixture(&helper, input);
        std::fs::remove_file(helper).unwrap();
        assert_eq!(result.phase, expected.0, "{result:?}");
        assert_eq!(result.exit_code, expected.1);
        assert_eq!(result.error.is_some(), expected.2);
    }

    #[test]
    fn helper_bounds_blocked_input_and_wait_and_honors_cancellation() {
        for (body, input, phase, cancel) in [
            ("exec sleep 30\n", vec![b'x'; 1024 * 1024], Input, false),
            (
                "cat >/dev/null\nexec sleep 30\n",
                b"profile\n".to_vec(),
                Wait,
                false,
            ),
            ("exec sleep 30\n", vec![b'x'; 1024 * 1024], Input, true),
        ] {
            let helper = helper_script(body);
            let started = Instant::now();
            let result = run_helper_until(&helper, &input, Duration::from_millis(200), || cancel);
            std::fs::remove_file(helper).unwrap();
            assert_eq!(result.phase, phase, "{result:?}");
            assert!(result.error.is_some());
            assert!(started.elapsed() < Duration::from_secs(2));
        }
    }

    #[test]
    fn helper_writes_exact_input_and_classifies_failures() {
        let capture = temporary_path("helper-input");
        let body = format!("cat > '{}'\n", capture.display());
        let helper = helper_script(&body);
        let input = b"test net\nsecret!9\n";
        let result = run_script_fixture(&helper, input);
        assert_eq!(result.phase, Complete, "{result:?}");
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(std::fs::read(&capture).unwrap(), input);
        std::fs::remove_file(&capture).unwrap();
        std::fs::remove_file(&helper).unwrap();

        assert_script("cat >/dev/null\nexit 7\n", input, HELPER_EXIT_7);
        assert_script("cat >/dev/null\nkill -TERM $$\n", input, HELPER_SIGTERM);
        assert_script("exit 7\n", &vec![b'x'; 1024 * 1024], HELPER_INPUT_EXIT_7);

        let result = run_helper(Path::new("/no/such/retrodeck-helper"), input);
        assert_eq!(result.phase, Start);
        assert!(result.error.is_some());
    }

    fn supervise_fixture(
        executable: &Path,
        arguments: &[OsString],
        environment: &[(OsString, OsString)],
        request: StopRequest,
    ) -> ChildResult {
        spawn_and_supervise(
            executable,
            arguments,
            environment,
            "fixture",
            |timeout| {
                if request == StopRequest::None {
                    thread::sleep(timeout);
                }
                request
            },
            Duration::from_millis(5),
            Duration::from_millis(50),
        )
    }

    fn run_fixture(action: &str, request: StopRequest) -> ChildResult {
        let (executable, arguments, environment) = fixture_command(action, &[]);
        supervise_fixture(&executable, &arguments, &environment, request)
    }

    #[test]
    fn passes_exact_generic_child_arguments_and_environment() {
        let capture = temporary_path("child-capture");
        let child = helper_script(&format!(
            "[ \"$#\" -eq 2 ] || exit 90\n\
             printf '%s\\n%s\\n%s\\n%s\\n' \"$1\" \"$2\" \
             \"$RETRODECK_ALPHA\" \"$RETRODECK_BETA\" > '{}'\n",
            capture.display()
        ));
        let arguments = [OsString::from("first argument"), OsString::from("second")];
        let environment = [
            environment_variable("RETRODECK_ALPHA", "alpha value"),
            environment_variable("RETRODECK_BETA", "beta"),
        ];
        let result = supervise_fixture(&child, &arguments, &environment, StopRequest::None);
        assert_eq!(result.exit_code, Some(0));
        let captured = std::fs::read_to_string(&capture).unwrap();
        assert_eq!(captured, "first argument\nsecond\nalpha value\nbeta\n");
        std::fs::remove_file(capture).unwrap();
        std::fs::remove_file(child).unwrap();
    }

    #[test]
    fn passes_the_configured_widget_key_to_the_child() {
        let key = "77c5424d-8dc4-4b22-a14b-d9ee90a2af76";
        let child = helper_script(&format!("[ \"$BMC_WIDGET_KEY\" = \"{key}\" ]"));
        let environment = [environment_variable(wayland::WIDGET_KEY_ENV, key)];
        let result = spawn_and_supervise(
            &child,
            &[],
            &environment,
            "keyed-widget",
            |_| StopRequest::None,
            Duration::from_millis(1),
            Duration::from_millis(1),
        );
        assert_eq!(result.exit_code, Some(0));
        std::fs::remove_file(child).unwrap();
    }

    #[test]
    fn classifies_clean_nonzero_signal_and_exec_failure() {
        assert_eq!(run_fixture("clean", StopRequest::None).exit_code, Some(0));
        assert_eq!(run_fixture("exit-7", StopRequest::None).exit_code, Some(7));
        let signaled = run_fixture("signal", StopRequest::None);
        assert_eq!(signaled.signal, Some(libc::SIGUSR1));
        let result = spawn_and_supervise(
            Path::new("/no/such/retrodeck-terminal"),
            &[],
            &[],
            "terminal",
            |_| StopRequest::None,
            Duration::from_millis(1),
            Duration::from_millis(1),
        );
        assert!(!result.started);
        assert!(result.error.unwrap().starts_with("cannot start terminal:"));
    }

    #[test]
    fn rejects_a_parent_that_changed_before_the_pre_exec_hook() {
        let (executable, arguments, environment) = fixture_command("clean", &[]);
        let mut command = Command::new(executable);
        command.args(arguments).envs(environment);
        unsafe {
            command.pre_exec(|| reset_child_process(0));
        }
        let error = command.spawn().expect_err("stale parent must prevent exec");
        assert_eq!(error.raw_os_error(), Some(libc::ESRCH));
    }

    #[test]
    fn kills_native_child_when_its_supervisor_is_sigkilled() {
        let path = temporary_path("parent-death-pid");
        let extra = [(
            OsString::from("RETRODECK_PROCESS_PIDS"),
            path.clone().into(),
        )];
        let (executable, arguments, environment) = fixture_command("supervisor", &extra);
        let mut supervisor = Command::new(executable)
            .args(arguments)
            .envs(environment)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let child_pid = loop {
            if let Ok(pid) = std::fs::read_to_string(&path)
                && let Ok(pid) = pid.parse::<libc::pid_t>()
            {
                break Some(pid);
            }
            if Instant::now() >= deadline {
                break None;
            }
            thread::sleep(Duration::from_millis(5));
        };
        // Always stop and reap the intermediate supervisor, including startup
        // failure. Its child deliberately ignores TERM to exercise SIGKILL.
        supervisor.kill().unwrap();
        let status = supervisor.wait().unwrap();
        let stopped = child_pid.is_some_and(|pid| {
            let deadline = Instant::now() + Duration::from_secs(2);
            while process_alive(pid) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            let stopped = !process_alive(pid);
            if !stopped {
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
            stopped
        });
        let _ = std::fs::remove_file(path);
        assert_eq!(status.signal(), Some(libc::SIGKILL));
        assert!(child_pid.is_some(), "child did not publish readiness");
        assert!(stopped, "child survived its supervisor's SIGKILL");
    }

    #[test]
    fn reports_shutdown_requests() {
        let result = run_fixture("wait", StopRequest::Shutdown);
        assert!(result.started);
        assert!(!result.exited_for_touch);
        assert!(result.shutdown_requested);
        assert_eq!(result.signal, Some(libc::SIGTERM));
    }

    fn run_stopped_group_fixture(action: &str) -> (ChildResult, Vec<libc::pid_t>) {
        let path = temporary_path(&format!("{action}-pids"));
        let extra = [(
            OsString::from("RETRODECK_PROCESS_PIDS"),
            path.clone().into(),
        )];
        let (executable, arguments, environment) = fixture_command(action, &extra);
        let started = Instant::now();
        let result = spawn_and_supervise(
            &executable,
            &arguments,
            &environment,
            "fixture",
            |timeout| {
                if path.exists() {
                    StopRequest::Touch
                } else {
                    thread::sleep(timeout);
                    assert!(
                        started.elapsed() < Duration::from_secs(2),
                        "{action} fixture did not publish process IDs"
                    );
                    StopRequest::None
                }
            },
            Duration::from_millis(5),
            Duration::from_millis(50),
        );
        let pids = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let pids = pids
            .split_whitespace()
            .map(|pid| pid.parse().unwrap())
            .collect();
        (result, pids)
    }

    fn assert_processes_exit(pids: &[libc::pid_t]) {
        for &pid in pids {
            let deadline = Instant::now() + Duration::from_secs(1);
            while process_alive(pid) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            assert!(!process_alive(pid), "process {pid} remained alive");
        }
    }

    #[test]
    fn terminates_complete_child_process_groups_after_touch() {
        for (action, signal) in [("leader-exits", libc::SIGTERM), ("group", libc::SIGKILL)] {
            let (result, pids) = run_stopped_group_fixture(action);
            assert!(result.started, "{action} fixture did not start");
            assert!(result.exited_for_touch, "{action} ignored touch");
            assert_eq!(result.signal, Some(signal), "{action} exit signal");
            assert_processes_exit(&pids);
        }
    }

    fn process_alive(pid: libc::pid_t) -> bool {
        let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return false;
        };
        status
            .rsplit_once(") ")
            .and_then(|(_, fields)| fields.chars().next())
            != Some('Z')
    }

    #[test]
    fn requires_an_uninterrupted_two_second_touch_hold() {
        let mut hold = TouchHold::new();
        hold.update(true, 12, 34);
        let started = hold.active_since.unwrap();
        assert!(!hold.complete(started + TOUCH_HOLD - Duration::from_millis(1)));
        hold.update(false, 12, 34);
        assert!(!hold.complete(started + TOUCH_HOLD));
        hold.update(true, 1279, 479);
        let restarted = hold.active_since.unwrap();
        assert!(hold.complete(restarted + TOUCH_HOLD));
    }
}
