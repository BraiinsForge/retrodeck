//! Native dashboard resources and synchronous effect execution.
use super::{
    app::{App, Cue, Effect, Event, Pending, View},
    cli, credits, menu, policy, settings, ui, wifi,
};
use crate::{audio, canvas, controls, fbdev, input, network, polling, process, wayland};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub const WIFI_HELPER: &str = "/usr/sbin/deck-wifi-profile-add";
pub const WIFI_STATUS: &str = "/var/run/deck-wifi/status";
#[derive(Clone, Debug)]
pub struct Paths {
    pub volume: PathBuf,
    pub brightness: PathBuf,
    pub brightness_max: PathBuf,
    pub brightness_state: PathBuf,
    pub keymap: PathBuf,
    pub wifi_helper: PathBuf,
    pub wifi_status: PathBuf,
}
impl Default for Paths {
    fn default() -> Self {
        Self {
            volume: settings::VOLUME_STATE.into(),
            brightness: settings::BRIGHTNESS_DEVICE.into(),
            brightness_max: settings::BRIGHTNESS_MAXIMUM.into(),
            brightness_state: settings::BRIGHTNESS_STATE.into(),
            keymap: settings::KEYMAP_STATE.into(),
            wifi_helper: WIFI_HELPER.into(),
            wifi_status: WIFI_STATUS.into(),
        }
    }
}
impl Paths {
    fn from_options(o: &cli::Options) -> Self {
        let mut p = Self::default();
        if let Some(v) = &o.volume_state {
            p.volume = v.clone();
        }
        if let Some(v) = &o.brightness {
            p.brightness = v.into();
        }
        if let Some(v) = &o.brightness_max {
            p.brightness_max = v.into();
        }
        if let Some(v) = &o.brightness_state {
            p.brightness_state = v.clone();
        }
        if let Some(v) = &o.keymap_state {
            p.keymap = v.clone();
        }
        if let Some(v) = &o.wifi_helper {
            p.wifi_helper = v.into();
        }
        if let Some(v) = &o.wifi_status {
            p.wifi_status = v.clone();
        }
        p
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Backend {
    Widget,
    Fbdev,
}
/// Ownership flags are set immediately after acquiring each process-global native resource.
/// Drop unwinds partial startup and runtime failures as well as normal shutdown.
pub struct Runtime {
    paths: Paths,
    launch_config: policy::LaunchConfig,
    display: Option<PathBuf>,
    backend: Option<Backend>,
    presentation_owned: bool,
    touch_owned: bool,
    controls_owned: bool,
    audio_owned: bool,
    sound_active: bool,
    input_until: u64,
    brightness_maximum: Option<u32>,
    default_volume: u8,
    layout: Option<menu::MenuLayout>,
    renderer: menu::MenuRenderer,
    running: bool,
    rescan: bool,
    now: u64,
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.shutdown();
    }
}
pub fn now_ms() -> u64 {
    polling::monotonic_nanoseconds() / 1_000_000
}
pub fn notes(cue: Cue) -> &'static [audio::Tone] {
    const VOLUME: [audio::Tone; 2] = [
        audio::Tone {
            frequency: 660,
            duration_ms: 60,
        },
        audio::Tone {
            frequency: 880,
            duration_ms: 60,
        },
    ];
    const PREVIOUS: [audio::Tone; 1] = [audio::Tone {
        frequency: 523,
        duration_ms: 35,
    }];
    const NEXT: [audio::Tone; 1] = [audio::Tone {
        frequency: 659,
        duration_ms: 35,
    }];
    const CONFIRM: [audio::Tone; 2] = [
        audio::Tone {
            frequency: 659,
            duration_ms: 25,
        },
        audio::Tone {
            frequency: 880,
            duration_ms: 30,
        },
    ];
    const BACK: [audio::Tone; 2] = [
        audio::Tone {
            frequency: 659,
            duration_ms: 25,
        },
        audio::Tone {
            frequency: 440,
            duration_ms: 30,
        },
    ];
    match cue {
        Cue::Volume => &VOLUME,
        Cue::Previous => &PREVIOUS,
        Cue::Next => &NEXT,
        Cue::Confirm => &CONFIRM,
        Cue::Back => &BACK,
    }
}
impl Runtime {
    pub fn new(options: &cli::Options) -> Result<Self, String> {
        let default_volume = settings::parse_inherited_volume(
            std::env::var("RETRO_DECK_VOLUME_PERCENT").ok().as_deref(),
        )?;
        let display = std::env::var_os("WAYLAND_DISPLAY")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        let renderer = menu::MenuRenderer::new(
            options
                .cover_directory
                .clone()
                .unwrap_or_else(|| "/mnt/data/nes-deck/covers".into()),
            Some(
                options
                    .settings_icon
                    .clone()
                    .unwrap_or_else(|| "/mnt/data/nes-deck/menu/settings-icon.png".into()),
            ),
        );
        Ok(Self {
            paths: Paths::from_options(options),
            launch_config: options.launch_config(),
            display,
            backend: None,
            presentation_owned: false,
            touch_owned: false,
            controls_owned: false,
            audio_owned: false,
            sound_active: false,
            input_until: 0,
            brightness_maximum: None,
            default_volume,
            layout: None,
            renderer,
            running: false,
            rescan: false,
            now: 0,
        })
    }
    fn widget(&self) -> bool {
        self.backend == Some(Backend::Widget)
    }
    fn open_presentation(&mut self) -> Result<(), String> {
        if let Some(display) = &self.display {
            if wayland::size().is_some() {
                self.backend = Some(Backend::Widget);
                return Ok(());
            }
            match wayland::open_widget_at(display) {
                Ok(()) => {
                    self.backend = Some(Backend::Widget);
                    self.presentation_owned = true;
                    return Ok(());
                }
                Err(error) => {
                    eprintln!("retrodeck: {error}; Wayland widget unavailable; trying fbdev")
                }
            }
        }
        if fbdev::size().is_none() {
            fbdev::open()?;
            self.presentation_owned = true;
        }
        self.backend = Some(Backend::Fbdev);
        Ok(())
    }
    fn initialize(&mut self, app: &mut App) -> Result<(), String> {
        if self.running {
            return Err("Dashboard runtime is already initialized".into());
        }
        self.now = now_ms();
        app.settings.volume = settings::load_volume(&self.paths.volume, self.default_volume)?;
        app.settings.last_audible_volume = if app.settings.volume > 0 {
            app.settings.volume
        } else if self.default_volume > 0 {
            self.default_volume
        } else {
            5
        };
        let (brightness, maximum) = settings::load_backlight(
            &self.paths.brightness,
            &self.paths.brightness_max,
            &self.paths.brightness_state,
        )?;
        app.settings.brightness = brightness;
        self.brightness_maximum = maximum;
        app.settings.keymap = settings::load_keymap(&self.paths.keymap)?;
        self.open_presentation()?;
        if !self.widget() {
            input::open_touch()?;
            self.touch_owned = true;
            app.touch_connected = true;
        }
        self.controls_owned = true;
        controls::scan()?;
        app.last_scan = Some(self.now);
        app.network = network::read_network_status(&self.paths.wifi_status);
        app.network_refreshed_at = self.now;
        self.render(app)?;
        self.present()?;
        self.running = true;
        Ok(())
    }
    fn render(&mut self, app: &mut App) -> Result<(), String> {
        match app.view {
            View::Dashboard => {
                self.layout =
                    Some(
                        self.renderer
                            .render(&app.games, app.system, app.position, &app.status),
                    )
            }
            View::Settings => {
                settings::render_frame(&app.settings, &app.network);
                self.layout = None;
            }
            View::Wifi => {
                wifi::render_frame(&app.wifi, &app.network);
                self.layout = None;
            }
            View::Credits => {
                app.credits.tick(
                    self.now
                        .saturating_sub(app.credits_started_at)
                        .min(i64::MAX as u64) as i64,
                );
                app.credits.render(
                    app.reduced_motion,
                    ui::color("background"),
                    ui::color("title"),
                    ui::color("text"),
                    ui::color("muted"),
                )?;
                self.layout = None;
            }
        }
        Ok(())
    }
    fn present(&self) -> Result<(), String> {
        canvas::with_pixels(|pixels| {
            if self.widget() {
                wayland::present_rgba(pixels)
            } else {
                fbdev::present_rgba(pixels)
            }
        })
    }
    fn cue(&mut self, cue: Cue, volume: u8) -> bool {
        if volume == 0 {
            return true;
        }
        let tones = notes(cue);
        let started_at = now_ms();
        match audio::play_tone_sequence(tones, i32::from(volume)) {
            Ok(audio::PlayOutcome::Started) => {
                self.audio_owned = true;
                self.sound_active = true;
                self.input_until = started_at
                    .saturating_add(tones.iter().map(|t| t.duration_ms as u64).sum::<u64>())
                    .saturating_add(60);
                true
            }
            Ok(audio::PlayOutcome::Busy) => {
                self.sound_active = true;
                true
            }
            Err(error) => {
                eprintln!("retrodeck: {error}");
                false
            }
        }
    }
    fn stop_sound(&mut self, finish: bool) {
        if self.audio_owned {
            if finish {
                audio::finish();
            } else {
                audio::stop();
            }
            self.audio_owned = false;
            self.sound_active = false;
            self.input_until = 0;
        }
    }
    fn shutdown(&mut self) {
        self.stop_sound(false);
        if self.controls_owned {
            self.controls_owned = false;
            controls::close();
        }
        if self.touch_owned {
            self.touch_owned = false;
            input::close_touch();
        }
        if self.presentation_owned {
            self.presentation_owned = false;
            if self.widget() {
                wayland::close();
            } else {
                fbdev::close();
            }
        }
        self.backend = None;
        self.layout = None;
        self.running = false;
    }
    fn settings_action(&mut self, plan: &settings::Effect) -> bool {
        let result = match &plan.action {
            settings::Action::Volume(value) => settings::save_volume(&self.paths.volume, *value),
            settings::Action::Brightness(value) => settings::set_backlight(
                &self.paths.brightness,
                &self.paths.brightness_state,
                self.brightness_maximum,
                *value,
            ),
            settings::Action::Keymap(value) => settings::save_keymap(&self.paths.keymap, value),
            _ => Err("Unexpected native settings action".into()),
        };
        if let Err(error) = &result {
            eprintln!("retrodeck: {error}");
        }
        result.is_ok()
    }
    fn effect(&mut self, effect: &Effect, app: &mut App) -> Result<Option<Event>, String> {
        Ok(match effect {
            Effect::Render => {
                self.render(app)?;
                None
            }
            Effect::Present => {
                self.present()?;
                None
            }
            Effect::Cue(cue) => {
                self.cue(*cue, app.settings.volume);
                None
            }
            Effect::VolumeCue => Some(Event::VolumeToneResult(
                self.cue(Cue::Volume, app.settings.volume),
            )),
            Effect::StopSound => {
                self.stop_sound(false);
                None
            }
            Effect::FinishSound => {
                self.stop_sound(true);
                None
            }
            Effect::ReapSound => {
                self.sound_active = audio::active();
                if !self.sound_active && self.now >= self.input_until {
                    self.audio_owned = false;
                }
                None
            }
            Effect::ScanControls => {
                self.controls_owned = true;
                controls::scan()?;
                Some(Event::ControlsRescanned(self.now))
            }
            Effect::CloseControls => {
                controls::close();
                self.controls_owned = false;
                None
            }
            Effect::ReconnectTouch => match input::open_touch() {
                Ok(()) => {
                    self.touch_owned = true;
                    Some(Event::TouchReconnected)
                }
                Err(_) => None,
            },
            Effect::OpenPresentation => {
                self.open_presentation()?;
                Some(Event::PresentationOpened)
            }
            Effect::DiscardTouch => None,
            Effect::ControllerSuspended => {
                eprintln!("retrodeck: controller input suspended after burst; waiting for quiet");
                None
            }
            Effect::ControllerResumed => {
                eprintln!("retrodeck: controller input resumed after quiet period");
                None
            }
            Effect::SettingsAction(plan) => Some(Event::SettingsResult(self.settings_action(plan))),
            Effect::WifiAction => {
                let request = wifi::helper_input(&app.wifi.ssid, &app.wifi.passphrase);
                let result = process::run_helper(&self.paths.wifi_helper, request.as_bytes());
                if let Some(error) = &result.error {
                    eprintln!("retrodeck: Wi-Fi helper failed: {error}");
                }
                let succeeded =
                    result.phase == process::HelperPhase::Complete && result.exit_code == Some(0);
                let failure_status = (result.phase == process::HelperPhase::Input)
                    .then(|| "WIFI PROFILE WRITE FAILED".to_owned());
                Some(Event::WifiResult {
                    succeeded,
                    failure_status,
                })
            }
            Effect::NetworkAction => Some(Event::NetworkResult(network::read_network_status(
                &self.paths.wifi_status,
            ))),
            Effect::PrepareLaunch => {
                let game = app.pending_application()?;
                let plan = policy::launch_plan(
                    &game,
                    app.settings.volume,
                    &app.settings.keymap,
                    self.widget(),
                    Some(&self.paths.volume),
                    &self.launch_config,
                );
                Some(Event::Prepared { game, plan })
            }
            Effect::Launch => {
                let Some(Pending::Child(active)) = &app.pending else {
                    return Err("Dashboard has no active launch".into());
                };
                let plan = &active.plan;
                let touch_disconnected = !plan.touch_supervision;
                if !self.widget() && !self.presentation_owned {
                    return Err("Dashboard cannot launch with borrowed fbdev presentation".into());
                }
                let arguments: Vec<_> = plan.arguments.iter().map(OsString::from).collect();
                let environment: Vec<_> = plan
                    .environment
                    .iter()
                    .map(|(k, v)| (OsString::from(k), OsString::from(v)))
                    .collect();
                let result = process::run_child(
                    Path::new(&plan.executable),
                    &arguments,
                    &environment,
                    &plan.label,
                    plan.touch_supervision,
                    plan.mirror_console,
                );
                if let Some(error) = &result.error {
                    eprintln!("retrodeck: {error}");
                }
                self.now = now_ms();
                Some(Event::ChildReturned {
                    result,
                    touch_disconnected,
                })
            }
            Effect::ReloadVolume => {
                let value = match settings::load_volume(&self.paths.volume, self.default_volume) {
                    Ok(v) => {
                        if v != app.settings.volume {
                            eprintln!("retrodeck: child updated game volume to {v}%");
                        }
                        Some(v)
                    }
                    Err(error) => {
                        eprintln!("retrodeck: cannot reload child volume: {error}");
                        None
                    }
                };
                Some(Event::ChildComplete(value))
            }
            Effect::StopLoop => {
                self.shutdown();
                None
            }
        })
    }
    fn step(
        &mut self,
        app: &mut App,
        event: Event,
        trace: &mut Vec<&'static str>,
    ) -> Result<(), String> {
        let effects = app.reduce(event, self.layout.as_ref())?;
        for effect in effects {
            trace.push(effect_name(&effect));
            if let Some(completion) = self.effect(&effect, app)? {
                self.step(app, completion, trace)?;
            }
        }
        Ok(())
    }
    fn iteration(&mut self, app: &mut App) -> Result<(), String> {
        self.now = now_ms();
        let mut trace = Vec::new();
        let rescan = std::mem::take(&mut self.rescan);
        self.step(
            app,
            Event::Begin {
                now: self.now,
                wayland: self.widget(),
                rescan,
            },
            &mut trace,
        )?;
        let poll = polling::dispatch(self.widget(), app.poll_timeout(now_ms()))?;
        self.now = now_ms();
        let reports: Vec<_> = (0..poll.touch_count)
            .map(|_| {
                if self.widget() {
                    wayland::next_touch()
                } else {
                    input::next_touch()
                }
                .ok_or_else(|| "Dashboard native touch queue ended early".to_owned())
            })
            .collect::<Result<_, _>>()?;
        let mut control_reports = Vec::new();
        while let Some(report) = controls::next_report() {
            control_reports.push(report);
        }
        if control_reports.len() != poll.control_count {
            return Err(format!(
                "Dashboard native control count changed from {} to {}",
                poll.control_count,
                control_reports.len()
            ));
        }
        if poll.refresh {
            self.present()?;
        }
        self.rescan |= poll.rescan;
        self.step(app, Event::Tick(self.now), &mut trace)?;
        if poll.ready {
            if poll.touch_lost {
                if self.touch_owned {
                    input::close_touch();
                    self.touch_owned = false;
                }
                self.step(app, Event::TouchLost, &mut trace)?;
            }
            let (gamepad, keyboard) = super::app::collect_actions(&control_reports);
            self.step(
                app,
                Event::Controls {
                    now: self.now,
                    gamepad,
                    keyboard,
                    quarantined: self.sound_active || self.now < self.input_until,
                },
                &mut trace,
            )?;
            if !poll.touch_lost && !trace.contains(&"discard-touch") {
                for report in reports {
                    self.step(
                        app,
                        Event::Touch {
                            now: self.now,
                            report,
                        },
                        &mut trace,
                    )?;
                }
            }
            if matches!(app.pending, Some(Pending::Launch(_))) {
                let effect = Effect::PrepareLaunch;
                trace.push(effect_name(&effect));
                if let Some(event) = self.effect(&effect, app)? {
                    self.step(app, event, &mut trace)?;
                }
            }
        }
        if poll.shutdown
            || process::shutdown_requested()
            || self.widget() && wayland::shutdown_requested()
        {
            self.shutdown();
        }
        Ok(())
    }
}
fn effect_name(effect: &Effect) -> &'static str {
    match effect {
        Effect::Render => "render",
        Effect::Present => "present",
        Effect::Cue(_) => "cue",
        Effect::VolumeCue => "volume-cue",
        Effect::StopSound => "stop-sound",
        Effect::FinishSound => "finish-sound",
        Effect::ReapSound => "reap-sound",
        Effect::ScanControls => "scan-controls",
        Effect::CloseControls => "close-controls",
        Effect::ReconnectTouch => "reconnect-touch",
        Effect::OpenPresentation => "open-presentation",
        Effect::DiscardTouch => "discard-touch",
        Effect::ControllerSuspended => "controller-suspended",
        Effect::ControllerResumed => "controller-resumed",
        Effect::SettingsAction(_) => "settings-action",
        Effect::WifiAction => "wifi-action",
        Effect::NetworkAction => "network-action",
        Effect::PrepareLaunch => "prepare-launch",
        Effect::Launch => "launch",
        Effect::ReloadVolume => "reload-volume",
        Effect::StopLoop => "stop-loop",
    }
}
pub fn main(arguments: impl IntoIterator<Item = String>) -> i32 {
    let options = match cli::parse(arguments) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("deck-menu: {error}");
            return 2;
        }
    };
    let validation = match cli::validate(&options) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("deck-menu: {error}");
            return 1;
        }
    };
    let (games, palette) = match validation {
        cli::Validation::Help => {
            println!("{}", cli::HELP);
            return 0;
        }
        cli::Validation::Manifest(_) | cli::Validation::Palette => return 0,
        cli::Validation::Run { games, palette } => (games, palette),
    };
    ui::install_palette(palette.colors);
    let path = options
        .credits
        .as_deref()
        .unwrap_or_else(|| Path::new(credits::CREDITS_PATH));
    let credits = match credits::load(path) {
        Ok(entries) => credits::Credits::new(entries),
        Err(error) => {
            eprintln!("retrodeck: {error}; the FOSS credits screen will show an error");
            eprintln!("deck-menu: credits failed to load");
            credits::Credits::empty()
        }
    };
    let reduced_motion =
        std::env::var_os("RETRO_DECK_REDUCED_MOTION").is_some_and(|s| !s.is_empty());
    let mut app = App::new(games, credits, reduced_motion);
    let result = (|| {
        process::install_signal_handlers()?;
        let mut runtime = Runtime::new(&options)?;
        runtime.initialize(&mut app)?;
        while runtime.running {
            runtime.iteration(&mut app)?;
        }
        Ok::<(), String>(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("deck-menu: {error}");
            1
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_and_input_tail_match_baseline() {
        let cases = [
            (Cue::Volume, 120),
            (Cue::Previous, 35),
            (Cue::Next, 35),
            (Cue::Confirm, 55),
            (Cue::Back, 55),
        ];
        for (cue, ms) in cases {
            assert_eq!(notes(cue).iter().map(|n| n.duration_ms).sum::<i32>(), ms);
        }
        assert_eq!(notes(Cue::Confirm)[0].frequency, 659);
    }
}
