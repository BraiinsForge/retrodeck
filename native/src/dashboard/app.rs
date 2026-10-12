//! Deterministic dashboard state transitions. Native I/O belongs in `runtime`.
use super::{credits, menu, policy, settings, wifi};
use crate::{controls, input::TouchReport, network::NetworkStatus, process::ChildResult};

pub const REBOOT_CONFIRMATION: &str = "PRESS A OR TAP AGAIN TO REBOOT";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    Dashboard,
    Settings,
    Wifi,
    Credits,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cue {
    Volume,
    Previous,
    Next,
    Confirm,
    Back,
}
impl From<settings::Cue> for Cue {
    fn from(c: settings::Cue) -> Self {
        match c {
            settings::Cue::Previous => Self::Previous,
            settings::Cue::Next => Self::Next,
            settings::Cue::Confirm => Self::Confirm,
            settings::Cue::Back => Self::Back,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Back,
    Settings,
    SystemPrevious,
    SystemNext,
    Previous,
    Next,
    Confirm,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Back,
    Settings,
    SystemPrevious,
    SystemNext,
    Left,
    Right,
    Up,
    Down,
    Confirm,
}

/// Reports are merged once per readiness snapshot, not once per button.
pub fn collect_actions(reports: &[controls::ControlReport]) -> (Vec<Action>, Vec<Action>) {
    let mut gamepad = Vec::new();
    let mut keyboard = Vec::new();
    let mut add = |kind: u32, action: Action| {
        let list = if kind == controls::GAMEPAD_REPORT {
            &mut gamepad
        } else {
            &mut keyboard
        };
        if !list.contains(&action) {
            list.push(action);
        }
    };
    for report in reports {
        if report.kind == controls::KEYBOARD_REPORT {
            let action = match report.value {
                1 => Some(Action::Back),
                15 => Some(if report.flags & controls::KEYBOARD_SHIFT != 0 {
                    Action::SystemPrevious
                } else {
                    Action::SystemNext
                }),
                28 | 96 => Some(Action::Confirm),
                103 => Some(Action::Up),
                105 => Some(Action::Left),
                106 => Some(Action::Right),
                108 => Some(Action::Down),
                _ => None,
            };
            if let Some(action) = action {
                add(report.kind, action);
            }
        } else if report.kind == controls::GAMEPAD_REPORT {
            for (mask, action) in [
                (1, Action::Back),
                (2, Action::Back),
                (4, Action::Confirm),
                (8, Action::Confirm),
                (16, Action::SystemPrevious),
                (32, Action::SystemNext),
                (64, Action::Settings),
                (256, Action::Left),
                (512, Action::Right),
                (1024, Action::Up),
                (2048, Action::Down),
            ] {
                if report.value & mask != 0 {
                    add(report.kind, action);
                }
            }
        }
    }
    (gamepad, keyboard)
}
pub fn command(actions: &[Action], view: View) -> Option<Command> {
    let has = |a| actions.contains(&a);
    if view != View::Dashboard && has(Action::Back) {
        Some(Command::Back)
    } else if matches!(view, View::Wifi | View::Credits) {
        None
    } else if has(Action::Settings) {
        Some(Command::Settings)
    } else if view != View::Settings && has(Action::SystemPrevious) {
        Some(Command::SystemPrevious)
    } else if view != View::Settings && has(Action::SystemNext) {
        Some(Command::SystemNext)
    } else if has(Action::Left) || has(Action::Up) {
        Some(Command::Previous)
    } else if has(Action::Right) || has(Action::Down) {
        Some(Command::Next)
    } else if has(Action::Confirm) {
        Some(Command::Confirm)
    } else {
        None
    }
}
#[derive(Clone, Debug)]
pub struct LaunchRequest {
    pub game: GameIndex,
    pub mode: Option<String>,
    pub touch_batch: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameIndex {
    Catalog(usize),
    Terminal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Recovery {
    Controls,
    Presentation,
    Volume,
}
#[derive(Debug)]
pub struct ActiveLaunch {
    pub game: policy::Game,
    pub plan: policy::LaunchPlan,
    pub result: Option<ChildResult>,
    pub recovery: Option<Recovery>,
}
#[derive(Debug)]
pub enum Pending {
    Launch(LaunchRequest),
    Settings(settings::Effect),
    VolumeTone(settings::Effect),
    Wifi,
    Network,
    Child(ActiveLaunch),
}
#[derive(Debug)]
pub enum Effect {
    Render,
    Present,
    Cue(Cue),
    VolumeCue,
    StopSound,
    FinishSound,
    ReapSound,
    ScanControls,
    CloseControls,
    ReconnectTouch,
    OpenPresentation,
    DiscardTouch,
    ControllerSuspended,
    ControllerResumed,
    SettingsAction(settings::Effect),
    WifiAction,
    NetworkAction,
    PrepareLaunch,
    Launch,
    ReloadVolume,
    StopLoop,
}
#[derive(Debug)]
pub enum Event {
    Begin {
        now: u64,
        wayland: bool,
        rescan: bool,
    },
    Tick(u64),
    Controls {
        now: u64,
        gamepad: Vec<Action>,
        keyboard: Vec<Action>,
        quarantined: bool,
    },
    Touch {
        now: u64,
        report: TouchReport,
    },
    TouchLost,
    TouchReconnected,
    SettingsResult(bool),
    VolumeToneResult(bool),
    WifiResult {
        succeeded: bool,
        failure_status: Option<String>,
    },
    NetworkResult(NetworkStatus),
    Prepared {
        game: policy::Game,
        plan: policy::LaunchPlan,
    },
    ChildReturned {
        result: ChildResult,
        touch_disconnected: bool,
    },
    ControlsRescanned(u64),
    PresentationOpened,
    ChildComplete(Option<u8>),
}
#[derive(Debug)]
pub struct App {
    pub games: Vec<policy::Game>,
    pub view: View,
    pub system: policy::System,
    pub position: usize,
    pub status: String,
    pub pressed: Option<menu::Target>,
    pub settings: settings::State,
    pub wifi: wifi::State,
    pub credits: credits::Credits,
    pub network: NetworkStatus,
    pub reduced_motion: bool,
    pub credits_started_at: u64,
    pub credits_rendered_at: u64,
    pub guard: policy::ControllerGuard,
    pub last_scan: Option<u64>,
    pub touch_connected: bool,
    pub last_touch_reconnect: u64,
    pub network_refreshed_at: u64,
    pub reboot_deadline: u64,
    pub pending: Option<Pending>,
}
fn screen(cue: Option<Cue>) -> Vec<Effect> {
    let mut e = vec![Effect::Render, Effect::Present];
    if let Some(c) = cue {
        e.push(Effect::Cue(c));
    }
    e
}
impl App {
    pub fn new(games: Vec<policy::Game>, credits: credits::Credits, reduced_motion: bool) -> Self {
        let system = populated_systems(&games)
            .first()
            .copied()
            .unwrap_or(policy::System::Deck);
        let mut settings = settings::State::default();
        settings.open = false;
        let mut wifi = wifi::State::default();
        wifi.open = false;
        Self {
            games,
            view: View::Dashboard,
            system,
            position: 0,
            status: String::new(),
            pressed: None,
            settings,
            wifi,
            credits,
            network: NetworkStatus::default(),
            reduced_motion,
            credits_started_at: 0,
            credits_rendered_at: 0,
            guard: policy::ControllerGuard::new(),
            last_scan: None,
            touch_connected: true,
            last_touch_reconnect: 0,
            network_refreshed_at: 0,
            reboot_deadline: 0,
            pending: None,
        }
    }
    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
        self.settings.status = self.status.clone();
    }
    pub fn clear_pressed(&mut self) {
        self.pressed = None;
        self.settings.pressed = None;
        self.wifi.pressed = None;
        self.credits.clear_pressed();
    }
    fn set_view(&mut self, view: View) {
        self.clear_pressed();
        self.view = view;
        self.settings.open = matches!(view, View::Settings | View::Wifi);
        self.wifi.open = view == View::Wifi;
    }
    fn cancel_reboot(&mut self) {
        self.reboot_deadline = 0;
        if self.status == REBOOT_CONFIRMATION {
            self.set_status("");
        }
    }
    pub fn selected_game(&self) -> Option<usize> {
        let indices = self.indices();
        indices.get(self.position % indices.len().max(1)).copied()
    }
    fn indices(&self) -> Vec<usize> {
        self.games
            .iter()
            .enumerate()
            .filter_map(|(i, g)| (g.system == self.system).then_some(i))
            .collect()
    }
    pub fn pending_application(&self) -> Result<policy::Game, String> {
        let Some(Pending::Launch(request)) = &self.pending else {
            return Err("Dashboard has no pending launch".into());
        };
        let mut game = match request.game {
            GameIndex::Catalog(index) => self
                .games
                .get(index)
                .cloned()
                .ok_or("Invalid launch index")?,
            GameIndex::Terminal => policy::builtins()
                .into_iter()
                .find(|g| g.id == "terminal")
                .ok_or("Terminal application missing")?,
        };
        if let Some(mode) = &request.mode {
            game.terminal_mode = Some(mode.clone());
        }
        Ok(game)
    }
    fn request_game(&mut self, index: usize, now: u64, touch_batch: bool) -> Vec<Effect> {
        let Some(game) = self.games.get(index) else {
            return Vec::new();
        };
        if game.id == "reboot" {
            if self.reboot_deadline > 0 && now < self.reboot_deadline {
                self.reboot_deadline = 0;
            } else {
                self.reboot_deadline = now.saturating_add(4000);
                self.set_status(REBOOT_CONFIRMATION);
                return screen(Some(Cue::Confirm));
            }
        } else {
            self.cancel_reboot();
        }
        self.pending = Some(Pending::Launch(LaunchRequest {
            game: GameIndex::Catalog(index),
            mode: None,
            touch_batch,
        }));
        Vec::new()
    }
    fn settings_plan(&mut self, plan: settings::Effect, touch_batch: bool) -> Vec<Effect> {
        match plan.action {
            settings::Action::Close => {
                self.set_view(View::Dashboard);
                self.set_status("");
                screen(plan.cue.map(Into::into))
            }
            settings::Action::Terminal => {
                self.pending = Some(Pending::Launch(LaunchRequest {
                    game: GameIndex::Terminal,
                    mode: Some("shell".into()),
                    touch_batch,
                }));
                plan.cue
                    .map(|c| vec![Effect::Cue(c.into())])
                    .unwrap_or_default()
            }
            settings::Action::Wifi => {
                wifi::open(&mut self.wifi);
                self.set_view(View::Wifi);
                screen(plan.cue.map(Into::into))
            }
            _ => {
                self.pending = Some(Pending::Settings(plan.clone()));
                vec![Effect::SettingsAction(plan)]
            }
        }
    }
    fn command(&mut self, command: Command, now: u64) -> Vec<Effect> {
        match command {
            Command::Back => {
                self.cancel_reboot();
                match self.view {
                    View::Credits | View::Settings => {
                        self.set_view(View::Dashboard);
                        self.set_status("");
                    }
                    View::Wifi => {
                        self.set_view(View::Settings);
                        self.set_status("WIFI EDITOR CLOSED");
                    }
                    _ => {}
                }
                screen(Some(Cue::Back))
            }
            Command::Settings => {
                self.cancel_reboot();
                let opening = self.view != View::Settings;
                if opening {
                    self.settings.selected = settings::Target::VolumeDown;
                }
                self.set_view(if opening {
                    View::Settings
                } else {
                    View::Dashboard
                });
                self.set_status("");
                screen(Some(if opening { Cue::Confirm } else { Cue::Back }))
            }
            Command::SystemPrevious | Command::SystemNext => {
                self.cancel_reboot();
                let systems = populated_systems(&self.games);
                if !systems.is_empty() {
                    let p = systems.iter().position(|s| *s == self.system).unwrap_or(0);
                    self.system = systems[if command == Command::SystemPrevious {
                        (p + systems.len() - 1) % systems.len()
                    } else {
                        (p + 1) % systems.len()
                    }];
                }
                self.position = 0;
                self.set_status("");
                screen(Some(if command == Command::SystemPrevious {
                    Cue::Previous
                } else {
                    Cue::Next
                }))
            }
            Command::Previous | Command::Next => {
                self.cancel_reboot();
                if self.view == View::Settings {
                    let c = settings::move_selection(
                        &mut self.settings,
                        if command == Command::Previous {
                            settings::Direction::Previous
                        } else {
                            settings::Direction::Next
                        },
                    );
                    self.set_status(self.settings.status.clone());
                    screen(Some(c.into()))
                } else {
                    let count = self.indices().len();
                    if count > 0 {
                        self.position = if command == Command::Previous {
                            (self.position + count - 1) % count
                        } else {
                            (self.position + 1) % count
                        };
                    }
                    self.set_status("");
                    screen((count > 0).then_some(if command == Command::Previous {
                        Cue::Previous
                    } else {
                        Cue::Next
                    }))
                }
            }
            Command::Confirm => {
                if self.view == View::Settings {
                    self.settings_plan(
                        settings::activate(&self.settings, self.settings.selected),
                        false,
                    )
                } else if let Some(index) = self.selected_game() {
                    let mut effects = self.request_game(index, now, false);
                    effects.push(Effect::Cue(Cue::Confirm));
                    effects
                } else {
                    Vec::new()
                }
            }
        }
    }
    fn dashboard_touch(
        &mut self,
        report: TouchReport,
        layout: &menu::MenuLayout,
        now: u64,
    ) -> Vec<Effect> {
        let target = layout.target_at(report.x, report.y);
        if report.pressed {
            self.pressed = target;
        }
        if !report.released {
            return Vec::new();
        }
        let pressed = self.pressed.take();
        let reboot = matches!(target,Some(menu::Target::Game(i)) if self.games.get(i).is_some_and(|g|g.id=="reboot"));
        if self.reboot_deadline > 0 && !reboot {
            self.cancel_reboot();
        }
        if target != pressed {
            return Vec::new();
        }
        match target {
            Some(menu::Target::Credits) => {
                self.credits_started_at = now;
                self.set_view(View::Credits);
                self.set_status("");
                screen(Some(Cue::Confirm))
            }
            Some(menu::Target::Settings) => {
                self.settings.selected = settings::Target::VolumeDown;
                self.set_view(View::Settings);
                self.set_status("");
                screen(Some(Cue::Confirm))
            }
            Some(menu::Target::Game(index)) => {
                let mut e = self.request_game(index, now, true);
                e.push(Effect::Cue(Cue::Confirm));
                e
            }
            Some(menu::Target::System(system)) => {
                let moved = self.system != system;
                self.system = system;
                self.position = 0;
                self.set_status("");
                screen(moved.then_some(Cue::Next))
            }
            Some(menu::Target::Previous) => self.command(Command::Previous, now),
            Some(menu::Target::Next) => self.command(Command::Next, now),
            None => Vec::new(),
        }
    }
    /// The reducer rejects out-of-order completions instead of quietly losing a pending action.
    fn check_pending(&self, event: &Event) -> Result<(), String> {
        let matches = match (&self.pending, event) {
            (None, _) => true,
            (Some(Pending::Launch(_)), Event::Prepared { .. }) => true,
            (Some(Pending::Launch(request)), Event::Touch { .. }) if request.touch_batch => true,
            (Some(Pending::Settings(_)), Event::SettingsResult(_)) => true,
            (Some(Pending::VolumeTone(_)), Event::VolumeToneResult(_)) => true,
            (Some(Pending::Wifi), Event::WifiResult { .. }) => true,
            (Some(Pending::Network), Event::NetworkResult(_)) => true,
            (Some(Pending::Child(active)), event) => matches!(
                (active.recovery, event),
                (None, Event::ChildReturned { .. })
                    | (Some(Recovery::Controls), Event::ControlsRescanned(_))
                    | (Some(Recovery::Presentation), Event::PresentationOpened)
                    | (Some(Recovery::Volume), Event::ChildComplete(_))
            ),
            _ => false,
        };
        if matches {
            Ok(())
        } else {
            Err(format!(
                "Dashboard completion out of order: {:?} before {:?}",
                self.pending, event
            ))
        }
    }
    pub fn reduce(
        &mut self,
        event: Event,
        layout: Option<&menu::MenuLayout>,
    ) -> Result<Vec<Effect>, String> {
        self.check_pending(&event)?;
        Ok(match event {
            Event::Begin {
                now,
                wayland,
                rescan,
            } => {
                let mut e = vec![Effect::ReapSound];
                if self.guard.recover_if_quiet(now) {
                    e.push(Effect::ControllerResumed);
                }
                if !wayland
                    && !self.touch_connected
                    && now.saturating_sub(self.last_touch_reconnect) >= 1000
                {
                    self.last_touch_reconnect = now;
                    e.push(Effect::ReconnectTouch);
                }
                if rescan
                    || self
                        .last_scan
                        .is_none_or(|t| t == 0 || now.saturating_sub(t) >= 1000)
                {
                    self.last_scan = Some(now);
                    e.push(Effect::ScanControls);
                }
                e
            }
            Event::Tick(now) => {
                let mut e = Vec::new();
                if self.reboot_deadline > 0 && now >= self.reboot_deadline {
                    self.reboot_deadline = 0;
                    if self.status == REBOOT_CONFIRMATION {
                        self.set_status("");
                        e.extend(screen(None));
                    }
                }
                if matches!(self.view, View::Settings | View::Wifi)
                    && now.saturating_sub(self.network_refreshed_at) >= 2000
                {
                    self.network_refreshed_at = now;
                    self.pending = Some(Pending::Network);
                    e.push(Effect::NetworkAction);
                }
                if self.view == View::Credits
                    && !self.reduced_motion
                    && now.saturating_sub(self.credits_rendered_at) >= 66
                {
                    self.credits_rendered_at = now;
                    e.extend(screen(None));
                }
                e
            }
            Event::Controls {
                now,
                mut gamepad,
                keyboard,
                quarantined,
            } => {
                let mut prefix = Vec::new();
                if !gamepad.is_empty() {
                    let was_suspended = self.guard.suspended;
                    if !self.guard.accept_edge(now) {
                        gamepad.clear();
                    }
                    if !was_suspended && self.guard.suspended {
                        prefix.push(Effect::ControllerSuspended);
                    }
                }
                if quarantined {
                    gamepad.clear();
                }
                for a in keyboard {
                    if !gamepad.contains(&a) {
                        gamepad.push(a);
                    }
                }
                if let Some(command) = command(&gamepad, self.view) {
                    self.clear_pressed();
                    prefix.push(Effect::DiscardTouch);
                    prefix.extend(self.command(command, now));
                }
                prefix
            }
            Event::Touch { now, report } => match self.view {
                View::Dashboard => {
                    self.dashboard_touch(report, layout.ok_or("Dashboard has no menu layout")?, now)
                }
                View::Settings => {
                    if let Some(plan) = settings::touch(
                        &mut self.settings,
                        &settings::Layout::default(),
                        report.x,
                        report.y,
                        report.pressed,
                        report.released,
                    ) {
                        self.settings_plan(plan, true)
                    } else {
                        Vec::new()
                    }
                }
                View::Credits => {
                    if self
                        .credits
                        .touch(credits::Touch {
                            x: report.x,
                            y: report.y,
                            pressed: report.pressed,
                            released: report.released,
                        })
                        .is_some()
                    {
                        self.set_view(View::Dashboard);
                        self.set_status("");
                        screen(Some(Cue::Back))
                    } else {
                        Vec::new()
                    }
                }
                View::Wifi => self.wifi_touch(report),
            },
            Event::TouchLost | Event::TouchReconnected => {
                let connected = matches!(event, Event::TouchReconnected);
                self.clear_pressed();
                self.touch_connected = connected;
                let status = if connected {
                    "TOUCHSCREEN RECONNECTED"
                } else {
                    "WAITING FOR TOUCHSCREEN"
                };
                if self.view == View::Wifi {
                    self.wifi.status = status.into();
                } else {
                    self.set_status(status);
                }
                screen(None)
            }
            Event::NetworkResult(network) => {
                let changed = self.network != network;
                self.network = network;
                self.pending = None;
                if changed { screen(None) } else { Vec::new() }
            }
            Event::SettingsResult(ok) => {
                let Some(Pending::Settings(plan)) = self.pending.take() else {
                    unreachable!()
                };
                settings::complete(&mut self.settings, &plan, ok, true);
                self.set_status(self.settings.status.clone());
                let mut e = screen(None);
                match plan.action {
                    settings::Action::Volume(v) if ok => {
                        if v == 0 {
                            e.push(Effect::StopSound);
                        } else {
                            self.pending = Some(Pending::VolumeTone(plan));
                            e.push(Effect::VolumeCue);
                        }
                    }
                    settings::Action::Volume(_) => {}
                    _ => {
                        if let Some(c) = plan.cue {
                            e.push(Effect::Cue(c.into()));
                        }
                    }
                }
                e
            }
            Event::VolumeToneResult(ok) => {
                let Some(Pending::VolumeTone(plan)) = self.pending.take() else {
                    unreachable!()
                };
                if !ok {
                    settings::complete(&mut self.settings, &plan, true, false);
                    self.set_status(self.settings.status.clone());
                    screen(None)
                } else {
                    Vec::new()
                }
            }
            Event::WifiResult {
                succeeded,
                failure_status,
            } => {
                self.pending = None;
                wifi::complete_save(&mut self.wifi, succeeded, failure_status.as_deref());
                vec![Effect::Render, Effect::Cue(Cue::Confirm), Effect::Present]
            }
            Event::Prepared { game, plan } => {
                let Some(Pending::Launch(_)) = self.pending.take() else {
                    unreachable!()
                };
                self.set_status(match plan.kind {
                    policy::LaunchKind::Reboot => "REBOOTING".into(),
                    policy::LaunchKind::Terminal => format!("STARTING {}", terminal_title(&plan)),
                    policy::LaunchKind::Game => format!("STARTING {}", game.title),
                });
                self.pending = Some(Pending::Child(ActiveLaunch {
                    game,
                    plan,
                    result: None,
                    recovery: None,
                }));
                vec![
                    Effect::Render,
                    Effect::Present,
                    Effect::FinishSound,
                    Effect::CloseControls,
                    Effect::Launch,
                ]
            }
            Event::ChildReturned {
                result,
                touch_disconnected,
            } => {
                self.clear_pressed();
                if result.shutdown_requested {
                    self.pending = None;
                    vec![Effect::StopLoop]
                } else {
                    if touch_disconnected {
                        self.touch_connected = false;
                    }
                    let Some(Pending::Child(active)) = &mut self.pending else {
                        unreachable!()
                    };
                    active.result = Some(result);
                    active.recovery = Some(Recovery::Controls);
                    vec![Effect::ScanControls]
                }
            }
            Event::ControlsRescanned(now) => {
                self.last_scan = Some(now);
                if let Some(Pending::Child(active)) = &mut self.pending {
                    active.recovery = Some(Recovery::Presentation);
                    vec![Effect::OpenPresentation]
                } else {
                    Vec::new()
                }
            }
            Event::PresentationOpened => {
                let Some(Pending::Child(active)) = &mut self.pending else {
                    unreachable!()
                };
                active.recovery = Some(Recovery::Volume);
                vec![Effect::ReloadVolume]
            }
            Event::ChildComplete(volume) => {
                let Some(Pending::Child(active)) = self.pending.take() else {
                    unreachable!()
                };
                if let Some(v) = volume {
                    self.settings.volume = v;
                    if v > 0 {
                        self.settings.last_audible_volume = v;
                    }
                }
                self.set_status(child_status(&active));
                screen(None)
            }
        })
    }
    fn wifi_touch(&mut self, report: TouchReport) -> Vec<Effect> {
        let target = wifi::target_at_state(&self.wifi, report.x, report.y);
        if report.pressed {
            self.wifi.pressed = target;
        }
        if !report.released {
            return Vec::new();
        }
        let old = self.wifi.pressed.take();
        if old != target || target.is_none() {
            return Vec::new();
        }
        match target.unwrap() {
            wifi::Target::Back => {
                self.set_view(View::Settings);
                self.set_status("WIFI EDITOR CLOSED");
                vec![Effect::Render, Effect::Cue(Cue::Back), Effect::Present]
            }
            wifi::Target::Save => {
                if let Err(status) = wifi::valid(&self.wifi) {
                    self.wifi.status = status;
                    vec![Effect::Render, Effect::Cue(Cue::Confirm), Effect::Present]
                } else {
                    self.pending = Some(Pending::Wifi);
                    vec![Effect::WifiAction]
                }
            }
            target => {
                if target == wifi::Target::Shift && self.wifi.mode == wifi::Mode::Symbols {
                    vec![Effect::Present]
                } else {
                    wifi::apply(&mut self.wifi, target);
                    self.wifi.status.clear();
                    vec![Effect::Render, Effect::Cue(Cue::Next), Effect::Present]
                }
            }
        }
    }
    pub fn poll_timeout(&self, now: u64) -> u32 {
        if self.view == View::Credits && !self.reduced_motion {
            (self
                .credits_rendered_at
                .saturating_add(66)
                .saturating_sub(now))
            .min(66) as u32
        } else {
            250
        }
    }
}
pub fn populated_systems(games: &[policy::Game]) -> Vec<policy::System> {
    [
        policy::System::Nes,
        policy::System::Gb,
        policy::System::Gbc,
        policy::System::Gba,
        policy::System::Zx,
        policy::System::Deck,
    ]
    .into_iter()
    .filter(|s| games.iter().any(|g| g.system == *s))
    .collect()
}
pub fn terminal_title(plan: &policy::LaunchPlan) -> String {
    match plan.arguments.first().map(String::as_str) {
        Some("lua") => "LUA REPL",
        Some("lisp") => "LISP REPL",
        Some("python") => "PYTHON REPL",
        Some("scheme") => "SCHEME REPL",
        _ => "TERMINAL",
    }
    .into()
}
pub fn child_status(active: &ActiveLaunch) -> String {
    let result = active.result.as_ref().expect("completed child result");
    let reboot = active.plan.kind == policy::LaunchKind::Reboot;
    let terminal = active.plan.kind == policy::LaunchKind::Terminal;
    let title = if terminal {
        terminal_title(&active.plan)
    } else {
        active.game.title.clone()
    };
    if result.error.as_ref().is_some_and(|s| !s.is_empty()) {
        if terminal {
            format!("{title} ERROR - CHECK LOG")
        } else if reboot {
            "REBOOT ERROR - CHECK LOG".into()
        } else {
            "GAME ERROR - CHECK LOG".into()
        }
    } else if !result.started {
        if terminal {
            format!("{title} DID NOT START")
        } else if reboot {
            "REBOOT DID NOT START".into()
        } else {
            "GAME DID NOT START".into()
        }
    } else if result.exited_for_touch {
        if reboot {
            "REBOOT CANCELLED".into()
        } else {
            format!("RETURNED FROM {title}")
        }
    } else if result.exit_code == Some(0) {
        if reboot {
            "REBOOT COMMAND EXITED".into()
        } else {
            format!("{title} EXITED")
        }
    } else if let Some(code) = result.exit_code {
        if reboot {
            format!("REBOOT FAILED (STATUS {code})")
        } else {
            format!("{title} EXITED (STATUS {code})")
        }
    } else if let Some(signal) = result.signal {
        if reboot {
            format!("REBOOT STOPPED (SIGNAL {signal})")
        } else {
            format!("{title} STOPPED (SIGNAL {signal})")
        }
    } else if reboot {
        "REBOOT STOPPED".into()
    } else {
        format!("{title} STOPPED")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        App::new(policy::builtins(), credits::Credits::empty(), false)
    }
    #[test]
    fn priorities_and_quarantine_keep_keyboard_available() {
        assert_eq!(
            command(
                &[Action::Confirm, Action::Right, Action::Settings],
                View::Dashboard
            ),
            Some(Command::Settings)
        );
        assert_eq!(
            command(&[Action::Back, Action::Settings], View::Wifi),
            Some(Command::Back)
        );
        assert_eq!(command(&[Action::Confirm], View::Credits), None);
        let mut a = app();
        let old = a.position;
        let effects = a
            .reduce(
                Event::Controls {
                    now: 100,
                    gamepad: vec![Action::Right],
                    keyboard: vec![Action::Left],
                    quarantined: true,
                },
                None,
            )
            .unwrap();
        assert_eq!(a.position, (old + a.games.len() - 1) % a.games.len());
        assert!(matches!(effects.first(), Some(Effect::DiscardTouch)));
    }
    #[test]
    fn reboot_requires_second_confirmation_and_expires_at_deadline() {
        let mut a = app();
        let i = a.games.iter().position(|g| g.id == "reboot").unwrap();
        a.request_game(i, 100, false);
        assert_eq!(a.reboot_deadline, 4100);
        assert!(a.pending.is_none());
        a.reduce(Event::Tick(4100), None).unwrap();
        assert_eq!(a.status, "");
        a.request_game(i, 4200, false);
        a.request_game(i, 4300, false);
        assert!(matches!(a.pending, Some(Pending::Launch(_))));
        assert_eq!(a.reboot_deadline, 0);
    }
    #[test]
    fn completion_order_is_enforced() {
        let mut a = app();
        a.pending = Some(Pending::Network);
        assert!(a.reduce(Event::ControlsRescanned(1), None).is_err());
        a.reduce(Event::NetworkResult(NetworkStatus::default()), None)
            .unwrap();
        assert!(a.pending.is_none());
    }
    #[test]
    fn burst_guard_counts_quarantined_edges_and_recovers_after_quiet() {
        let mut a = app();
        for n in 0..13 {
            a.reduce(
                Event::Controls {
                    now: n * 10,
                    gamepad: vec![Action::Confirm],
                    keyboard: vec![],
                    quarantined: true,
                },
                None,
            )
            .unwrap();
        }
        assert!(a.guard.suspended);
        a.reduce(
            Event::Begin {
                now: 1119,
                wayland: true,
                rescan: false,
            },
            None,
        )
        .unwrap();
        assert!(a.guard.suspended);
        a.reduce(
            Event::Begin {
                now: 1120,
                wayland: true,
                rescan: false,
            },
            None,
        )
        .unwrap();
        assert!(!a.guard.suspended);
    }
}
