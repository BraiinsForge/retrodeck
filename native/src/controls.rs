use crate::evdev_input::{
    AxisInfo, GAMEPAD_PRODUCT, GAMEPAD_VENDOR, GamepadSnapshot, gamepad_axes, gamepad_snapshot,
    open_device,
};
use evdev::raw_stream::RawDevice;
use evdev::{AbsoluteAxisCode, EventSummary, InputEvent, KeyCode, SynchronizationCode};
use rustix::event::{PollFd, PollFlags};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::io::{self, ErrorKind};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};

pub const KEYBOARD_REPORT: u32 = 0;
pub const GAMEPAD_REPORT: u32 = 1;
pub const KEYBOARD_SHIFT: u32 = 1;
pub const KEYBOARD_REPEAT: u32 = 2;

const GAMEPAD_X_NEGATIVE: u32 = 1 << 8;
const GAMEPAD_X_POSITIVE: u32 = 1 << 9;
const GAMEPAD_Y_NEGATIVE: u32 = 1 << 10;
const GAMEPAD_Y_POSITIVE: u32 = 1 << 11;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlReport {
    pub kind: u32,
    pub value: u32,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ControlAction {
    Ignore,
    Report(ControlReport),
    Resynchronize,
}

#[derive(Clone, Copy, Debug, Default)]
struct KeyboardState {
    left_shift: bool,
    right_shift: bool,
    dropping_events: bool,
}

impl KeyboardState {
    fn handle(&mut self, event: InputEvent) -> ControlAction {
        if self.dropping_events {
            return match event.destructure() {
                EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                    ControlAction::Resynchronize
                }
                _ => ControlAction::Ignore,
            };
        }

        match event.destructure() {
            EventSummary::Synchronization(_, SynchronizationCode::SYN_DROPPED, _) => {
                self.dropping_events = true;
                ControlAction::Ignore
            }
            EventSummary::Key(_, KeyCode::KEY_LEFTSHIFT, value) => {
                self.left_shift = value != 0;
                ControlAction::Ignore
            }
            EventSummary::Key(_, KeyCode::KEY_RIGHTSHIFT, value) => {
                self.right_shift = value != 0;
                ControlAction::Ignore
            }
            EventSummary::Key(_, code, value) => {
                let repeat = value == 2 && keyboard_key_repeats(code);
                if value != 1 && !repeat {
                    return ControlAction::Ignore;
                }
                let mut flags = 0;
                if self.left_shift || self.right_shift {
                    flags |= KEYBOARD_SHIFT;
                }
                if repeat {
                    flags |= KEYBOARD_REPEAT;
                }
                ControlAction::Report(ControlReport {
                    kind: KEYBOARD_REPORT,
                    value: u32::from(code.0),
                    flags,
                })
            }
            _ => ControlAction::Ignore,
        }
    }

    fn resynchronize(&mut self, left_shift: bool, right_shift: bool) {
        self.left_shift = left_shift;
        self.right_shift = right_shift;
        self.dropping_events = false;
    }
}

fn keyboard_key_repeats(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::KEY_UP | KeyCode::KEY_DOWN | KeyCode::KEY_LEFT | KeyCode::KEY_RIGHT
    )
}

#[derive(Clone, Copy, Debug, Default)]
struct GamepadState {
    snapshot: GamepadSnapshot,
    state: u32,
    dropping_events: bool,
}

impl GamepadState {
    #[cfg(test)]
    fn new(snapshot: GamepadSnapshot) -> Self {
        let mut state = Self::default();
        state.resynchronize(snapshot);
        state
    }

    fn handle(&mut self, event: InputEvent) -> ControlAction {
        if self.dropping_events {
            return match event.destructure() {
                EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                    ControlAction::Resynchronize
                }
                _ => ControlAction::Ignore,
            };
        }

