/// Pure settings reducer and persistence. Rendering is represented as deterministic draw ops.
use std::path::Path;

use crate::{control_file, state_file};

pub const VOLUME_DEFAULT: u8 = 42;
pub const VOLUME_STEP: u8 = 5;
pub const BRIGHTNESS_DEFAULT: u8 = 100;
pub const VOLUME_STATE: &str = "/mnt/data/nes-deck/state/menu-volume.state";
pub const BRIGHTNESS_STATE: &str = "/mnt/data/nes-deck/state/menu-brightness.state";
pub const BRIGHTNESS_DEVICE: &str = "/sys/class/backlight/display-bl/brightness";
pub const BRIGHTNESS_MAXIMUM: &str = "/sys/class/backlight/display-bl/max_brightness";
pub const KEYMAP_STATE: &str = "/mnt/data/nes-deck/state/terminal-keymap.state";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Close,
    VolumeDown,
    VolumeUp,
    BrightnessDown,
    BrightnessUp,
    Terminal,
    Keymap,
    Wifi,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Previous,
    Next,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cue {
    Back,
    Previous,
    Next,
    Confirm,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Close,
    Volume(u8),
    Brightness(u8),
    Keymap(String),
    Terminal,
    Wifi,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Effect {
    pub action: Action,
    pub cue: Option<Cue>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}
impl Rect {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w as i32 && y >= self.y && y < self.y + self.h as i32
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Layout {
    pub close: Rect,
    pub wifi: Rect,
    pub volume_down: Rect,
    pub volume_up: Rect,
    pub brightness_down: Rect,
    pub brightness_up: Rect,
    pub terminal: Rect,
    pub keymap: Rect,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            close: Rect::new(1212, 12, 56, 56),
            wifi: Rect::new(926, 20, 262, 108),
            volume_down: Rect::new(108, 208, 104, 104),
            volume_up: Rect::new(228, 208, 104, 104),
            brightness_down: Rect::new(438, 208, 104, 104),
            brightness_up: Rect::new(558, 208, 104, 104),
            terminal: Rect::new(792, 208, 112, 104),
            keymap: Rect::new(1036, 208, 112, 104),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub open: bool,
    pub selected: Target,
    pub pressed: Option<Target>,
    pub volume: u8,
    pub last_audible_volume: u8,
    pub brightness: u8,
    pub keymap: String,
    pub status: String,
}
impl State {
    pub fn new(volume: u8, brightness: u8, keymap: &str) -> Self {
        assert!(volume <= 100, "volume must be 0..100");
        assert!(brightness <= 100, "brightness must be 0..100");
        assert!(matches!(keymap, "us" | "cz"), "unknown terminal keymap");
        Self {
            open: true,
            selected: Target::VolumeDown,
            pressed: None,
            volume,
            last_audible_volume: if volume == 0 { VOLUME_DEFAULT } else { volume },
            brightness,
            keymap: keymap.into(),
            status: String::new(),
        }
    }
}
impl Default for State {
    fn default() -> Self {
        Self::new(VOLUME_DEFAULT, 100, "us")
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DrawOp {
    Clear(u32),
    Fill(Rect, u32),
    Outline(Rect, u32, u32),
    Text {
        i32_: i32,
        y: i32,
        text: String,
        scale: u32,
        color: u32,
    },
    Centered {
        rect: Rect,
        text: String,
        scale: u32,
        color: u32,
    },
}
pub fn parse_inherited_volume(v: Option<&str>) -> Result<u8, String> {
    match v {
        None => Ok(VOLUME_DEFAULT),
        Some(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => s
            .parse::<u16>()
            .ok()
            .filter(|x| *x <= 100)
            .map(|x| x as u8)
            .ok_or("volume must be 0..100".into()),
        Some("") => Err("RETRO_DECK_VOLUME_PERCENT is empty; expected 0 through 100".into()),
        _ => Err("RETRO_DECK_VOLUME_PERCENT must be an integer from 0 through 100".into()),
    }
}
pub fn volume_state_text(v: u8) -> Result<String, String> {
    if v <= 100 {
        Ok(format!("{v}\n"))
    } else {
        Err("volume outside 0..100".into())
    }
}
pub fn parse_volume_state(s: &str) -> Result<u8, String> {
    if !s.ends_with('\n') {
        return Err("volume state must end in newline".into());
    }
    let d = &s[..s.len() - 1];
    if d.is_empty() || (d.len() > 1 && d.starts_with('0')) || !d.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("volume state must contain a canonical integer from 0 through 100 followed by a newline".into());
    }
    d.parse::<u16>()
        .ok()
        .filter(|x| *x <= 100)
        .map(|x| x as u8)
        .ok_or("volume state outside 0..100".into())
}
pub fn brightness_valid(v: u8) -> bool {
    (10..=100).contains(&v) && v % 10 == 0
}
pub fn brightness_state_text(v: u8) -> Result<String, String> {
    if brightness_valid(v) {
        Ok(format!("{v}\n"))
    } else {
        Err("brightness must be a 10-point step from 10 through 100".into())
    }
}
pub fn parse_brightness_state(s: &str) -> Result<u8, String> {
    let v = parse_volume_state(s)?;
    if brightness_valid(v) {
        Ok(v)
    } else {
        Err("brightness state must contain a 10-point step from 10 through 100 followed by a newline".into())
    }
}
pub fn brightness_raw_value(percent: u32, max: u32) -> u32 {
    if max == 0 {
        0
    } else {
        (((percent as u64 * max as u64 + 50) / 100) as u32).clamp(1, max)
    }
}

pub fn observed_brightness_percent(cur: u32, max: u32) -> Result<u8, String> {
    if max == 0 || cur > max {
        return Err("brightness is outside the backlight range".into());
    }
    let observed = ((cur as u64 * 100 + (max / 2) as u64) / max as u64) as u32;
    Ok((((observed + 5) / 10) * 10).clamp(10, 100) as u8)
}

fn control_integer(bytes: &[u8], role: &str) -> Result<u32, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| format!("{role} must contain an unsigned integer"))?;
    let text = text.trim_matches(|c: char| matches!(c, '\u{9}'..='\u{d}' | ' '));
    if text.is_empty() {
        return Err(format!("{role} is empty"));
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{role} must contain an unsigned integer"));
    }
    text.parse::<u32>()
        .map_err(|_| format!("{role} is too large"))
}

pub fn save_volume(path: &Path, v: u8) -> Result<(), String> {
    state_file::write(path, volume_state_text(v)?.as_bytes())
}

pub fn save_brightness(path: &Path, v: u8) -> Result<(), String> {
    state_file::write(path, brightness_state_text(v)?.as_bytes())
}

pub fn load_volume(path: &Path, default: u8) -> Result<u8, String> {
    let value = match state_file::read(path)? {
        state_file::StateRead::Missing => {
            save_volume(path, default)?;
            default
        }
        state_file::StateRead::Value(bytes) if bytes == b"on\n" => {
            save_volume(path, default)?;
            default
        }
        state_file::StateRead::Value(bytes) if bytes == b"off\n" => {
            save_volume(path, 0)?;
            0
        }
        state_file::StateRead::Value(bytes) => {
            let text =
                std::str::from_utf8(&bytes).map_err(|_| "volume state is not UTF-8".to_owned())?;
            parse_volume_state(text)?
        }
    };
    Ok(value)
}

pub fn set_backlight(
    device: &Path,
    state: &Path,
    maximum: Option<u32>,
    percent: u8,
) -> Result<(), String> {
    let maximum = maximum.ok_or_else(|| "brightness maximum is unknown".to_owned())?;
    if maximum == 0 || !brightness_valid(percent) {
        return Err("brightness must be a 10-point step from 10 through 100".into());
    }
    control_file::write(
        device,
        format!("{}\n", brightness_raw_value(percent as u32, maximum)).as_bytes(),
    )
    .map_err(|_| format!("cannot write brightness {}", device.display()))?;
    save_brightness(state, percent)
        .map_err(|_| format!("cannot save brightness state {}", state.display()))?;
    Ok(())
}

pub fn load_backlight(
    device: &Path,
    maximum_path: &Path,
    state: &Path,
) -> Result<(u8, Option<u32>), String> {
    let maximum = control_integer(&control_file::read(maximum_path)?, "maximum brightness")?;
    if maximum == 0 {
        return Err("brightness is outside the backlight range".into());
    }
    let current = control_integer(&control_file::read(device)?, "brightness")?;
    if current > maximum {
        return Err("brightness is outside the backlight range".into());
    }
    let percent = match state_file::read(state)? {
        state_file::StateRead::Value(bytes) => {
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| "brightness state is not UTF-8".to_owned())?;
            parse_brightness_state(text)?
        }
        state_file::StateRead::Missing => observed_brightness_percent(current, maximum)?,
    };
    set_backlight(device, state, Some(maximum), percent)?;
    Ok((percent, Some(maximum)))
}
pub fn target_at(l: &Layout, x: i32, y: i32) -> Option<Target> {
    [
        (Target::Close, l.close),
        (Target::VolumeDown, l.volume_down),
        (Target::VolumeUp, l.volume_up),
        (Target::BrightnessDown, l.brightness_down),
        (Target::BrightnessUp, l.brightness_up),
        (Target::Terminal, l.terminal),
        (Target::Keymap, l.keymap),
        (Target::Wifi, l.wifi),
    ]
    .iter()
    .copied()
    .find_map(|(t, r)| r.contains(x, y).then_some(t))
}
pub fn activate(s: &State, t: Target) -> Effect {
    let cue = match t {
        Target::Close => Cue::Back,
        Target::VolumeDown | Target::BrightnessDown => Cue::Previous,
        Target::VolumeUp | Target::BrightnessUp => Cue::Next,
        _ => Cue::Confirm,
    };
    let action = match t {
        Target::Close => Action::Close,
        Target::VolumeDown => Action::Volume(if s.volume > VOLUME_STEP {
            s.volume - VOLUME_STEP
        } else {
            0
        }),
        Target::VolumeUp => Action::Volume(if s.volume == 0 {
            s.last_audible_volume.max(VOLUME_STEP)
        } else {
            (s.volume + VOLUME_STEP).min(100)
        }),
        Target::BrightnessDown => Action::Brightness(s.brightness.saturating_sub(10).max(10)),
        Target::BrightnessUp => Action::Brightness((s.brightness + 10).min(100)),
        Target::Terminal => Action::Terminal,
        Target::Keymap => Action::Keymap(if s.keymap == "cz" {
            "us".into()
        } else {
            "cz".into()
        }),
        Target::Wifi => Action::Wifi,
    };
    Effect {
        action,
        cue: Some(cue),
    }
}
pub fn complete(s: &mut State, e: &Effect, ok: bool, tone_ok: bool) {
    match &e.action {
        Action::Close => {
            s.open = false;
            s.status.clear()
        }
        Action::Volume(v) => {
            if ok {
                s.volume = *v;
                if *v > 0 {
                    s.last_audible_volume = *v
                }
                s.status = if *v == 0 {
                    "GAME VOLUME MUTED".into()
                } else if !tone_ok {
                    "VOLUME SAVED; CONFIRMATION TONE FAILED".into()
                } else {
                    format!("GAME VOLUME {v}")
                }
            } else {
                s.status = "VOLUME STATE ERROR".into()
            }
        }
        Action::Brightness(v) => {
            if ok {
                s.brightness = *v;
                s.status = format!("BRIGHTNESS {v}")
            } else {
                s.status = "BRIGHTNESS ERROR - CHECK LOG".into()
            }
        }
        Action::Keymap(v) => {
            if ok {
                s.keymap = v.clone();
                s.status = if v == "cz" {
                    "TERMINAL KEYS: CZECH".into()
                } else {
                    "TERMINAL KEYS: US ANSI".into()
                }
            } else {
                s.status = "KEYMAP STATE ERROR".into()
            }
        }
        _ => {}
    }
}
pub fn move_selection(s: &mut State, d: Direction) -> Cue {
    const O: [Target; 7] = [
        Target::VolumeDown,
        Target::VolumeUp,
        Target::BrightnessDown,
        Target::BrightnessUp,
        Target::Terminal,
        Target::Keymap,
        Target::Wifi,
    ];
    let i = O.iter().position(|x| *x == s.selected).unwrap_or(0);
    s.selected = O[match d {
        Direction::Previous => (i + 6) % 7,
        Direction::Next => (i + 1) % 7,
    }];
    s.status.clear();
    if matches!(d, Direction::Previous) {
        Cue::Previous
    } else {
        Cue::Next
    }
}
pub fn touch(
    s: &mut State,
    l: &Layout,
    x: i32,
    y: i32,
    pressed: bool,
    released: bool,
) -> Option<Effect> {
    let t = target_at(l, x, y);
    if pressed {
        s.pressed = t
    }
    if released {
        let old = s.pressed.take();
        if old.is_some() && old == t {
            return Some(activate(s, t.unwrap()));
        }
    }
    None
}
pub fn render(s: &State) -> Vec<DrawOp> {
    render_with_network(s, &crate::network::NetworkStatus::default())
}

fn pixel_panel(ops: &mut Vec<DrawOp>, r: Rect, inside: u32, border: u32) {
    const CUT: u32 = 4;
    if r.w <= CUT * 2 || r.h <= CUT * 2 {
        return;
    }
    ops.push(DrawOp::Fill(
        Rect::new(r.x + CUT as i32, r.y, r.w - CUT * 2, r.h),
        border,
    ));
    ops.push(DrawOp::Fill(
        Rect::new(r.x, r.y + CUT as i32, r.w, r.h - CUT * 2),
        border,
    ));
    let inner_w = r.w - CUT * 2;
    let inner_h = r.h - CUT * 2;
    if inner_w > CUT * 2 && inner_h > CUT * 2 {
        ops.push(DrawOp::Fill(
            Rect::new(
                r.x + (CUT * 2) as i32,
                r.y + CUT as i32,
                inner_w - CUT * 2,
                inner_h,
            ),
            inside,
        ));
        ops.push(DrawOp::Fill(
            Rect::new(
                r.x + CUT as i32,
                r.y + (CUT * 2) as i32,
                inner_w,
                inner_h - CUT * 2,
            ),
            inside,
        ));
    }
}

/// Build the complete settings view using the native palette and network status.
pub fn render_with_network(s: &State, network: &crate::network::NetworkStatus) -> Vec<DrawOp> {
    let mut o = vec![DrawOp::Clear(super::ui::color("background"))];
    let text = super::ui::color("text");
    let muted = super::ui::color("muted");
    let border = super::ui::color("control-border");
    let surface = super::ui::color("control-surface");
    let active = super::ui::color("active");
    let accent = super::ui::color("accent");
    o.push(DrawOp::Text {
        i32_: 64,
        y: 22,
        text: "ACTIVE WIFI".into(),
        scale: 1,
        color: muted,
    });
    o.push(DrawOp::Text {
        i32_: 64,
        y: 44,
        text: super::ui::fit_text_width(
            if network.ssid.is_empty() {
                "NOT CONNECTED"
            } else {
                &network.ssid
            },
            300,
            3,
        ),
        scale: 3,
        color: text,
    });
    o.push(DrawOp::Text {
        i32_: 392,
        y: 22,
        text: "WLAN0".into(),
        scale: 1,
        color: muted,
    });
    o.push(DrawOp::Text {
        i32_: 392,
        y: 44,
        text: if network.wlan_ipv4.is_empty() {
            "NO ADDRESS".into()
        } else {
            network.wlan_ipv4.clone()
        },
        scale: 2,
        color: text,
    });
    o.push(DrawOp::Text {
        i32_: 620,
        y: 22,
        text: "WIREGUARD".into(),
        scale: 1,
        color: muted,
    });
    o.push(DrawOp::Text {
        i32_: 620,
        y: 44,
        text: if network.wireguard_ipv4.is_empty() {
            "NO ADDRESS".into()
        } else {
            network.wireguard_ipv4.clone()
        },
        scale: 2,
        color: text,
    });
    o.push(DrawOp::Text {
        i32_: 64,
        y: 88,
        text: super::ui::fit_text_width(&format!("AUTO WIFI: {}", network.selector), 790, 1),
        scale: 1,
        color: super::ui::color("footer"),
    });
    let wifi = Rect::new(926, 20, 262, 108);
    pixel_panel(
        &mut o,
        wifi,
        if s.selected == Target::Wifi {
            active
        } else {
            surface
        },
        if s.selected == Target::Wifi {
            accent
        } else {
            border
        },
    );
    let controls = [
        (Rect::new(108, 208, 104, 104), Target::VolumeDown),
        (Rect::new(228, 208, 104, 104), Target::VolumeUp),
        (Rect::new(438, 208, 104, 104), Target::BrightnessDown),
        (Rect::new(558, 208, 104, 104), Target::BrightnessUp),
        (Rect::new(792, 208, 112, 104), Target::Terminal),
        (Rect::new(1036, 208, 112, 104), Target::Keymap),
    ];
    for (r, t) in controls {
        pixel_panel(
            &mut o,
            r,
            if s.selected == t { active } else { surface },
            if s.selected == t { accent } else { border },
        );
    }
    // Wi-Fi card geometry is relative to the card, as in the legacy renderer.
    for (dx, dy, w, h) in [
        (-6, 0, 12, 5),
        (-12, 5, 6, 5),
        (6, 5, 6, 5),
        (-18, 10, 6, 5),
        (12, 10, 6, 5),
        (-6, 22, 12, 5),
        (-12, 27, 6, 5),
        (6, 27, 6, 5),
        (-3, 36, 6, 6),
    ] {
        o.push(DrawOp::Fill(
            Rect::new(938 + 27 + dx, 44 + 5 + dy, w as u32, h as u32),
            text,
        ));
    }
    o.push(DrawOp::Text {
        i32_: 1004,
        y: 48,
        text: "WIFI".into(),
        scale: 3,
        color: text,
    });
    o.push(DrawOp::Text {
        i32_: 1004,
        y: 84,
        text: "SETTINGS".into(),
        scale: 2,
        color: muted,
    });
    for (r, loud) in [
        (Rect::new(108, 208, 104, 104), false),
        (Rect::new(228, 208, 104, 104), true),
    ] {
        let x = r.x + 24;
        let y = r.y + (r.h as i32 / 2);
        for (dx, dy, w, h) in [
            (0, -12, 12, 24),
            (12, -20, 12, 40),
            (24, -28, 8, 56),
            (40, -16, 4, 32),
            (44, -12, 4, 24),
        ] {
            o.push(DrawOp::Fill(Rect::new(x + dx, y + dy, w, h), text));
        }
        if loud {
            o.push(DrawOp::Fill(Rect::new(x + 56, y - 24, 4, 48), text));
            o.push(DrawOp::Fill(Rect::new(x + 60, y - 16, 4, 32), text));
        }
    }
    for (r, bright) in [
        (Rect::new(438, 208, 104, 104), false),
        (Rect::new(558, 208, 104, 104), true),
    ] {
        let cx = r.x + r.w as i32 / 2;
        let cy = r.y + r.h as i32 / 2;
        let half = if bright { 16 } else { 12 };
        let reach = if bright { 34 } else { 28 };
        let size = (half * 2) as u32;
        o.push(DrawOp::Fill(
            Rect::new(cx - half + 4, cy - half, size - 8, size),
            text,
        ));
        o.push(DrawOp::Fill(
            Rect::new(cx - half, cy - half + 4, size, size - 8),
            text,
        ));
        for (x, y, w, h) in [
            (cx - 3, cy - reach, 6, 10),
            (cx - 3, cy + reach - 10, 6, 10),
            (cx - reach, cy - 3, 10, 6),
            (cx + reach - 10, cy - 3, 10, 6),
        ] {
            o.push(DrawOp::Fill(Rect::new(x, y, w, h), text));
        }
        if bright {
            for (x, y) in [
                (cx - 25, cy - 25),
                (cx + 18, cy - 25),
                (cx - 25, cy + 18),
                (cx + 18, cy + 18),
            ] {
                o.push(DrawOp::Fill(Rect::new(x, y, 7, 7), text));
            }
        }
    }
    o.push(DrawOp::Outline(Rect::new(825, 238, 46, 34), text, 3));
    o.push(DrawOp::Fill(Rect::new(845, 272, 6, 7), text));
    o.push(DrawOp::Fill(Rect::new(816, 279, 64, 3), text));
    o.push(DrawOp::Text {
        i32_: 832,
        y: 247,
        text: ">_".into(),
        scale: 2,
        color: text,
    });
    for offset in (-16..=16).step_by(4) {
        o.push(DrawOp::Fill(
            Rect::new(1240 + offset, 40 + offset, 4, 4),
            text,
        ));
        o.push(DrawOp::Fill(
            Rect::new(1240 + offset, 40 - offset, 4, 4),
            text,
        ));
    }
    let centered = |o: &mut Vec<DrawOp>, r: Rect, value: String, scale: u32, colour: u32| {
        o.push(DrawOp::Centered {
            rect: r,
            text: value,
            scale,
            color: colour,
        })
    };
    centered(
        &mut o,
        Rect::new(82, 328, 276, 34),
        if s.volume == 0 {
            "OFF".into()
        } else {
            s.volume.to_string()
        },
        3,
        text,
    );
    centered(
        &mut o,
        Rect::new(82, 366, 276, 28),
        "VOLUME".into(),
        2,
        muted,
    );
    centered(
        &mut o,
        Rect::new(412, 328, 276, 34),
        s.brightness.to_string(),
        3,
        text,
    );
    centered(
        &mut o,
        Rect::new(412, 366, 276, 28),
        "BRIGHTNESS".into(),
        2,
        muted,
    );
    centered(
        &mut o,
        Rect::new(750, 328, 196, 34),
        "TERMINAL".into(),
        3,
        text,
    );
    centered(
        &mut o,
        Rect::new(750, 366, 196, 28),
        "/BIN/ASH".into(),
        2,
        muted,
    );
    centered(&mut o, Rect::new(994, 328, 196, 34), "KEYS".into(), 3, text);
    centered(
        &mut o,
        Rect::new(1036, 208, 112, 104),
        if s.keymap == "cz" { "CZ" } else { "EN" }.into(),
        4,
        text,
    );
    centered(
        &mut o,
        Rect::new(994, 366, 196, 28),
        if s.keymap == "cz" { "CZECH" } else { "US ANSI" }.into(),
        2,
        muted,
    );
    centered(
        &mut o,
        Rect::new(12, 406, 1256, 28),
        "RETRO DECK 1.0.0 RUST+ECL".into(),
        1,
        muted,
    );
    if !s.status.is_empty() {
        centered(
            &mut o,
            Rect::new(12, 440, 1256, 28),
            s.status.clone(),
            super::ui::fit_text_scale(&s.status, 1256, 2, 1),
            super::ui::color("footer"),
        );
    }
    o
}

pub fn execute_draw_ops(ops: &[DrawOp]) {
    for op in ops {
        match op {
            DrawOp::Clear(c) => crate::canvas::clear(*c),
            DrawOp::Fill(r, c) => super::ui::fill(r.x, r.y, r.w, r.h, *c),
            DrawOp::Outline(r, c, w) => super::ui::outline(r.x, r.y, r.w, r.h, *c, *w),
            DrawOp::Text {
                i32_,
                y,
                text,
                scale,
                color,
            } => super::ui::text(*i32_, *y, text, *scale, *color),
            DrawOp::Centered {
                rect,
                text,
                scale,
                color,
            } => super::ui::centered_in_rect(rect.x, rect.y, rect.w, rect.h, text, *scale, *color),
        }
    }
}

pub fn render_frame(s: &State, network: &crate::network::NetworkStatus) -> Layout {
    let ops = render_with_network(s, network);
    execute_draw_ops(&ops);
    Layout::default()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical() {
        assert_eq!(parse_volume_state("42\n"), Ok(42));
        assert!(parse_volume_state("042\n").is_err())
    }
    #[test]
    fn wrap() {
        let mut s = State::default();
        move_selection(&mut s, Direction::Previous);
        assert_eq!(s.selected, Target::Wifi)
    }
    #[test]
    fn raw() {
        assert_eq!(brightness_raw_value(50, 255), 128)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Back,
    Previous,
    Next,
    Confirm,
}
pub fn controller(s: &mut State, command: Command) -> Option<Effect> {
    match command {
        Command::Back => Some(activate(s, Target::Close)),
        Command::Previous => {
            move_selection(s, Direction::Previous);
            None
        }
        Command::Next => {
            move_selection(s, Direction::Next);
            None
        }
        Command::Confirm => Some(activate(s, s.selected)),
    }
}
pub fn save_keymap(path: &Path, keymap: &str) -> Result<(), String> {
    if keymap != "us" && keymap != "cz" {
        return Err("keymap must be us or cz".into());
    }
    state_file::write(path, format!("{keymap}\n").as_bytes())
}

pub fn load_keymap(path: &Path) -> Result<String, String> {
    match state_file::read(path)? {
        state_file::StateRead::Missing => {
            save_keymap(path, "us")?;
            Ok("us".to_owned())
        }
        state_file::StateRead::Value(bytes) => {
            let text =
                std::str::from_utf8(&bytes).map_err(|_| "keymap state is not UTF-8".to_owned())?;
            match text {
                "us\n" => Ok("us".to_owned()),
                "cz\n" => Ok("cz".to_owned()),
                _ => Err("keymap state must contain us or cz followed by a newline".into()),
            }
        }
    }
}
