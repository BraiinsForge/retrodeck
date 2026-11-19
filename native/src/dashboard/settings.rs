/// Pure settings reducer and persistence. Rendering is represented as deterministic draw ops.
use std::path::Path;

use crate::state_file;

pub const VOLUME_DEFAULT: u8 = 42;
pub const VOLUME_STEP: u8 = 5;
pub const VOLUME_STATE: &str = "/mnt/data/nes-deck/state/menu-volume.state";
pub const KEYMAP_STATE: &str = "/mnt/data/nes-deck/state/terminal-keymap.state";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Close,
    VolumeDown,
    VolumeUp,
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
    pub terminal: Rect,
    pub keymap: Rect,
}
impl Layout {
    pub fn for_settings(s: &State) -> Self {
        let mut layout = Self::default();
        if !s.volume_wifi {
            layout.terminal.x = 394;
            layout.keymap.x = 774;
        }
        layout
    }
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            close: Rect::new(1212, 12, 56, 56),
            wifi: Rect::new(926, 20, 262, 108),
            volume_down: Rect::new(148, 208, 104, 104),
            volume_up: Rect::new(268, 208, 104, 104),
            terminal: Rect::new(584, 208, 112, 104),
            keymap: Rect::new(964, 208, 112, 104),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub volume_wifi: bool,
    pub open: bool,
    pub selected: Target,
    pub pressed: Option<Target>,
    pub volume: u8,
    pub last_audible_volume: u8,
    pub keymap: String,
    pub status: String,
}
impl State {
    pub fn initial_target(&self) -> Target {
        if self.volume_wifi {
            Target::VolumeDown
        } else {
            Target::Terminal
        }
    }

    pub fn available(&self, target: Target) -> bool {
        self.volume_wifi || !matches!(target, Target::VolumeDown | Target::VolumeUp | Target::Wifi)
    }