        match event.destructure() {
            EventSummary::Synchronization(_, SynchronizationCode::SYN_DROPPED, _) => {
                self.dropping_events = true;
                ControlAction::Ignore
            }
            EventSummary::Key(_, code, value)
                if code.0 >= KeyCode::BTN_TRIGGER.0 && code.0 <= KeyCode::BTN_BASE2.0 =>
            {
                let bit = 1 << (code.0 - KeyCode::BTN_TRIGGER.0);
                if value != 0 {
                    self.snapshot.raw_buttons |= bit;
                } else {
                    self.snapshot.raw_buttons &= !bit;
                }
                ControlAction::Ignore
            }
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, value) => {
                self.snapshot.x.value = value;
                ControlAction::Ignore
            }
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, value) => {
                self.snapshot.y.value = value;
                ControlAction::Ignore
            }
            EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                let state = gamepad_state(self.snapshot);
                let pressed = state & !self.state;
                self.state = state;
                if pressed == 0 {
                    ControlAction::Ignore
                } else {
                    ControlAction::Report(ControlReport {
                        kind: GAMEPAD_REPORT,
                        value: pressed,
                        flags: 0,
                    })
                }
            }
            _ => ControlAction::Ignore,
        }
    }

    fn resynchronize(&mut self, snapshot: GamepadSnapshot) {
        self.snapshot = snapshot;
        self.state = gamepad_state(snapshot);
        self.dropping_events = false;
    }
}

fn gamepad_state(snapshot: GamepadSnapshot) -> u32 {
    snapshot.raw_buttons
        | axis_state(snapshot.x, GAMEPAD_X_NEGATIVE, GAMEPAD_X_POSITIVE)
        | axis_state(snapshot.y, GAMEPAD_Y_NEGATIVE, GAMEPAD_Y_POSITIVE)
}

fn axis_state(axis: AxisInfo, negative: u32, positive: u32) -> u32 {
    if axis.maximum <= axis.minimum {
        return 0;
    }
    let span = i64::from(axis.maximum) - i64::from(axis.minimum);
    let low = axis.minimum + (span / 3) as i32;
    let high = axis.maximum - (span / 3) as i32;
    if axis.value <= low {
        negative
    } else if axis.value >= high {
        positive
    } else {
        0
    }
}

const INPUT_DIRECTORY: &str = "/dev/input";
const MAXIMUM_GAMEPADS: usize = 2;
const MAXIMUM_KEYBOARDS: usize = 4;
const MAXIMUM_CONTROL_REPORTS: usize = 64;

struct GamepadDevice {
    path: PathBuf,
    physical_path: String,
    device: RawDevice,
    state: GamepadState,
}

impl GamepadDevice {
    fn from_device(path: &Path, device: RawDevice) -> Option<Self> {
        let (mut x, mut y) = gamepad_axes(&device).ok()?;
        x.value = 0;
        y.value = 0;
        let physical_path = device
            .physical_path()
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        Some(Self {
            path: path.to_owned(),
            physical_path,
            device,
            state: GamepadState {
                snapshot: GamepadSnapshot {
                    x,
                    y,
                    raw_buttons: 0,
                },
                state: 0,
                dropping_events: false,
            },
        })
    }

    fn resynchronize(&mut self) -> Result<(), String> {
        let snapshot = gamepad_snapshot(&self.device)
            .map_err(|error| format!("cannot resynchronize {}: {error}", self.path.display()))?;
        self.state.resynchronize(snapshot);
        Ok(())
    }

    fn drain(&mut self, reports: &mut VecDeque<ControlReport>) -> bool {
        loop {
            let events = match self.device.fetch_events() {
                Ok(events) => events.collect::<Vec<_>>(),
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == ErrorKind::WouldBlock => return true,
                Err(_) => return false,
            };
            if events.is_empty() {
                return false;
            }
            for event in events {
                match self.state.handle(event) {
                    ControlAction::Ignore => {}
                    ControlAction::Report(report) => queue_report(reports, report),
                    ControlAction::Resynchronize => {
                        if self.resynchronize().is_err() {
                            return false;
                        }
                    }
                }
            }
        }
    }
}

struct KeyboardDevice {
    path: PathBuf,
    device: RawDevice,
    state: KeyboardState,
    grabbed: bool,
}

impl KeyboardDevice {
    fn from_device(path: &Path, device: RawDevice) -> Self {
        Self {
            path: path.to_owned(),
            device,
            state: KeyboardState::default(),
            grabbed: false,
        }
    }

