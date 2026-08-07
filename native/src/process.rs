use crate::{fbdev, input, wayland};
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, IntoRawFd, OwnedFd, RawFd};
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
    pub signal: Option<i32>,
    pub error: Option<String>,
}

pub fn run_helper(executable: &Path, input: &[u8]) -> HelperResult {
    let mut child = match Command::new(executable).stdin(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(error) => {
            return HelperResult {
                phase: HelperPhase::Start,
                exit_code: None,
                signal: None,
                error: Some(error.to_string()),
            };
        }
    };
    let input_error = child
        .stdin
        .take()
        .map_or_else(
            || Some(io::Error::other("helper stdin is unavailable")),
            |mut stdin| {
                let write_error = stdin.write_all(input).err();
                let descriptor = stdin.into_raw_fd();
                let close_error =
                    (unsafe { libc::close(descriptor) } != 0).then(io::Error::last_os_error);
                write_error.or(close_error)
            },
        )
        .map(|error| error.to_string());
    let (exit_code, signal, wait_error) = match child.wait() {
        Ok(status) => (status.code(), status.signal(), None),
        Err(error) => (None, None, Some(error.to_string())),
    };
    if let Some(error) = input_error {
        return HelperResult {
            phase: HelperPhase::Input,
            exit_code,
            signal,
            error: Some(error),
        };
    }
    if let Some(error) = wait_error {
        return HelperResult {
            phase: HelperPhase::Wait,
            exit_code: None,
            signal: None,
            error: Some(error),
        };
    }
    HelperResult {
        phase: HelperPhase::Complete,
        exit_code,
        signal,
        error: None,
    }
}

fn widget_handoff_requested(environment: &[(OsString, OsString)]) -> bool {
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
    let widget_handoff = if widget_handoff_requested(environment) {
        if !interaction.uses_wayland {
            return ChildResult {
                error: Some("BMC widget launch requires an open dashboard widget".to_owned()),
                ..ChildResult::default()
            };
        }
        match wayland::connect_child_widget() {
            Ok(handoff) => Some(handoff),
            Err(error) => {
                return ChildResult {
                    error: Some(error),
                    ..ChildResult::default()
                };
            }
        }
    } else {
        None
    };
    let tty = TtySnapshot::capture();
    eprintln!("retrodeck: launching {label}");
    let result = spawn_and_supervise(
        executable,
        arguments,
        environment,
        widget_handoff,
        label,
        |timeout| interaction.step(timeout),
        POLL_INTERVAL,
        TERM_GRACE,
    );
    tty.restore();
    result
}

pub fn run_terminal(executable: &Path, keymap: &OsStr, mode: &OsStr, label: &str) -> ChildResult {
    let arguments = [OsString::from(mode)];
    let environment = [(OsString::from("RETRO_DECK_KEYMAP"), OsString::from(keymap))];
    run_child(executable, &arguments, &environment, label, true, true)
}

fn spawn_and_supervise<F>(
    executable: &Path,
    arguments: &[OsString],
    environment: &[(OsString, OsString)],
    widget_handoff: Option<OwnedFd>,
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
    let handoff_fd = widget_handoff.as_ref().map(AsRawFd::as_raw_fd);
    if handoff_fd.is_some() {
        command.env(
            wayland::WIDGET_HANDOFF_FD_ENV,
            wayland::WIDGET_HANDOFF_FD.to_string(),
        );
    }
    // signal(2), setpgid(2), fcntl(2), and dup2(2) are async-signal-safe;
    // the hook only reports errno.
    unsafe {
        command.pre_exec(move || reset_child_process(handoff_fd));
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
    drop(widget_handoff);
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

fn reset_child_process(widget_handoff: Option<RawFd>) -> io::Result<()> {
    for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP, libc::SIGPIPE] {
        if unsafe { libc::signal(signal, libc::SIG_DFL) } == libc::SIG_ERR {
            return Err(io::Error::last_os_error());
        }
    }
    if unsafe { libc::setpgid(0, 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if let Some(source) = widget_handoff {
        if source != wayland::WIDGET_HANDOFF_FD
            && unsafe { libc::dup2(source, wayland::WIDGET_HANDOFF_FD) } == -1
        {
            return Err(io::Error::last_os_error());
        }
        let flags = unsafe { libc::fcntl(wayland::WIDGET_HANDOFF_FD, libc::F_GETFD) };
        if flags == -1
            || unsafe {
                libc::fcntl(
                    wayland::WIDGET_HANDOFF_FD,
                    libc::F_SETFD,
                    flags & !libc::FD_CLOEXEC,
                )
            } == -1
        {
            return Err(io::Error::last_os_error());
        }
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
    use HelperPhase::{Complete, Input, Start};
    use std::env;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

    type HelperOutcome = (HelperPhase, Option<i32>, Option<i32>, bool);

    const HELPER_EXIT_7: HelperOutcome = (Complete, Some(7), None, false);
    const HELPER_SIGTERM: HelperOutcome = (Complete, None, Some(libc::SIGTERM), false);
    const HELPER_INPUT_EXIT_7: HelperOutcome = (Input, Some(7), None, true);

    fn assert_script(body: &str, input: &[u8], expected: HelperOutcome) {
        let helper = helper_script(body);
        let result = run_helper(&helper, input);
        std::fs::remove_file(helper).unwrap();
        assert_eq!(result.phase, expected.0);
        assert_eq!(result.exit_code, expected.1);
        assert_eq!(result.signal, expected.2);
        assert_eq!(result.error.is_some(), expected.3);
    }

    #[test]
    fn helper_writes_exact_input_and_classifies_failures() {
        let capture = temporary_path("helper-input");
        let body = format!("cat > '{}'\n", capture.display());
        let helper = helper_script(&body);
        let input = b"test net\nsecret!9\n";
        let result = run_helper(&helper, input);
        assert_eq!(result.phase, Complete);
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
            None,
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
    fn hands_the_registered_widget_connection_to_the_child() {
        let (handoff, _peer) = UnixStream::pair().unwrap();
        let handoff: OwnedFd = handoff.into();
        let child = helper_script(
            "[ \"$RETRO_DECK_WIDGET_FD\" = \"9\" ] && [ -S /proc/self/fd/9 ]",
        );
        let result = spawn_and_supervise(
            &child,
            &[],
            &[],
            Some(handoff),
            "widget-handoff",
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
            None,
            "terminal",
            |_| StopRequest::None,
            Duration::from_millis(1),
            Duration::from_millis(1),
        );
        assert!(!result.started);
        assert!(result.error.unwrap().starts_with("cannot start terminal:"));
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
            None,
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