    pub fn new(volume: u8, keymap: &str) -> Self {
        assert!(volume <= 100, "volume must be 0..100");
        assert!(matches!(keymap, "us" | "cz"), "unknown terminal keymap");
        Self {
            open: true,
            volume_wifi: false,
            selected: Target::Terminal,
            pressed: None,
            volume,
            last_audible_volume: if volume == 0 { VOLUME_DEFAULT } else { volume },
            keymap: keymap.into(),
            status: String::new(),
        }
    }
}
impl Default for State {
    fn default() -> Self {
        Self::new(VOLUME_DEFAULT, "us")
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
pub fn save_volume(path: &Path, v: u8) -> Result<(), String> {
    state_file::write(path, volume_state_text(v)?.as_bytes())
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

pub fn target_at(s: &State, l: &Layout, x: i32, y: i32) -> Option<Target> {
    [
        (Target::Close, l.close),
        (Target::VolumeDown, l.volume_down),
        (Target::VolumeUp, l.volume_up),
        (Target::Terminal, l.terminal),
        (Target::Keymap, l.keymap),
        (Target::Wifi, l.wifi),
    ]
    .iter()
    .copied()
    .find_map(|(t, r)| (s.available(t) && r.contains(x, y)).then_some(t))
}
pub fn activate(s: &State, t: Target) -> Effect {
    let cue = match t {
        Target::Close => Cue::Back,
        Target::VolumeDown => Cue::Previous,
        Target::VolumeUp => Cue::Next,
        _ => Cue::Confirm,
    };
    let action = match t {
        Target::Close => Action::Close,
        Target::VolumeDown => Action::Volume(s.volume.saturating_sub(VOLUME_STEP)),
        Target::VolumeUp => Action::Volume(if s.volume == 0 {
            s.last_audible_volume.max(VOLUME_STEP)
        } else {
            (s.volume + VOLUME_STEP).min(100)
        }),
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
    const O: [Target; 5] = [
        Target::VolumeDown,
        Target::VolumeUp,
        Target::Terminal,
        Target::Keymap,
        Target::Wifi,
    ];
    let order = if s.volume_wifi { &O[..] } else { &O[2..4] };
    let index = order.iter().position(|x| *x == s.selected);
    s.selected = match (index, d) {
        (Some(i), Direction::Previous) => order[(i + order.len() - 1) % order.len()],
        (Some(i), Direction::Next) => order[(i + 1) % order.len()],
        (None, _) => s.initial_target(),
    };
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
    let t = target_at(s, l, x, y);
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
    let layout = Layout::for_settings(s);
    if s.volume_wifi {
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
        let wifi = layout.wifi;
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
            (layout.volume_down, Target::VolumeDown),
            (layout.volume_up, Target::VolumeUp),
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
        for (r, loud) in [(layout.volume_down, false), (layout.volume_up, true)] {
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
    } else {
        o.push(DrawOp::Text {
            i32_: 64,
            y: 44,
            text: "SETTINGS".into(),
            scale: 3,
            color: text,
        });
    }
    for (rect, target) in [
        (layout.terminal, Target::Terminal),
        (layout.keymap, Target::Keymap),
    ] {
        pixel_panel(
            &mut o,
            rect,
            if s.selected == target {
                active
            } else {
                surface
            },
            if s.selected == target { accent } else { border },
        );
    }
    let terminal_x = layout.terminal.x + layout.terminal.w as i32 / 2;
    let terminal_y = layout.terminal.y;
    o.push(DrawOp::Outline(
        Rect::new(terminal_x - 23, terminal_y + 30, 46, 34),
        text,
        3,
    ));
    o.push(DrawOp::Fill(
        Rect::new(terminal_x - 3, terminal_y + 64, 6, 7),
        text,
    ));
    o.push(DrawOp::Fill(
        Rect::new(terminal_x - 32, terminal_y + 71, 64, 3),
        text,
    ));
    o.push(DrawOp::Text {
        i32_: terminal_x - 16,
        y: terminal_y + 39,
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
    let keys_x = layout.keymap.x + layout.keymap.w as i32 / 2;
    let caption = |x, w: u32, y, h| Rect::new(x - w as i32 / 2, y, w, h);
    if s.volume_wifi {
        let volume_x = (layout.volume_down.x + layout.volume_up.x + layout.volume_up.w as i32) / 2;
        centered(
            &mut o,
            caption(volume_x, 276, 328, 34),
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
            caption(volume_x, 276, 366, 28),
            "VOLUME".into(),
            2,
            muted,
        );
    }
    centered(
        &mut o,
        caption(terminal_x, 196, 328, 34),
        "TERMINAL".into(),
        3,
        text,
    );
    centered(
        &mut o,
        caption(terminal_x, 196, 366, 28),
        "/BIN/ASH".into(),
        2,
        muted,
    );
    centered(
        &mut o,
        caption(keys_x, 196, 328, 34),
        "KEYS".into(),
        3,
        text,
    );
    centered(
        &mut o,
        layout.keymap,
        if s.keymap == "cz" { "CZ" } else { "EN" }.into(),
        4,
        text,
    );
    centered(
        &mut o,
        caption(keys_x, 196, 366, 28),
        if s.keymap == "cz" { "CZECH" } else { "US ANSI" }.into(),
        2,
        muted,
    );
    centered(
        &mut o,
        Rect::new(12, 406, 1256, 28),
        concat!("RETRO DECK ", env!("CARGO_PKG_VERSION"), " RUST").into(),
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
    Layout::for_settings(s)
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
    fn controller_cycles_through_settings_actions_in_both_directions() {
        let mut state = State {
            volume_wifi: true,
            selected: Target::VolumeDown,
            ..State::default()
        };
        let order = [
            Target::VolumeDown,
            Target::VolumeUp,
            Target::Terminal,
            Target::Keymap,
            Target::Wifi,
        ];
        for expected in order.into_iter().cycle().skip(1).take(order.len()) {
            assert_eq!(controller(&mut state, Command::Next), None);
            assert_eq!(state.selected, expected);
        }
        for expected in order.into_iter().rev() {
            assert_eq!(controller(&mut state, Command::Previous), None);
            assert_eq!(state.selected, expected);
        }
        for (target, action) in [
            (Target::VolumeDown, Action::Volume(37)),
            (Target::VolumeUp, Action::Volume(47)),
            (Target::Terminal, Action::Terminal),
            (Target::Keymap, Action::Keymap("cz".into())),
            (Target::Wifi, Action::Wifi),
        ] {
            state.selected = target;
            assert_eq!(
                controller(&mut state, Command::Confirm).unwrap().action,
                action
            );
        }
        assert_eq!(
            controller(&mut state, Command::Back).unwrap().action,
            Action::Close
        );
    }

    #[test]
    fn touch_activates_the_released_control_and_cancels_a_drag() {
        let layout = Layout::default();
        let mut state = State {
            volume_wifi: true,
            selected: Target::VolumeDown,
            ..State::default()
        };
        for (target, rect) in [
            (Target::VolumeDown, layout.volume_down),
            (Target::VolumeUp, layout.volume_up),
            (Target::Terminal, layout.terminal),
            (Target::Keymap, layout.keymap),
            (Target::Wifi, layout.wifi),
            (Target::Close, layout.close),
        ] {
            let x = rect.x + rect.w as i32 / 2;
            let y = rect.y + rect.h as i32 / 2;
            assert_eq!(target_at(&state, &layout, x, y), Some(target));
            assert_eq!(touch(&mut state, &layout, x, y, true, false), None);
            let expected = activate(&state, target);
            assert_eq!(
                touch(&mut state, &layout, x, y, false, true),
                Some(expected)
            );
        }
        assert_eq!(touch(&mut state, &layout, 200, 260, true, false), None);
        assert_eq!(touch(&mut state, &layout, 320, 260, false, true), None);
        assert_eq!(state.pressed, None);
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