    fn activate(&mut self) {
        match self.device.grab() {
            Ok(()) => self.grabbed = true,
            Err(error) => eprintln!(
                "retrodeck: warning: cannot exclusively grab {}: {error}",
                self.path.display()
            ),
        }
        let _ = self.resynchronize();
    }

    fn resynchronize(&mut self) -> Result<(), String> {
        let (left_shift, right_shift) = keyboard_snapshot(&self.device)
            .map_err(|error| format!("cannot resynchronize {}: {error}", self.path.display()))?;
        self.state.resynchronize(left_shift, right_shift);
        Ok(())
    }

    fn drain(&mut self, reports: &mut VecDeque<ControlReport>) -> bool {
        loop {
            let events = match self.device.fetch_events() {
                Ok(events) => events.collect::<Vec<_>>(),
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == ErrorKind::WouldBlock => return true,
                Err(_) => return false,
            };
            if events.is_empty() {
                return false;
            }
            for event in events {
                match self.state.handle(event) {
                    ControlAction::Ignore => {}
                    ControlAction::Report(report) => queue_report(reports, report),
                    ControlAction::Resynchronize => {
                        if self.resynchronize().is_err() {
                            return false;
                        }
                    }
                }
            }
        }
    }
}

impl Drop for KeyboardDevice {
    fn drop(&mut self) {
        if self.grabbed {
            let _ = self.device.ungrab();
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct InputNode {
    path: PathBuf,
    device: u64,
    inode: u64,
    number: u64,
    changed: (i64, i64),
}

#[derive(Debug, Eq, PartialEq)]
struct InputCensus {
    include_keyboards: bool,
    nodes: Vec<InputNode>,
}

impl InputCensus {
    fn replaced(&self, next: &Self, path: &Path) -> bool {
        let at = |census: &Self| {
            census
                .nodes
                .binary_search_by(|node| node.path.as_path().cmp(path))
                .ok()
        };
        match (at(self), at(next)) {
            (Some(old), Some(new)) => self.nodes[old] != next.nodes[new],
            _ => false,
        }
    }
}
#[derive(Default)]
pub(crate) struct Controls {
    gamepads: Vec<GamepadDevice>,
    keyboards: Vec<KeyboardDevice>,
    reports: VecDeque<ControlReport>,
    census: Option<InputCensus>,
    rescan_requested: bool,
}

impl Controls {
    fn scan(&mut self, include_keyboards: bool) -> Result<(usize, usize), String> {
        self.scan_directory(Path::new(INPUT_DIRECTORY), include_keyboards)
    }

    fn scan_directory(
        &mut self,
        directory: &Path,
        include_keyboards: bool,
    ) -> Result<(usize, usize), String> {
        let nodes = match input_nodes(directory) {
            Ok(nodes) => nodes,
            Err(error) => {
                self.rescan_requested = true;
                return Err(error);
            }
        };
        let census = InputCensus {
            include_keyboards,
            nodes,
        };
        // Even dropping an otherwise idle evdev handle can wait for an RCU
        // grace period in the kernel. Do not reopen unchanged input nodes
        // on the dashboard thread just to check for hotplug.
        if !self.rescan_requested && self.census.as_ref() == Some(&census) {
            return Ok((self.gamepads.len(), self.keyboards.len()));
        }
        self.rescan_requested = false;
        let mut complete = true;
        let mut gamepads = Vec::new();
        let mut keyboards = Vec::new();
        for node in &census.nodes {
            let Some(device) = open_device(&node.path) else {
                complete = false;
                continue;
            };
            let identity = device.input_id();
            let gamepad =
                identity.vendor() == GAMEPAD_VENDOR && identity.product() == GAMEPAD_PRODUCT;
            let keyboard = include_keyboards && keyboard_capabilities(&device);
            if gamepad {
                if let Some(device) = GamepadDevice::from_device(&node.path, device) {
                    gamepads.push(device);
                } else {
                    complete = false;
                }
                if keyboard {
                    if let Some(device) = open_device(&node.path) {
                        keyboards.push(KeyboardDevice::from_device(&node.path, device));
                    } else {
                        complete = false;
                    }
                }
            } else if keyboard {
                keyboards.push(KeyboardDevice::from_device(&node.path, device));
            }
        }
        gamepads.sort_by(|left, right| {
            left.physical_path
                .cmp(&right.physical_path)
                .then_with(|| left.path.cmp(&right.path))
        });
        gamepads.truncate(MAXIMUM_GAMEPADS);
        keyboards.sort_by(|left, right| left.path.cmp(&right.path));
        keyboards.truncate(MAXIMUM_KEYBOARDS);

        let replaced = |path: &Path| {
            self.census
                .as_ref()
                .is_some_and(|old| old.replaced(&census, path))
        };
        let gamepads_changed = self.gamepads.len() != gamepads.len()
            || self.gamepads.iter().zip(&gamepads).any(|(left, right)| {
                left.path != right.path
                    || left.physical_path != right.physical_path
                    || replaced(&left.path)
            });
        if gamepads_changed {
            self.gamepads.clear();
            self.gamepads = gamepads;
            for gamepad in &mut self.gamepads {
                let _ = gamepad.resynchronize();
            }
        }

        let keyboards_changed = self.keyboards.len() != keyboards.len()
            || self
                .keyboards
                .iter()
                .zip(&keyboards)
                .any(|(left, right)| left.path != right.path || replaced(&left.path));
        if keyboards_changed {
            self.keyboards.clear();
            self.keyboards = keyboards;
            for keyboard in &mut self.keyboards {
                keyboard.activate();
            }
        }

        if gamepads_changed || keyboards_changed {
            self.reports.clear();
        }
        self.census = complete.then_some(census);
        Ok((self.gamepads.len(), self.keyboards.len()))
    }

    pub(crate) fn report_count(&self) -> usize {
        self.reports.len()
    }

    pub(crate) fn rescan_requested(&self) -> bool {
        self.rescan_requested
    }

    pub(crate) fn append_poll_descriptors<'a>(&'a self, descriptors: &mut Vec<PollFd<'a>>) {
        descriptors.extend(self.gamepads.iter().map(|device| {
            PollFd::from_borrowed_fd(
                device.device.as_fd(),
                PollFlags::IN | PollFlags::ERR | PollFlags::HUP,
            )
        }));
        descriptors.extend(self.keyboards.iter().map(|device| {
            PollFd::from_borrowed_fd(
                device.device.as_fd(),
                PollFlags::IN | PollFlags::ERR | PollFlags::HUP,
            )
        }));
    }

    pub(crate) fn read_ready(&mut self, ready: &[PollFlags]) {
        let first_keyboard = self.gamepads.len();
        let mut lost_gamepads = Vec::new();
        for (index, gamepad) in self.gamepads.iter_mut().enumerate() {
            let flags = ready.get(index).copied().unwrap_or(PollFlags::empty());
            if !flags.intersects(PollFlags::IN | PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL)
            {
                continue;
            }
            if flags.contains(PollFlags::IN) && gamepad.drain(&mut self.reports) {
                continue;
            }
            lost_gamepads.push(index);
        }
        for index in lost_gamepads.into_iter().rev() {
            self.gamepads.remove(index);
            self.rescan_requested = true;
        }

        let mut lost_keyboards = Vec::new();
        for (index, keyboard) in self.keyboards.iter_mut().enumerate() {
            let flags = ready
                .get(first_keyboard + index)
                .copied()
                .unwrap_or(PollFlags::empty());
            if !flags.intersects(PollFlags::IN | PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL)
            {
                continue;
            }
            if flags.contains(PollFlags::IN) && keyboard.drain(&mut self.reports) {
                continue;
            }
            lost_keyboards.push(index);
        }
        for index in lost_keyboards.into_iter().rev() {
            self.keyboards.remove(index);
            self.rescan_requested = true;
        }
    }
}

fn queue_report(reports: &mut VecDeque<ControlReport>, report: ControlReport) {
    if report.kind == GAMEPAD_REPORT {
        if let Some(current) = reports
            .iter_mut()
            .find(|current| current.kind == GAMEPAD_REPORT)
        {
            current.value |= report.value;
            return;
        }
    }
    if reports.len() < MAXIMUM_CONTROL_REPORTS && !reports.contains(&report) {
        reports.push_back(report);
    }
}

thread_local! {
    static CONTROLS: RefCell<Controls> = RefCell::new(Controls::default());
}

pub(crate) fn with_controls<T>(function: impl FnOnce(&mut Controls) -> T) -> T {
    CONTROLS.with(|controls| function(&mut controls.borrow_mut()))
}

pub fn scan() -> Result<(usize, usize), String> {
    with_controls(|controls| controls.scan(true))
}

pub fn scan_gamepads() -> Result<usize, String> {
    with_controls(|controls| {
        *controls = Controls::default();
        controls.scan(false).map(|counts| counts.0)
    })
}

pub fn close() {
    CONTROLS.with(|controls| *controls.borrow_mut() = Controls::default());
}

pub fn next_report() -> Option<ControlReport> {
    CONTROLS.with(|controls| controls.borrow_mut().reports.pop_front())
}

fn input_nodes(directory: &Path) -> Result<Vec<InputNode>, String> {
    use std::os::unix::fs::MetadataExt;
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot open {}: {error}", directory.display()))?;
    let mut nodes = Vec::new();
    for path in entries
        .filter_map(Result::ok)
        .filter_map(|entry| event_path(entry.path()))
    {
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        nodes.push(InputNode {
            path,
            device: meta.dev(),
            inode: meta.ino(),
            number: meta.rdev(),
            changed: (meta.ctime(), meta.ctime_nsec()),
        });
    }
    nodes.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    Ok(nodes)
}

fn event_path(path: PathBuf) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    let suffix = name.strip_prefix("event")?;
    (!suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())).then_some(path)
}

fn keyboard_capabilities(device: &RawDevice) -> bool {
    let Some(keys) = device.supported_keys() else {
        return false;
    };
    keys.contains(KeyCode::KEY_ENTER)
        && keys.contains(KeyCode::KEY_ESC)
        && keys.contains(KeyCode::KEY_TAB)
        && keys.contains(KeyCode::KEY_UP)
        && keys.contains(KeyCode::KEY_DOWN)
        && keys.contains(KeyCode::KEY_LEFT)
        && keys.contains(KeyCode::KEY_RIGHT)
        && (keys.contains(KeyCode::KEY_LEFTSHIFT) || keys.contains(KeyCode::KEY_RIGHTSHIFT))
}

fn keyboard_snapshot(device: &RawDevice) -> io::Result<(bool, bool)> {
    let keys = device.get_key_state()?;
    Ok((
        keys.contains(KeyCode::KEY_LEFTSHIFT),
        keys.contains(KeyCode::KEY_RIGHTSHIFT),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::EventType;
    #[test]
    fn idle_scans_reuse_the_input_census_but_hotplug_rechecks_nodes() {
        let dir = crate::test_support::fixture_directory("controls-census");
        // Fake nodes cannot be opened as evdev devices. This distinguishes
        // a directory-only check from a scan that tries to probe them.
        std::fs::write(dir.join("event1"), []).unwrap();
        let mut controls = Controls {
            census: Some(InputCensus {
                include_keyboards: true,
                nodes: input_nodes(&dir).unwrap(),
            }),
            ..Controls::default()
        };
        assert_eq!(controls.scan_directory(&dir, true).unwrap(), (0, 0));
        assert!(controls.census.is_some());
        std::fs::write(dir.join("event2"), []).unwrap();
        controls.scan_directory(&dir, true).unwrap();
        assert!(controls.census.is_none(), "a new device must be probed");
        controls.census = Some(InputCensus {
            include_keyboards: true,
            nodes: input_nodes(&dir).unwrap(),
        });
        controls.rescan_requested = true;
        controls.scan_directory(&dir, true).unwrap();
        assert!(
            controls.census.is_none(),
            "device loss must trigger a fresh probe"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn event(event_type: EventType, code: u16, value: i32) -> InputEvent {
        InputEvent::new(event_type.0, code, value)
    }

    fn key(code: KeyCode, value: i32) -> InputEvent {
        event(EventType::KEY, code.0, value)
    }

    fn syn(code: SynchronizationCode) -> InputEvent {
        event(EventType::SYNCHRONIZATION, code.0, 0)
    }

    fn report_boundary() -> InputEvent {
        syn(SynchronizationCode::SYN_REPORT)
    }

    macro_rules! assert_handle {
        ($state:expr, $event:expr, $expected:expr) => {
            assert_eq!($state.handle($event), $expected)
        };
    }

    fn axis(code: AbsoluteAxisCode, value: i32) -> InputEvent {
        event(EventType::ABSOLUTE, code.0, value)
    }

    fn axis_info(minimum: i32, maximum: i32, value: i32) -> AxisInfo {
        AxisInfo {
            minimum,
            maximum,
            value,
        }
    }

    fn report(kind: u32, value: u32, flags: u32) -> ControlReport {
        ControlReport { kind, value, flags }
    }

    fn keyboard_report(code: KeyCode, flags: u32) -> ControlAction {
        ControlAction::Report(report(KEYBOARD_REPORT, u32::from(code.0), flags))
    }

    fn assert_keyboard_report(state: &mut KeyboardState, code: KeyCode, value: i32, flags: u32) {
        assert_eq!(state.handle(key(code, value)), keyboard_report(code, flags));
    }

    fn gamepad_report(value: u32) -> ControlAction {
        ControlAction::Report(report(GAMEPAD_REPORT, value, 0))
    }

    fn gamepad_snapshot(x: i32, y: i32, raw_buttons: u32) -> GamepadSnapshot {
        GamepadSnapshot {
            x: axis_info(0, 255, x),
            y: axis_info(0, 255, y),
            raw_buttons,
        }
    }

    #[test]
    fn keyboard_tracks_shift_and_accepts_only_arrow_repeats() {
        let mut state = KeyboardState::default();
        assert_keyboard_report(&mut state, KeyCode::KEY_TAB, 1, 0);
        assert_handle!(state, key(KeyCode::KEY_LEFTSHIFT, 1), ControlAction::Ignore);
        assert_keyboard_report(&mut state, KeyCode::KEY_TAB, 1, KEYBOARD_SHIFT);
        assert_handle!(state, key(KeyCode::KEY_TAB, 2), ControlAction::Ignore);
        let shifted_repeat = KEYBOARD_SHIFT | KEYBOARD_REPEAT;
        assert_keyboard_report(&mut state, KeyCode::KEY_RIGHT, 2, shifted_repeat);
        state.handle(key(KeyCode::KEY_RIGHTSHIFT, 1));
        state.handle(key(KeyCode::KEY_LEFTSHIFT, 0));
        assert_keyboard_report(&mut state, KeyCode::KEY_ENTER, 1, KEYBOARD_SHIFT);
        state.handle(key(KeyCode::KEY_RIGHTSHIFT, 0));
        assert_handle!(state, key(KeyCode::KEY_ENTER, 0), ControlAction::Ignore);
    }

    #[test]
    fn keyboard_drop_waits_for_report_and_resynchronizes_without_an_edge() {
        let mut state = KeyboardState::default();
        let dropped = syn(SynchronizationCode::SYN_DROPPED);
        assert_handle!(state, dropped, ControlAction::Ignore);
        assert_handle!(state, key(KeyCode::KEY_LEFTSHIFT, 1), ControlAction::Ignore);
        assert_handle!(state, key(KeyCode::KEY_TAB, 1), ControlAction::Ignore);
        assert_handle!(state, report_boundary(), ControlAction::Resynchronize);
        state.resynchronize(false, true);
        assert_keyboard_report(&mut state, KeyCode::KEY_TAB, 1, KEYBOARD_SHIFT);
    }

    #[test]
    fn gamepad_uses_exact_thirds_and_reports_rising_edges_on_syn_report() {
        let state = |value| axis_state(axis_info(0, 255, value), 1, 2);
        assert_eq!([84, 85, 86, 169, 170].map(state), [1, 1, 0, 0, 2]);
        assert_eq!(axis_state(axis_info(4, 4, 4), 1, 2), 0);

        let mut state = GamepadState::new(gamepad_snapshot(127, 127, 0));
        let thumb2 = 1 << (KeyCode::BTN_THUMB2.0 - KeyCode::BTN_TRIGGER.0);
        state.handle(axis(AbsoluteAxisCode::ABS_X, 255));
        state.handle(key(KeyCode::BTN_THUMB2, 1));
        assert_handle!(
            state,
            report_boundary(),
            gamepad_report(GAMEPAD_X_POSITIVE | thumb2)
        );
        assert_handle!(state, report_boundary(), ControlAction::Ignore);
        state.handle(key(KeyCode::BTN_THUMB2, 0));
        state.handle(report_boundary());
        state.handle(key(KeyCode::BTN_THUMB2, 1));
        assert_handle!(state, report_boundary(), gamepad_report(thumb2));
    }

    #[test]
    fn keyboard_emits_unknown_presses_but_filters_non_arrow_repeats() {
        let mut state = KeyboardState::default();
        assert_keyboard_report(&mut state, KeyCode::KEY_KPENTER, 1, 0);
        assert_handle!(state, key(KeyCode::KEY_KPENTER, 2), ControlAction::Ignore);
        assert_keyboard_report(&mut state, KeyCode::KEY_A, 1, 0);
    }

    #[test]
    fn report_queue_merges_gamepads_deduplicates_keys_and_stays_bounded() {
        let mut reports = VecDeque::new();
        queue_report(&mut reports, report(GAMEPAD_REPORT, 1, 0));
        queue_report(&mut reports, report(GAMEPAD_REPORT, 4, 0));
        let key_report = report(KEYBOARD_REPORT, 28, 0);
        queue_report(&mut reports, key_report);
        queue_report(&mut reports, key_report);
        for value in 0..100 {
            queue_report(
                &mut reports,
                report(KEYBOARD_REPORT, value, KEYBOARD_REPEAT),
            );
        }
        assert_eq!(reports.len(), MAXIMUM_CONTROL_REPORTS);
        assert_eq!(reports.front(), Some(&report(GAMEPAD_REPORT, 5, 0)));
        assert_eq!(
            reports
                .iter()
                .filter(|report| **report == key_report)
                .count(),
            1
        );
    }

    #[test]
    fn event_nodes_require_a_numeric_suffix() {
        assert_eq!(
            event_path(PathBuf::from("/dev/input/event12")),
            Some(PathBuf::from("/dev/input/event12"))
        );
        for path in [
            "/dev/input/event",
            "/dev/input/event2a",
            "/dev/input/mouse0",
        ] {
            assert_eq!(event_path(PathBuf::from(path)), None);
        }
    }

    #[test]
    fn gamepad_preserves_all_eight_raw_button_edges() {
        for index in 0..8 {
            let mut state = GamepadState::new(gamepad_snapshot(127, 127, 0));
            state.handle(key(KeyCode(KeyCode::BTN_TRIGGER.0 + index), 1));
            assert_handle!(state, report_boundary(), gamepad_report(1 << index));
        }
        let mut state = GamepadState::new(gamepad_snapshot(127, 127, 0));
        state.handle(key(KeyCode::BTN_DEAD, 1));
        assert_handle!(state, report_boundary(), ControlAction::Ignore);
    }

    #[test]
    fn gamepad_drop_uses_snapshot_without_synthesizing_an_edge() {
        let mut state = GamepadState::new(gamepad_snapshot(127, 127, 0));
        state.handle(syn(SynchronizationCode::SYN_DROPPED));
        state.handle(axis(AbsoluteAxisCode::ABS_Y, 255));
        state.handle(key(KeyCode::BTN_BASE, 1));
        assert_handle!(state, report_boundary(), ControlAction::Resynchronize);
        state.resynchronize(gamepad_snapshot(127, 255, 1 << 6));
        assert_handle!(state, report_boundary(), ControlAction::Ignore);
        state.handle(axis(AbsoluteAxisCode::ABS_Y, 127));
        state.handle(report_boundary());
        state.handle(axis(AbsoluteAxisCode::ABS_Y, 255));
        assert_handle!(state, report_boundary(), gamepad_report(GAMEPAD_Y_POSITIVE));
    }
}
